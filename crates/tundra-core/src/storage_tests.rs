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

#[cfg(unix)]
#[test]
fn platform_directory_aliases_work_without_bypassing_migration_locks() {
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias = directory.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let path = alias.join("protected.sqlite");
    let core = encrypted(&path);
    let wallet = core
        .import_wallet("Directory alias", SINGLE, Network::Signet)
        .unwrap();
    assert!(matches!(
        migrate_plaintext_storage(real.join("protected.sqlite"), KEY.to_vec()),
        Err(Error::StorageBusy)
    ));
    drop(core);
    assert_eq!(
        encrypted(&real.join("protected.sqlite")).wallets().unwrap()[0].id,
        wallet.id
    );
    let legacy = alias.join("legacy.sqlite");
    let old = Core::open(&legacy).unwrap();
    assert!(matches!(
        migrate_plaintext_storage(real.join("legacy.sqlite"), KEY.to_vec()),
        Err(Error::StorageBusy)
    ));
    drop(old);
    migrate_plaintext_storage(&legacy, KEY.to_vec()).unwrap();
    assert_eq!(
        storage_format(real.join("legacy.sqlite")).unwrap(),
        StorageFormat::ProtectedOrUnknown
    );
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
    let db = open_protected(&path, &KEY).unwrap();
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

#[test]
fn format_hint_never_calls_an_empty_or_unknown_existing_file_missing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("format.sqlite");
    assert_eq!(storage_format(&path).unwrap(), StorageFormat::Missing);
    fs::write(&path, []).unwrap();
    assert_eq!(
        storage_format(&path).unwrap(),
        StorageFormat::ProtectedOrUnknown
    );
    assert!(matches!(
        Core::open_protected(&path, KEY.to_vec()),
        Err(Error::StorageLocked)
    ));
    assert!(matches!(
        migrate_plaintext_storage(&path, KEY.to_vec()),
        Err(Error::StorageLocked)
    ));
    assert!(fs::read(&path).unwrap().is_empty());
    fs::write(&path, [1u8; 64]).unwrap();
    assert_eq!(
        storage_format(&path).unwrap(),
        StorageFormat::ProtectedOrUnknown
    );
    fs::remove_file(&path).unwrap();
    drop(Core::open(&path).unwrap());
    assert_eq!(storage_format(&path).unwrap(), StorageFormat::Plaintext);
}

