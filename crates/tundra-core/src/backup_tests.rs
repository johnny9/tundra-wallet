use super::*;
const SINGLE: &str = include_str!("../../../tests/fixtures/single-sig.txt");
const PASSWORD: &str = "Public backup test password 🧊 ' spaces ";
const STORAGE_KEY: [u8; 32] = [0x11; 32]; // Storage encryption only, never a signing key.

#[test]
#[ignore = "explicit public interoperability fixture generator; requires an output path"]
fn export_interop_fixture() {
    let output = std::env::var_os("TUNDRA_PUBLIC_BACKUP_OUTPUT").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = sample(&dir.path().join("wallet.sqlite"));
    core.export_backup(Path::new(&output), PASSWORD.into())
        .unwrap();
}

#[test]
fn pinned_public_backup_fixture_uses_the_supported_profile() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/backup-v1.tundra");
    let info = inspect_backup(&path, PASSWORD.into()).unwrap();
    assert_eq!(info.wallets.len(), 1);
    assert_eq!(info.wallets[0].name, "Backup public fixture");
    assert!(info.wallets[0].total_sats.is_none());
    assert!(info.wallets[0].synced_at.is_none());
    assert_eq!(info.drafts, 0);
    assert_eq!(info.submissions, 0);
}

fn sample(path: &Path) -> (Core, String) {
    let core = Core::open_protected(path, STORAGE_KEY.to_vec()).unwrap();
    let wallet = core
        .import_wallet("Backup public fixture", SINGLE, crate::Network::Signet)
        .unwrap();
    let address = core.receive_address(&wallet.id).unwrap();
    core.set_label(
        &wallet.id,
        "addr",
        &address.address,
        "Backup private metadata sentinel 🧊",
    )
    .unwrap();
    (core, wallet.id)
}

#[test]
fn encrypted_snapshot_is_standalone_inspectable_and_does_not_change_live_wallet() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("wallet.sqlite");
    let backup = dir.path().join("snapshot.tundra");
    let (core, id) = sample(&live);
    let labels = core.export_labels(&id).unwrap();
    let result = core.export_backup(&backup, PASSWORD.into()).unwrap();
    assert_eq!(result.wallets.len(), 1);
    assert_eq!(result.wallets[0].id, id);
    assert!(result.wallets[0].total_sats.is_none());
    assert!(result.wallets[0].synced_at.is_none());
    assert_eq!(result.drafts, 0);
    let bytes = fs::read(&backup).unwrap();
    for sentinel in [
        b"SQLite format 3".as_slice(),
        b"Backup private metadata sentinel",
        SINGLE.as_bytes(),
        PASSWORD.as_bytes(),
    ] {
        assert!(
            !bytes
                .windows(sentinel.len())
                .any(|window| window == sentinel)
        );
    }
    let inspected = inspect_backup(&backup, PASSWORD.into()).unwrap();
    assert_eq!(result.created_at, inspected.created_at);
    assert_eq!(inspected.wallets[0].id, id);
    assert_eq!(core.export_labels(&id).unwrap(), labels);
    assert_eq!(core.receive_address(&id).unwrap().index, 1);
    let db = open_snapshot(&backup, &password_key(PASSWORD.into()).unwrap()).unwrap();
    let snapshot_labels: String = db
        .query_row("SELECT label FROM labels", [], |r| r.get(0))
        .unwrap();
    assert!(snapshot_labels.contains("metadata sentinel"));
    let snapshot = engine::load(&db, &id).unwrap();
    assert_eq!(
        snapshot
            .wallet
            .derivation_index(bdk_wallet::KeychainKind::External),
        Some(0)
    );
    assert!(db.execute("DELETE FROM wallets", []).is_err());
    assert_eq!(fs::read(&backup).unwrap(), bytes);
}

