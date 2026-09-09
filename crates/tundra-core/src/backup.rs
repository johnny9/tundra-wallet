//! Portable encrypted snapshots. Native callers own document exchange and password UX.
//! Inspection never opens a backup as live wallet storage or authorizes spending.
use crate::{Core, Error, Result, WalletSummary, engine, storage};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use std::{
    fs::{self, File},
    path::Path,
};
use zeroize::Zeroizing;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;
const APPLICATION_ID: u32 = 0x54444231; // TDB1, inside authenticated encrypted pages.
const SCHEMA_VERSION: u32 = 7;
const MANIFEST: &str = "CREATE TABLE backup_manifest (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL CHECK(version=1), created_at INTEGER NOT NULL CHECK(created_at>=0));";

#[derive(Debug, Clone)]
pub struct BackupSummary {
    pub created_at: u64,
    /// No balance/freshness claim is returned from a historical snapshot.
    pub wallets: Vec<WalletSummary>,
    pub drafts: u32,
    pub submissions: u32,
}

fn password_key(password: String) -> Result<Zeroizing<String>> {
    let password = Zeroizing::new(password);
    if password.chars().count() < 16
        || password.len() > 1024
        || password.chars().any(char::is_control)
    {
        return Err(Error::InvalidInput(
            "backup password must contain at least 16 characters and at most 1024 UTF-8 bytes, without control characters",
        ));
    }
    // Avoid interpreting a password resembling x'64hex' as a raw key. Exact UTF-8,
    // including spaces, is retained; no normalization or trimming changes the password.
    let mut key = Zeroizing::new(String::from("Tundra backup v1:"));
    key.push_str(&password);
    Ok(key)
}

fn profile(db: &Connection, schema: Option<&str>) -> Result<()> {
    // Version this profile instead of depending on mutable SQLCipher global defaults.
    db.pragma_update(schema, "cipher_page_size", 4096)?;
    db.pragma_update(schema, "kdf_iter", 256_000)?;
    db.pragma_update(schema, "cipher_hmac_algorithm", "HMAC_SHA512")?;
    db.pragma_update(schema, "cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512")?;
    db.pragma_update(schema, "cipher_use_hmac", true)?;
    db.pragma_update(schema, "cipher_plaintext_header_size", 0)?;
    Ok(())
}

fn standalone_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::StorageLocked)?;
    if !metadata.is_file()
        || !(4096..=MAX_BYTES).contains(&metadata.len())
        || metadata.len() % 4096 != 0
    {
        return Err(Error::StorageLocked);
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sibling = path.as_os_str().to_os_string();
        sibling.push(suffix);
        match fs::symlink_metadata(sibling) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(Error::StorageLocked),
        }
    }
    Ok(())
}

fn open_snapshot(path: &Path, key: &str) -> Result<Connection> {
    standalone_file(path)?;
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    storage::backend(&db)?;
    db.pragma_update(None, "key", key)?;
    profile(&db, None)?;
    db.pragma_update(None, "trusted_schema", false)?;
    db.pragma_update(None, "query_only", true)?;
    db.execute_batch("BEGIN DEFERRED")?;
    db.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| Error::StorageLocked)?;
    Ok(db)
}

type SchemaRow = (String, String, String, Option<String>);
fn schema(db: &Connection) -> Result<Vec<SchemaRow>> {
    let count: u32 = db.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
    let oversized: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE length(sql)>65536 OR length(name)>128 OR length(tbl_name)>128)", [], |r| r.get(0))?;
    if count > 128 || oversized {
        return Err(Error::CorruptState);
    }
    Ok(db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY name")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn validate(db: &Connection) -> Result<BackupSummary> {
    let application: u32 = db.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if application != APPLICATION_ID || version != SCHEMA_VERSION {
        return Err(Error::CorruptState);
    }
    // Reject arbitrary triggers, views, virtual tables or changed constraints before
    // querying wallet data. Only the exact known app schema plus manifest is accepted.
    let expected = storage::open_plain(Path::new(":memory:"))?;
    // Retain this versioned schema when the live app schema advances, so old backup
    // readers/restorers can be migrated deliberately instead of silently accepting drift.
    expected.execute_batch(include_str!("backup_schema_v1.sql"))?;
    expected.execute_batch(MANIFEST)?;
    if schema(db)? != schema(&expected)? {
        return Err(Error::CorruptState);
    }
    storage::integrity(db)?;
    let created_at = db.query_row(
        "SELECT created_at FROM backup_manifest WHERE id=1 AND version=1",
        [],
        |r| r.get::<_, u64>(0),
    )?;
    let count: u32 = db.query_row("SELECT count(*) FROM wallets", [], |r| r.get(0))?;
    if count > 1000 {
        return Err(Error::CorruptState);
    }
    let oversized: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM wallets WHERE length(CAST(state_json AS BLOB))>?1)",
        [MAX_STATE_BYTES],
        |r| r.get(0),
    )?;
    if oversized {
        return Err(Error::CorruptState);
    }
    let ids = db
        .prepare("SELECT id FROM wallets ORDER BY created_at,id")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut wallets = Vec::with_capacity(ids.len());
    for id in ids {
        let loaded = engine::load(db, &id)?;
        let payload = serde_json::json!({"descriptors": [
            {"desc": loaded.wallet.public_descriptor(bdk_wallet::KeychainKind::External).to_string(), "internal": false},
            {"desc": loaded.wallet.public_descriptor(bdk_wallet::KeychainKind::Internal).to_string(), "internal": true}
        ]}).to_string();
        let preview = crate::descriptor::preview_import(&payload, loaded.summary.network)?;
        if preview.policy != loaded.summary.policy {
            return Err(Error::CorruptState);
        }
        let mut summary = loaded.summary;
        summary.synced_at = None;
        wallets.push(summary);
    }
    Ok(BackupSummary {
        created_at,
        wallets,
        drafts: db.query_row("SELECT count(*) FROM drafts", [], |r| r.get(0))?,
        submissions: db.query_row("SELECT count(*) FROM broadcast_attempts", [], |r| r.get(0))?,
    })
}

