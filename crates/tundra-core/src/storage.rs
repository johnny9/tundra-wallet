//! SQLCipher connection policy. Native platforms own wrapping/retaining storage keys.
use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use std::{
    fmt::Write,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroizing;

fn backend(conn: &Connection) -> Result<()> {
    // A plain SQLite build ignores PRAGMA key. Verify the linked implementation first.
    let version: String = conn.query_row("PRAGMA cipher_version", [], |r| r.get(0))?;
    if version != tundra_sqlcipher::VERSION
        || rusqlite::version() != tundra_sqlcipher::SQLITE_VERSION
    {
        return Err(Error::Storage);
    }
    conn.pragma_update(None, "cipher_log_level", "NONE")?;
    conn.pragma_update(None, "cipher_memory_security", true)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(())
}

pub(crate) fn open_plain(path: &Path) -> Result<Connection> {
    let conn = if path == Path::new(":memory:") {
        Connection::open_in_memory()?
    } else {
        Connection::open_with_flags(
            canonical_path(path)?,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?
    };
    backend(&conn)?;
    // Probe before setting WAL or running schema migrations on an encrypted file.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    Ok(conn)
}

pub(crate) fn validate_key_path(path: &Path, storage_key: &[u8]) -> Result<()> {
    if storage_key.len() != 32 || path.as_os_str().is_empty() || path == Path::new(":memory:") {
        return Err(Error::InvalidInput("protected storage"));
    }
    Ok(())
}

fn raw_key(storage_key: &[u8]) -> Result<Zeroizing<String>> {
    let mut raw_key = Zeroizing::new(String::with_capacity(67));
    raw_key.push_str("x'");
    for byte in storage_key.iter() {
        write!(&mut *raw_key, "{byte:02x}").map_err(|_| Error::Storage)?;
    }
    raw_key.push('\'');
    Ok(raw_key)
}

pub(crate) fn open_protected(path: &Path, storage_key: &[u8]) -> Result<Connection> {
    validate_key_path(path, storage_key)?;
    let canonical = canonical_path(path)?;
    let path = canonical.as_path();
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.len() < 16 => {
            return Err(Error::StorageLocked);
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(Error::Storage),
    }
    // Do not interpret URI key/cipher parameters or follow a final-path symlink.
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    backend(&conn)?;
    let raw_key = raw_key(storage_key)?;
    conn.pragma_update(None, "key", raw_key.as_str())?;
    // Setting a key alone does not validate it. This read must precede all DB mutations.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| Error::StorageLocked)?;
    Ok(conn)
}

fn canonical_path(path: &Path) -> Result<PathBuf> {
    // Android's app-data roots and Apple's /var can be directory symlinks. Resolve
    // only the parent: the final database component must still pass NOFOLLOW.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or(Error::InvalidInput("storage path"))?;
    Ok(parent
        .canonicalize()
        .map_err(|_| Error::Storage)?
        .join(name))
}

fn lock_file(path: &Path) -> Result<File> {
    let mut name = canonical_path(path)?.into_os_string();
    name.push(".tundra-lock");
    match fs::symlink_metadata(&name) {
        Ok(metadata) if !metadata.is_file() => return Err(Error::Storage),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(Error::Storage),
    }
    // This lock file contains no key or wallet metadata. Never remove it during use:
    // unlinking a held lock could let a later opener lock a different inode.
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(name).map_err(|_| Error::Storage)
}

pub(crate) struct StorageLock(File);
impl Drop for StorageLock {
    fn drop(&mut self) {
        // Explicit unlock also releases a transient inherited descriptor during a
        // concurrent process spawn; relying only on close can briefly retain the lock.
        #[cfg(unix)]
        let _ = rustix::fs::flock(&self.0, rustix::fs::FlockOperation::Unlock);
        #[cfg(not(unix))]
        let _ = self.0.unlock();
    }
}

fn acquire(file: &File, exclusive: bool) -> Result<()> {
    // Rust 1.93's std File locking is unsupported on Android. The pinned safe
    // rustix wrapper provides the same flock semantics on all three Unix targets.
    #[cfg(unix)]
    {
        use rustix::fs::{FlockOperation, flock};
        let operation = if exclusive {
            FlockOperation::NonBlockingLockExclusive
        } else {
            FlockOperation::NonBlockingLockShared
        };
        flock(file, operation).map_err(|error| {
            if std::io::Error::from(error).kind() == ErrorKind::WouldBlock {
                Error::StorageBusy
            } else {
                Error::Storage
            }
        })
    }
    #[cfg(not(unix))]
    {
        let result = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        result.map_err(|_| Error::StorageBusy)
    }
}

pub(crate) fn shared_lock(path: &Path) -> Result<Option<Arc<StorageLock>>> {
    if path == Path::new(":memory:") {
        return Ok(None);
    }
    let file = lock_file(path)?;
    acquire(&file, false)?;
    Ok(Some(Arc::new(StorageLock(file))))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageFormat {
    Missing,
    Plaintext,
    ProtectedOrUnknown,
}

/// A format hint for native key retention, never proof that a database is valid.
/// An empty or unrecognized existing file must not authorize replacement of a lost key.
pub fn storage_format(path: impl AsRef<Path>) -> Result<StorageFormat> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() || path == Path::new(":memory:") {
        return Err(Error::InvalidInput("protected storage"));
    }
    let canonical = canonical_path(path)?;
    let path = canonical.as_path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) if value.is_file() => value,
        Ok(_) => return Err(Error::Storage),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(StorageFormat::Missing),
        Err(_) => return Err(Error::Storage),
    };
    if metadata.len() < 16 {
        return Ok(StorageFormat::ProtectedOrUnknown);
    }
    let mut header = [0u8; 16];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|_| Error::Storage)?;
    Ok(if &header == b"SQLite format 3\0" {
        StorageFormat::Plaintext
    } else {
        StorageFormat::ProtectedOrUnknown
    })
}