#[test]
fn backup_password_bounds_wrong_credentials_corruption_and_live_databases_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("wallet.sqlite");
    let backup = dir.path().join("snapshot.tundra");
    let (core, _) = sample(&live);
    for invalid in [
        "".into(),
        "too short".into(),
        "x".repeat(1025),
        "a long password\0with control".into(),
    ] {
        assert!(core.export_backup(&backup, invalid).is_err());
        assert!(!backup.exists());
    }
    core.export_backup(&backup, PASSWORD.into()).unwrap();
    let original = fs::read(&backup).unwrap();
    for wrong in [
        "a different long test password".into(),
        PASSWORD.trim().into(),
    ] {
        assert!(inspect_backup(&backup, wrong).is_err());
        assert_eq!(fs::read(&backup).unwrap(), original);
    }
    assert!(inspect_backup(&live, PASSWORD.into()).is_err());
    assert!(core.export_backup(&backup, PASSWORD.into()).is_err());
    assert_eq!(fs::read(&backup).unwrap(), original);
    for index in [0, 4096 + 100, original.len() - 1] {
        let mut corrupt = original.clone();
        corrupt[index] ^= 1;
        fs::write(&backup, &corrupt).unwrap();
        assert!(inspect_backup(&backup, PASSWORD.into()).is_err());
        assert_eq!(fs::read(&backup).unwrap(), corrupt);
    }
    fs::write(&backup, &original).unwrap();
    let sibling = backup.with_file_name("snapshot.tundra-wal");
    fs::write(&sibling, b"incomplete external snapshot").unwrap();
    assert!(inspect_backup(&backup, PASSWORD.into()).is_err());
    fs::remove_file(sibling).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&backup)
        .unwrap()
        .set_len(MAX_BYTES + 4096)
        .unwrap();
    assert!(inspect_backup(&backup, PASSWORD.into()).is_err());
}