pub fn inspect_backup(path: impl AsRef<Path>, password: String) -> Result<BackupSummary> {
    let key = password_key(password)?;
    let path = storage::canonical_path(path.as_ref())?;
    validate(&open_snapshot(&path, &key)?)
}

impl Core {
    /// Export a self-contained password-protected snapshot to a NEW app-private path.
    /// Native document writers may copy the ciphertext after this returns successfully.
    /// Existing destination files are never overwritten by this core operation.
    pub fn export_backup(&self, path: impl AsRef<Path>, password: String) -> Result<BackupSummary> {
        self.export_snapshot(path.as_ref(), password, |_| Ok(()))
    }
    fn export_snapshot(
        &self,
        path: &Path,
        password: String,
        mut boundary: impl FnMut(ExportStep) -> Result<()>,
    ) -> Result<BackupSummary> {
        let key = password_key(password)?;
        let path = storage::canonical_path(path)?;
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(Error::AlreadyExists),
        }
        let parent = path.parent().ok_or(Error::InvalidInput("backup path"))?;
        let output = tempfile::Builder::new()
            .prefix(".tundra-backup-")
            .tempfile_in(parent)
            .map_err(|_| Error::Storage)?;
        let output_path = output
            .path()
            .to_str()
            .ok_or(Error::InvalidInput("backup path"))?;
        let mut db = self.lock()?;
        db.execute(
            "ATTACH DATABASE ?1 AS backup_copy KEY ?2",
            params![output_path, key.as_str()],
        )?;
        let result = (|| -> Result<()> {
            profile(&db, Some("backup_copy"))?;
            db.pragma_update(Some("backup_copy"), "max_page_count", MAX_BYTES / 4096)?;
            db.pragma_update(Some("backup_copy"), "journal_mode", "DELETE")?;
            db.pragma_update(Some("backup_copy"), "synchronous", "FULL")?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let version: u32 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            if version != SCHEMA_VERSION {
                return Err(Error::CorruptState);
            }
            tx.query_row("SELECT sqlcipher_export('backup_copy')", [], |_| Ok(()))?;
            tx.execute_batch(&MANIFEST.replace(
                "CREATE TABLE backup_manifest",
                "CREATE TABLE backup_copy.backup_manifest",
            ))?;
            tx.execute(
                "INSERT INTO backup_copy.backup_manifest VALUES(1,1,?1)",
                [engine::now()?],
            )?;
            tx.pragma_update(Some("backup_copy"), "user_version", SCHEMA_VERSION)?;
            tx.pragma_update(Some("backup_copy"), "application_id", APPLICATION_ID)?;
            boundary(ExportStep::Copied)?;
            tx.commit()?;
            Ok(())
        })();
        // Failed export must not leave an attached DB/key in the long-lived core.
        let detached = db.execute_batch("DETACH DATABASE backup_copy");
        result?;
        detached?;
        drop(db);
        let verified = open_snapshot(output.path(), &key)?;
        let summary = validate(&verified)?;
        verified.close().map_err(|_| Error::Storage)?;
        output.as_file().sync_all().map_err(|_| Error::Storage)?;
        boundary(ExportStep::Verified)?;
        let installed = output.persist_noclobber(&path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                Error::AlreadyExists
            } else {
                Error::Storage
            }
        })?;
        installed.sync_all().map_err(|_| Error::Storage)?;
        boundary(ExportStep::Installed)?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| Error::Storage)?;
        Ok(summary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportStep {
    Copied,
    Verified,
    Installed,
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