#[test]
fn plaintext_upgrade_preserves_signatures_reservations_and_uncertain_submissions() {
    let directory = tempfile::tempdir().unwrap();
    // Exercise bound ATTACH path parameters with quotes and Unicode as well.
    let nested = directory.path().join("Storage's 🧊");
    fs::create_dir(&nested).unwrap();
    let path = nested.join("Wallet's.sqlite");
    let (core, wallet, draft, signed) = crate::signing::tests::saved_ledger(&path);
    let review = core.drafts(&wallet).unwrap().remove(0);
    let outpoint = review.inputs[0].outpoint.clone();
    core.set_label(&wallet, "output", &outpoint, LABEL).unwrap();
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    let final_tx = core.finalize_draft(&wallet, &draft).unwrap();
    {
        let db = core.lock().unwrap();
        // Persist public fixture uncertainty directly; this test performs no network IO.
        db.execute("INSERT INTO broadcast_attempts(wallet_id,draft_id,endpoint,txid,wtxid,requested_at) VALUES(?1,?2,'https://unused.invalid',?3,?4,1)", params![wallet,draft,final_tx.txid,final_tx.wtxid]).unwrap();
        db.execute(
            "INSERT INTO hardware_registrations VALUES(?1,'deadbeef',?2,?3)",
            params![wallet, vec![1u8; 32], vec![2u8; 32]],
        )
        .unwrap();
    }
    let labels = core.export_labels(&wallet).unwrap();
    let progress = core.signing_progress(&wallet, &draft).unwrap();
    let balance = core.wallets().unwrap()[0].total_sats;
    drop(core);
    migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
    assert_eq!(
        storage_format(&path).unwrap(),
        StorageFormat::ProtectedOrUnknown
    );
    assert!(
        !fs::read(&path)
            .unwrap()
            .windows(LABEL.len())
            .any(|x| x == LABEL.as_bytes())
    );
    assert!(!nested.join("Wallet's.sqlite-wal").exists());
    assert!(!nested.join("Wallet's.sqlite-shm").exists());
    let reopened = encrypted(&path);
    assert_eq!(reopened.wallets().unwrap()[0].total_sats, balance);
    assert_eq!(reopened.export_labels(&wallet).unwrap(), labels);
    assert_eq!(
        reopened.signing_progress(&wallet, &draft).unwrap(),
        progress
    );
    assert_eq!(
        reopened.finalized_draft(&wallet, &draft).unwrap().unwrap(),
        final_tx
    );
    assert!(
        !reopened
            .broadcast_status(&wallet, &draft)
            .unwrap()
            .unwrap()
            .acknowledged
    );
    assert_eq!(
        reopened
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        reopened
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM hardware_registrations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    reopened.set_frozen(&wallet, &outpoint, true).unwrap();
    drop(reopened);
    // Repeating after restart validates the existing cipher; it never changes its key.
    migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
    assert!(migrate_plaintext_storage(&path, OTHER_KEY.to_vec()).is_err());
    let reopened = encrypted(&path);
    assert!(reopened.finalized_draft(&wallet, &draft).is_err());
    assert_eq!(
        reopened
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM freezes", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn migration_refuses_live_core_clones_and_independent_connections() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("busy.sqlite");
    let first = Core::open(&path).unwrap();
    first
        .import_wallet("Busy", SINGLE, Network::Signet)
        .unwrap();
    let clone = first.clone();
    let independent = Core::open(&path).unwrap();
    for handle in [first, clone, independent] {
        assert!(matches!(
            migrate_plaintext_storage(&path, KEY.to_vec()),
            Err(Error::StorageBusy)
        ));
        drop(handle);
    }
    migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
    assert_eq!(encrypted(&path).wallets().unwrap()[0].name, "Busy");
}

#[test]
fn migration_also_refuses_an_uncooperative_sqlite_wal_reader() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reader.sqlite");
    let core = Core::open(&path).unwrap();
    core.import_wallet("Reader", SINGLE, Network::Signet)
        .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("BEGIN; SELECT count(*) FROM wallets;")
        .unwrap();
    drop(core);
    assert!(migrate_plaintext_storage(&path, KEY.to_vec()).is_err());
    assert_eq!(storage_format(&path).unwrap(), StorageFormat::Plaintext);
    assert_eq!(
        db.query_row("SELECT name FROM wallets", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "Reader"
    );
    drop(db);
    migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
    assert_eq!(encrypted(&path).wallets().unwrap()[0].name, "Reader");
}

#[test]
fn migration_exclusive_guard_blocks_new_cores_until_replacement_finishes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("exclusive.sqlite");
    drop(Core::open(&path).unwrap());
    migrate(&path, &KEY, |_| {
        assert!(matches!(Core::open(&path), Err(Error::StorageBusy)));
        assert!(matches!(
            Core::open_protected(&path, KEY.to_vec()),
            Err(Error::StorageBusy)
        ));
        Ok(())
    })
    .unwrap();
    assert!(encrypted(&path).wallets().unwrap().is_empty());
}

#[test]
fn migration_refuses_missing_invalid_and_future_schema_without_replacing_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("future.sqlite");
    assert!(matches!(
        migrate_plaintext_storage(&path, KEY.to_vec()),
        Err(Error::NotFound)
    ));
    assert!(!path.exists());
    let core = Core::open(&path).unwrap();
    core.import_wallet("Future", SINGLE, Network::Signet)
        .unwrap();
    core.lock()
        .unwrap()
        .execute_batch("PRAGMA user_version=999;")
        .unwrap();
    drop(core);
    assert!(matches!(
        migrate_plaintext_storage(&path, KEY.to_vec()),
        Err(Error::CorruptState)
    ));
    assert_eq!(storage_format(&path).unwrap(), StorageFormat::Plaintext);
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT name FROM wallets", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "Future"
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        999
    );
    assert!(migrate_plaintext_storage(&path, vec![]).is_err());
}

