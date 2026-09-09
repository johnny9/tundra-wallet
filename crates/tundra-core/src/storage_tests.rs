use super::*;
use crate::{Core, Network};
use std::fs;

// Public database-encryption fixtures, never Bitcoin signing keys.
const KEY: [u8; 32] = [0x11; 32];
const OTHER_KEY: [u8; 32] = [0x22; 32];
const SINGLE: &str = include_str!("../../../tests/fixtures/single-sig.txt");
const LABEL: &str = "Public storage fixture 🧊 retained label";

fn encrypted(path: &Path) -> Core {
    Core::open_protected(path, KEY.to_vec()).unwrap()
}

#[test]
fn encrypted_database_and_wal_reopen_with_metadata_and_unknown_balance() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallet.sqlite");
    let core = encrypted(&path);
    let wallet = core
        .import_wallet("Storage test", SINGLE, Network::Signet)
        .unwrap();
    let first = core.receive_address(&wallet.id).unwrap();
    core.set_label(&wallet.id, "addr", &first.address, LABEL)
        .unwrap();
    for path in [&path, &directory.path().join("wallet.sqlite-wal")] {
        let bytes = fs::read(path).unwrap();
        assert!(!bytes.is_empty());
        for value in [
            b"SQLite format 3".as_slice(),
            LABEL.as_bytes(),
            first.address.as_bytes(),
            b"Storage test",
        ] {
            assert!(!bytes.windows(value.len()).any(|part| part == value));
        }
    }
    let db = core.lock().unwrap();
    let provider: String = db
        .query_row("PRAGMA cipher_provider", [], |r| r.get(0))
        .unwrap();
    assert_eq!(provider, tundra_sqlcipher::PROVIDER);
    let errors = db
        .prepare("PRAGMA cipher_integrity_check")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert!(errors.is_empty());
    assert_eq!(
        db.query_row("PRAGMA temp_store", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    drop(db);
    drop(core);
    let reopened = encrypted(&path);
    let summary = reopened.wallets().unwrap().remove(0);
    assert_eq!(summary.id, wallet.id);
    assert!(summary.synced_at.is_none());
    assert!(summary.total_sats.is_none());
    assert!(reopened.export_labels(&wallet.id).unwrap().contains(LABEL));
    assert_eq!(
        reopened.receive_address(&wallet.id).unwrap().index,
        first.index + 1
    );
}

#[test]
fn wrong_missing_and_invalid_storage_keys_never_reset_existing_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallet.sqlite");
    let core = encrypted(&path);
    let wallet = core
        .import_wallet("Preserve", SINGLE, Network::Signet)
        .unwrap();
    drop(core);
    let before = fs::read(&path).unwrap();
    for length in [0, 1, 31, 33, 64] {
        assert!(Core::open_protected(&path, vec![0x11; length]).is_err());
    }
    assert!(matches!(
        Core::open_protected(&path, OTHER_KEY.to_vec()),
        Err(Error::StorageLocked)
    ));
    assert!(Core::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(encrypted(&path).wallets().unwrap()[0].id, wallet.id);
    let new_path = directory.path().join("invalid.sqlite");
    assert!(Core::open_protected(&new_path, vec![]).is_err());
    assert!(!new_path.exists());
    assert!(Core::open_protected(":memory:", KEY.to_vec()).is_err());
}

#[test]
fn plaintext_is_refused_without_implicit_migration_or_data_loss() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.sqlite");
    let core = Core::open(&path).unwrap();
    let wallet = core
        .import_wallet("Legacy", SINGLE, Network::Signet)
        .unwrap();
    drop(core);
    let before = fs::read(&path).unwrap();
    assert!(before.starts_with(b"SQLite format 3"));
    assert!(matches!(
        Core::open_protected(&path, KEY.to_vec()),
        Err(Error::StorageLocked)
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        Core::open(&path).unwrap().wallets().unwrap()[0].id,
        wallet.id
    );
}

#[cfg(unix)]
#[test]
fn protected_storage_refuses_a_symlink_without_touching_its_target() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("original.sqlite");
    drop(encrypted(&path));
    let before = fs::read(&path).unwrap();
    let alias = directory.path().join("alias.sqlite");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(Core::open_protected(&alias, KEY.to_vec()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn tampered_authenticated_header_is_refused_without_repair_or_reset() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tamper.sqlite");
    let core = encrypted(&path);
    core.import_wallet("Preserve", SINGLE, Network::Signet)
        .unwrap();
    drop(core);
    let mut bytes = fs::read(&path).unwrap();
    bytes[256] ^= 1;
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        Core::open_protected(&path, KEY.to_vec()),
        Err(Error::StorageLocked)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn keyed_connections_share_atomic_state_and_rollback_failed_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shared.sqlite");
    let first = encrypted(&path);
    let wallet = first
        .import_wallet("Shared", SINGLE, Network::Signet)
        .unwrap();
    let second = encrypted(&path);
    assert_eq!(first.receive_address(&wallet.id).unwrap().index, 0);
    let address = second.receive_address(&wallet.id).unwrap();
    assert_eq!(address.index, 1);
    first
        .set_label(&wallet.id, "addr", &address.address, LABEL)
        .unwrap();
    {
        let mut db = first.lock().unwrap();
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        tx.execute("UPDATE labels SET label='uncommitted'", [])
            .unwrap();
        tx.execute("UPDATE wallets SET state_json='corrupt'", [])
            .unwrap();
        // Drop rolls back both metadata and BDK state; no replacement snapshot escapes.
    }
    assert!(second.export_labels(&wallet.id).unwrap().contains(LABEL));
    assert_eq!(second.receive_address(&wallet.id).unwrap().index, 2);
}

#[test]
fn keyed_schema_migration_preserves_wallet_and_refuses_future_versions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("migration.sqlite");
    let core = encrypted(&path);
    let wallet = core
        .import_wallet("Migration", SINGLE, Network::Signet)
        .unwrap();
    core.receive_address(&wallet.id).unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DROP TABLE broadcast_attempts; PRAGMA user_version=5;")
        .unwrap();
    drop(core);
    let migrated = encrypted(&path);
    assert_eq!(migrated.receive_address(&wallet.id).unwrap().index, 1);
    assert_eq!(
        migrated
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    migrated
        .lock()
        .unwrap()
        .execute_batch("PRAGMA user_version=999;")
        .unwrap();
    drop(migrated);
    assert!(matches!(
        Core::open_protected(&path, KEY.to_vec()),
        Err(Error::CorruptState)
    ));
    let db = open_protected(&path, KEY.to_vec()).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        999
    );
    assert_eq!(
        db.query_row("SELECT name FROM wallets", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "Migration"
    );
}