/// Upgrade app-owned plaintext storage after the native platform durably retains the key.
/// No other Core connection may be open. Repeating after a completed replacement only
/// validates the protected file with the supplied key; it never rekeys or resets it.
pub fn migrate_plaintext_storage(path: impl AsRef<Path>, storage_key: Vec<u8>) -> Result<()> {
    let storage_key = Zeroizing::new(storage_key);
    migrate(path.as_ref(), &storage_key, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationStep {
    Exported,
    Verified,
    Replaced,
}

fn migrate(
    path: &Path,
    storage_key: &[u8],
    mut boundary: impl FnMut(MigrationStep) -> Result<()>,
) -> Result<()> {
    validate_key_path(path, storage_key)?;
    let canonical = canonical_path(path)?;
    let path = canonical.as_path();
    let lock = lock_file(path)?;
    acquire(&lock, true)?;
    let lock = StorageLock(lock);
    match storage_format(path)? {
        StorageFormat::Missing => return Err(Error::NotFound),
        StorageFormat::ProtectedOrUnknown => {
            open_protected(path, storage_key)?;
            return Ok(());
        }
        StorageFormat::Plaintext => {}
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = tempfile::Builder::new()
        .prefix(".tundra-protect-")
        .tempfile_in(parent)
        .map_err(|_| Error::Storage)?;
    let mut source = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    backend(&source)?;
    // Checkpoint/delete legacy WAL before any replacement. Another SQLite handle that
    // ignores the app lock still prevents the journal-mode transition while holding WAL.
    let journal: String = source.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))?;
    if journal != "delete" {
        return Err(Error::StorageBusy);
    }
    source.execute_batch("PRAGMA locking_mode=EXCLUSIVE; PRAGMA synchronous=FULL;")?;
    let version: i64 = source.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if !(1..=7).contains(&version) {
        return Err(Error::CorruptState);
    }
    let raw_key = raw_key(storage_key)?;
    let temporary_path = output
        .path()
        .to_str()
        .ok_or(Error::InvalidInput("storage path"))?;
    source.execute(
        "ATTACH DATABASE ?1 AS protected_copy KEY ?2",
        params![temporary_path, raw_key.as_str()],
    )?;
    {
        let tx = source.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        tx.query_row("SELECT sqlcipher_export('protected_copy')", [], |_| Ok(()))?;
        tx.pragma_update(Some("protected_copy"), "user_version", version)?;
        boundary(MigrationStep::Exported)?;
        tx.commit()?;
    }
    source.execute_batch("DETACH DATABASE protected_copy")?;
    // The encrypted file uses DELETE journaling; its committed bytes are self-contained.
    let verified = open_protected(output.path(), storage_key)?;
    integrity(&verified)?;
    if verified.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? != version {
        return Err(Error::CorruptState);
    }
    verified.close().map_err(|_| Error::Storage)?;
    output.as_file().sync_all().map_err(|_| Error::Storage)?;
    boundary(MigrationStep::Verified)?;
    source.close().map_err(|_| Error::Storage)?;
    // Same-directory atomic rename, no plaintext backup. The advisory lock remains held
    // through the directory sync, so no current core opens either intermediate state.
    let installed = output.persist(path).map_err(|_| Error::Storage)?;
    installed.sync_all().map_err(|_| Error::Storage)?;
    boundary(MigrationStep::Replaced)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::Storage)?;
    drop(lock);
    Ok(())
}

fn integrity(db: &Connection) -> Result<()> {
    let mut cipher = db.prepare("PRAGMA cipher_integrity_check")?;
    if cipher.query([])?.next()?.is_some() {
        return Err(Error::CorruptState);
    }
    let check: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(Error::CorruptState);
    }
    let mut foreign = db.prepare("PRAGMA foreign_key_check")?;
    if foreign.query([])?.next()?.is_some() {
        return Err(Error::CorruptState);
    }
    Ok(())
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