#[test]
fn backup_preserves_signatures_final_bytes_and_uncertain_submission_records() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("public.sqlite");
    let backup = dir.path().join("snapshot.tundra");
    let (core, wallet_id, draft_id, signed) = crate::signing::tests::saved_ledger(&live);
    core.accept_signed_psbt(&wallet_id, &draft_id, &signed.serialize())
        .unwrap();
    let finalized = core.finalize_draft(&wallet_id, &draft_id).unwrap();
    {
        let db = core.lock().unwrap();
        // Existing public signature fixture with a synthetic uncertain receipt; no IO.
        db.execute("INSERT INTO broadcast_attempts(wallet_id,draft_id,endpoint,txid,wtxid,requested_at) VALUES(?1,?2,'http://127.0.0.1:1',?3,?4,1)", params![wallet_id, draft_id, finalized.txid, finalized.wtxid]).unwrap();
    }
    let info = core.export_backup(&backup, PASSWORD.into()).unwrap();
    assert_eq!(info.drafts, 1);
    assert_eq!(info.submissions, 1);
    assert!(info.wallets[0].synced_at.is_none());
    let snapshot = open_snapshot(&backup, &password_key(PASSWORD.into()).unwrap()).unwrap();
    let bytes: Vec<u8> = snapshot
        .query_row("SELECT transaction_bytes FROM finalized_drafts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(bytes, finalized.transaction_bytes);
    let signatures: String = snapshot
        .query_row("SELECT psbt FROM draft_signatures", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        signatures,
        core.lock()
            .unwrap()
            .query_row("SELECT psbt FROM draft_signatures", [], |r| r
                .get::<_, String>(0))
            .unwrap()
    );
    assert_eq!(
        snapshot
            .query_row("SELECT acknowledged FROM broadcast_attempts", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        snapshot
            .query_row("SELECT count(*) FROM reservations", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert_eq!(core.drafts(&wallet_id).unwrap()[0].state, "finalized");
}

#[test]
fn backup_rejects_unexpected_schema_and_future_version_without_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("wallet.sqlite");
    let backup = dir.path().join("snapshot.tundra");
    let (core, _) = sample(&live);
    core.export_backup(&backup, PASSWORD.into()).unwrap();
    let key = password_key(PASSWORD.into()).unwrap();
    let db = Connection::open(&backup).unwrap();
    storage::backend(&db).unwrap();
    db.pragma_update(None, "key", key.as_str()).unwrap();
    profile(&db, None).unwrap();
    db.execute_batch(
        "CREATE TRIGGER unexpected AFTER UPDATE ON wallets BEGIN DELETE FROM labels; END;",
    )
    .unwrap();
    drop(db);
    let before = fs::read(&backup).unwrap();
    assert!(inspect_backup(&backup, PASSWORD.into()).is_err());
    assert_eq!(fs::read(&backup).unwrap(), before);
    let db = Connection::open(&backup).unwrap();
    storage::backend(&db).unwrap();
    db.pragma_update(None, "key", key.as_str()).unwrap();
    profile(&db, None).unwrap();
    db.execute_batch("DROP TRIGGER unexpected; PRAGMA user_version=999;")
        .unwrap();
    drop(db);
    let before = fs::read(&backup).unwrap();
    assert!(inspect_backup(&backup, PASSWORD.into()).is_err());
    assert_eq!(fs::read(&backup).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn backup_refuses_final_symlinks_but_accepts_parent_directory_aliases() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias = dir.path().join("alias");
    symlink(&real, &alias).unwrap();
    let (core, _) = sample(&real.join("wallet.sqlite"));
    let backup = alias.join("snapshot.tundra");
    core.export_backup(&backup, PASSWORD.into()).unwrap();
    inspect_backup(&backup, PASSWORD.into()).unwrap();
    let link = real.join("linked.tundra");
    symlink(&backup, &link).unwrap();
    assert!(inspect_backup(&link, PASSWORD.into()).is_err());
    assert!(core.export_backup(&link, PASSWORD.into()).is_err());
}

#[test]
fn failed_exports_detach_the_snapshot_and_never_replace_an_existing_destination() {
    for stop in [
        ExportStep::Copied,
        ExportStep::Verified,
        ExportStep::Installed,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (core, id) = sample(&dir.path().join("wallet.sqlite"));
        let backup = dir.path().join("snapshot.tundra");
        let before = core.export_labels(&id).unwrap();
        assert!(
            core.export_snapshot(&backup, PASSWORD.into(), |step| {
                if step == stop {
                    Err(Error::Storage)
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(core.export_labels(&id).unwrap(), before);
        assert_eq!(core.receive_address(&id).unwrap().index, 1);
        if stop == ExportStep::Installed {
            inspect_backup(&backup, PASSWORD.into()).unwrap();
        } else {
            assert!(!backup.exists());
        }
        // A fresh successful export also proves a failure did not retain the attached key.
        core.export_backup(dir.path().join("retry.tundra"), PASSWORD.into())
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = sample(&dir.path().join("wallet.sqlite"));
    let backup = dir.path().join("snapshot.tundra");
    assert!(
        core.export_snapshot(&backup, PASSWORD.into(), |step| {
            if step == ExportStep::Verified {
                fs::write(&backup, b"another document").unwrap();
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(fs::read(&backup).unwrap(), b"another document");
}

#[test]
fn passwords_resembling_raw_keys_still_use_the_versioned_password_kdf() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = sample(&dir.path().join("wallet.sqlite"));
    let backup = dir.path().join("snapshot.tundra");
    let password = format!("x'{}'", "11".repeat(32));
    core.export_backup(&backup, password.clone()).unwrap();
    inspect_backup(&backup, password).unwrap();
    assert!(storage::open_protected(&backup, &STORAGE_KEY).is_err());
    let db = open_snapshot(
        &backup,
        &password_key(format!("x'{}'", "11".repeat(32))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        db.query_row("PRAGMA kdf_iter", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "256000"
    );
}

#[test]
#[ignore = "child fixture invoked by kill_during_backup_keeps_live_data_and_only_complete_exports"]
fn export_child() {
    use std::io::Write;
    let directory = std::env::var_os("TUNDRA_BACKUP_TEST_DIR").unwrap();
    let stop = std::env::var("TUNDRA_BACKUP_TEST_STOP").unwrap();
    let directory = Path::new(&directory);
    let core = Core::open_protected(directory.join("wallet.sqlite"), STORAGE_KEY.to_vec()).unwrap();
    core.export_snapshot(
        &directory.join("snapshot.tundra"),
        PASSWORD.into(),
        |step| {
            if format!("{step:?}") == stop {
                println!("at-boundary");
                std::io::stdout().flush().unwrap();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn kill_during_backup_keeps_live_data_and_only_complete_exports() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::Duration,
    };
    for stop in [
        ExportStep::Copied,
        ExportStep::Verified,
        ExportStep::Installed,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("wallet.sqlite");
        let backup = dir.path().join("snapshot.tundra");
        let (core, id) = sample(&live);
        let labels = core.export_labels(&id).unwrap();
        drop(core);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "backup::tests::export_child",
                "--nocapture",
            ])
            .env("TUNDRA_BACKUP_TEST_DIR", dir.path())
            .env("TUNDRA_BACKUP_TEST_STOP", format!("{stop:?}"))
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
        assert!(ready, "child did not reach {stop:?}");
        let core = Core::open_protected(&live, STORAGE_KEY.to_vec()).unwrap();
        assert_eq!(core.export_labels(&id).unwrap(), labels);
        assert_eq!(core.receive_address(&id).unwrap().index, 1);
        if stop == ExportStep::Installed {
            inspect_backup(&backup, PASSWORD.into()).unwrap();
        } else {
            assert!(!backup.exists());
        }
        for entry in fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let bytes = fs::read(path).unwrap();
                assert!(
                    !bytes
                        .windows(32)
                        .any(|part| part == b"Backup private metadata sentinel")
                );
            }
        }
        core.export_backup(dir.path().join("retry.tundra"), PASSWORD.into())
            .unwrap();
    }
}