#[test]
fn injected_migration_errors_leave_an_original_or_complete_encrypted_database() {
    for boundary in [
        MigrationStep::Exported,
        MigrationStep::Verified,
        MigrationStep::Replaced,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("failure.sqlite");
        let core = Core::open(&path).unwrap();
        let wallet = core
            .import_wallet("Retained", SINGLE, Network::Signet)
            .unwrap();
        let address = core.receive_address(&wallet.id).unwrap();
        core.set_label(&wallet.id, "addr", &address.address, LABEL)
            .unwrap();
        drop(core);
        assert!(
            migrate(&path, &KEY, |step| if step == boundary {
                Err(Error::Storage)
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(
            storage_format(&path).unwrap(),
            if boundary == MigrationStep::Replaced {
                StorageFormat::ProtectedOrUnknown
            } else {
                StorageFormat::Plaintext
            }
        );
        migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
        let reopened = encrypted(&path);
        assert_eq!(reopened.receive_address(&wallet.id).unwrap().index, 1);
        assert!(reopened.export_labels(&wallet.id).unwrap().contains(LABEL));
    }
}

#[test]
#[ignore = "child fixture invoked by kill_during_migration_keeps_a_recoverable_database"]
fn migration_child() {
    use std::io::Write;
    let path = std::env::var_os("TUNDRA_MIGRATION_TEST_DB").unwrap();
    let stop = std::env::var("TUNDRA_MIGRATION_TEST_STOP").unwrap();
    migrate(Path::new(&path), &KEY, |step| {
        if format!("{step:?}") == stop {
            println!("at-boundary");
            std::io::stdout().flush().unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn kill_during_migration_keeps_a_recoverable_database() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::Duration,
    };
    for boundary in [
        MigrationStep::Exported,
        MigrationStep::Verified,
        MigrationStep::Replaced,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("crash.sqlite");
        let core = Core::open(&path).unwrap();
        let wallet = core
            .import_wallet("Migration crash", SINGLE, Network::Signet)
            .unwrap();
        let address = core.receive_address(&wallet.id).unwrap();
        core.set_label(&wallet.id, "addr", &address.address, LABEL)
            .unwrap();
        drop(core);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "storage::tests::migration_child",
                "--nocapture",
            ])
            .env("TUNDRA_MIGRATION_TEST_DB", &path)
            .env("TUNDRA_MIGRATION_TEST_STOP", format!("{boundary:?}"))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line.unwrap() == "at-boundary" {
                    let _ = sender.send(());
                    break;
                }
            }
        });
        let ready = receiver.recv_timeout(Duration::from_secs(10)).is_ok();
        child.kill().unwrap();
        child.wait().unwrap();
        reader.join().unwrap();
        assert!(ready, "child did not reach {boundary:?}");
        assert_eq!(
            storage_format(&path).unwrap(),
            if boundary == MigrationStep::Replaced {
                StorageFormat::ProtectedOrUnknown
            } else {
                StorageFormat::Plaintext
            }
        );
        migrate_plaintext_storage(&path, KEY.to_vec()).unwrap();
        let reopened = encrypted(&path);
        assert_eq!(reopened.receive_address(&wallet.id).unwrap().index, 1);
        assert!(reopened.export_labels(&wallet.id).unwrap().contains(LABEL));
        integrity(&reopened.lock().unwrap()).unwrap();
        // Any abandoned temporary database/journal contains no plaintext public sentinel.
        for entry in fs::read_dir(directory.path()).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".tundra-protect-")
            {
                let bytes = fs::read(entry.path()).unwrap();
                assert!(!bytes.windows(LABEL.len()).any(|x| x == LABEL.as_bytes()));
            }
        }
    }
}
