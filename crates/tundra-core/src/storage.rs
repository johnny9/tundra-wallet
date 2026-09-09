//! SQLCipher connection policy. Native platforms own wrapping/retaining storage keys.
use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::{fmt::Write, path::Path};
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
    let conn = Connection::open(path)?;
    backend(&conn)?;
    // Probe before setting WAL or running schema migrations on an encrypted file.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    Ok(conn)
}

pub(crate) fn open_protected(path: &Path, storage_key: Vec<u8>) -> Result<Connection> {
    // Consume and clear our owned buffers. Platform and UniFFI copies have separate lives.
    let storage_key = Zeroizing::new(storage_key);
    if storage_key.len() != 32 || path.as_os_str().is_empty() || path == Path::new(":memory:") {
        return Err(Error::InvalidInput("protected storage"));
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
    let mut raw_key = Zeroizing::new(String::with_capacity(67));
    raw_key.push_str("x'");
    for byte in storage_key.iter() {
        write!(&mut *raw_key, "{byte:02x}").map_err(|_| Error::Storage)?;
    }
    raw_key.push('\'');
    conn.pragma_update(None, "key", raw_key.as_str())?;
    // Setting a key alone does not validate it. This read must precede all DB mutations.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| Error::StorageLocked)?;
    Ok(conn)
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
