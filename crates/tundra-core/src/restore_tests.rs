use super::*;
use crate::{BroadcastObservation, CoinStatus, Network};
use bdk_wallet::bitcoin::{Transaction, consensus::deserialize};
const PASSWORD: &str = "Public backup test password 🧊 ' spaces ";
const KEY: [u8; 32] = [0x33; 32]; // Public storage encryption fixture; never a signing key.

fn public_backup() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/backup-v1.tundra")
}
fn submitted(path: &Path) -> (Core, String, String, crate::FinalizedReview) {
    let (core, wallet, draft, signed) = crate::signing::tests::saved_ledger(path);
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    let finalized = core.finalize_draft(&wallet, &draft).unwrap();
    core.lock().unwrap().execute("INSERT INTO broadcast_attempts(wallet_id,draft_id,endpoint,txid,wtxid,requested_at) VALUES(?1,?2,'http://127.0.0.1:1',?3,?4,1)", params![wallet,draft,finalized.txid,finalized.wtxid]).unwrap();
    (core, wallet, draft, finalized)
}
fn held(core: &Core, wallet: &str) -> u32 {
    core.lock()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM recovery_holds WHERE wallet_id=?1",
            [wallet],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn restore_old_profile_into_new_protected_storage_preserves_metadata_and_unknown_balance() {
    let dir = tempfile::tempdir().unwrap();
    let source = public_backup();
    let before = fs::read(&source).unwrap();
    let destination = dir.path().join("restored.sqlite");
    let summary = restore_backup(&source, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    assert_eq!(summary.wallets.len(), 1);
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    let wallet = core.wallets().unwrap().remove(0);
    assert_eq!(wallet.name, "Backup public fixture");
    assert_eq!(wallet.network, Network::Signet);
    assert!(
        wallet.total_sats.is_none()
            && wallet.available_sats.is_none()
            && wallet.synced_at.is_none()
    );
    assert_eq!(core.receive_address(&wallet.id).unwrap().index, 1);
    assert!(
        core.export_labels(&wallet.id)
            .unwrap()
            .contains("Backup private metadata sentinel")
    );
    assert!(Core::open(&destination).is_err());
    assert!(Core::open_protected(&destination, vec![0x22; 32]).is_err());
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        core.lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
    let next = dir.path().join("new-schema.tundra");
    core.export_backup(&next, PASSWORD.into()).unwrap();
    assert_eq!(
        inspect_backup(&next, PASSWORD.into()).unwrap().wallets[0].id,
        wallet.id
    );
}

#[test]
fn recovered_submission_needs_explicit_fresh_review_and_revalidates_freezes_and_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, finalized) = submitted(&dir.path().join("source.sqlite"));
    let original = source.drafts(&wallet).unwrap().remove(0);
    let input = original.inputs[0].outpoint.clone();
    source
        .set_label(&wallet, "output", &input, "Current label 🧊")
        .unwrap();
    source.set_frozen(&wallet, &input, true).unwrap();
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let destination = dir.path().join("restored.sqlite");
    restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "invalidated");
    assert_eq!(
        core.drafts(&wallet).unwrap()[0].inputs[0].label,
        original.inputs[0].label
    );
    assert!(
        core.export_labels(&wallet)
            .unwrap()
            .contains("Current label 🧊")
    );
    assert_eq!(held(&core, &wallet), 2);
    let info = core.broadcast_status(&wallet, &draft).unwrap().unwrap();
    assert!(!info.acknowledged);
    assert!(core.recovery_required(&wallet, &draft).unwrap());
    assert!(core.signing_progress(&wallet, &draft).is_err());
    assert!(core.finalize_draft(&wallet, &draft).is_err());
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &finalized.txid, info.attempt_id, false)
            .is_err()
    );
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &finalized.txid, info.attempt_id, true)
            .is_err()
    );
    // Test-only fresh timestamp over the public fixture graph. Actual sync/reorg IO
    // is separately covered by keyless regtest; this does not claim a network scan.
    core.lock()
        .unwrap()
        .execute("UPDATE wallets SET synced_at=2", [])
        .unwrap();
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &finalized.txid, info.attempt_id, true)
            .is_err()
    ); // frozen
    core.set_frozen(&wallet, &input, false).unwrap();
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &"00".repeat(32), info.attempt_id, true)
            .is_err()
    );
    assert!(
        core.resume_recovered_submission(
            &wallet,
            &draft,
            &finalized.txid,
            info.attempt_id + 1,
            true
        )
        .is_err()
    );
    assert_eq!(held(&core, &wallet), 2);
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "invalidated");
    assert!(
        core.create_draft(crate::DraftRequest {
            wallet_id: wallet.clone(),
            payment: crate::Payment::Send {
                address: original.outputs[0].address.clone(),
                sats: 1000
            },
            selected_outpoints: Some(vec![input.clone()]),
            fee_sat_per_kwu: 250,
            label: "Blocked recovery input".into()
        })
        .is_err()
    );
    let resumed = core
        .resume_recovered_submission(&wallet, &draft, &finalized.txid, info.attempt_id, true)
        .unwrap();
    assert_eq!(resumed, finalized);
    assert_eq!(held(&core, &wallet), 0);
    assert!(!core.recovery_required(&wallet, &draft).unwrap());
    assert_eq!(
        core.broadcast_status(&wallet, &draft).unwrap().unwrap(),
        info
    );
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &finalized.txid, info.attempt_id, true)
            .is_err()
    );
    drop(core);
    let reopened = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    assert_eq!(
        reopened.finalized_draft(&wallet, &draft).unwrap().unwrap(),
        finalized
    );
}

#[test]
fn ordinary_sync_invalidation_does_not_release_recovery_holds_but_exact_observation_does() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, finalized) = submitted(&dir.path().join("source.sqlite"));
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let destination = dir.path().join("restored.sqlite");
    restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = engine::load(&tx, &wallet).unwrap();
        let original = loaded.state.clone();
        loaded.state.local_chain.blocks.remove(&1);
        loaded.wallet = bdk_wallet::Wallet::load()
            .load_wallet_no_persist(loaded.state.clone())
            .unwrap()
            .unwrap();
        crate::sync::invalidate_drafts(&tx, &wallet, &loaded.wallet).unwrap();
        assert_eq!(
            tx.query_row("SELECT count(*) FROM recovery_holds", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        loaded.state = original;
        loaded.wallet = bdk_wallet::Wallet::load()
            .load_wallet_no_persist(loaded.state.clone())
            .unwrap()
            .unwrap();
        crate::sync::invalidate_drafts(&tx, &wallet, &loaded.wallet).unwrap();
        engine::save(&tx, &wallet, &mut loaded).unwrap();
        tx.execute("UPDATE wallets SET synced_at=2", []).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(held(&core, &wallet), 2);
    let inputs = core.drafts(&wallet).unwrap()[0]
        .inputs
        .iter()
        .map(|i| i.outpoint.clone())
        .collect::<Vec<_>>();
    assert!(
        core.coins(&wallet)
            .unwrap()
            .iter()
            .filter(|c| inputs.contains(&c.outpoint))
            .all(|c| c.status == CoinStatus::Reserved)
    );
    let transaction: Transaction = deserialize(&finalized.transaction_bytes).unwrap();
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = engine::load(&tx, &wallet).unwrap();
        let mut update = bdk_wallet::Update::default();
        update
            .tx_update
            .txs
            .push(std::sync::Arc::new(transaction.clone()));
        update
            .tx_update
            .seen_ats
            .insert((transaction.compute_txid(), 10));
        loaded.wallet.apply_update(update).unwrap();
        crate::sync::invalidate_drafts(&tx, &wallet, &loaded.wallet).unwrap();
        engine::save(&tx, &wallet, &mut loaded).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(held(&core, &wallet), 0);
    assert!(!core.recovery_required(&wallet, &draft).unwrap());
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "observed");
    assert_eq!(
        core.broadcast_status(&wallet, &draft)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::Mempool
    );
}

#[test]
fn invalid_backup_or_existing_destination_never_resets_wallet_data() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("wallet.sqlite");
    let source = public_backup();
    assert!(
        restore_backup(
            &source,
            &destination,
            "incorrect public test password".into(),
            KEY.to_vec()
        )
        .is_err()
    );
    assert!(!destination.exists());
    assert!(restore_backup(&source, &destination, PASSWORD.into(), vec![]).is_err());
    assert!(!destination.exists());
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    core.import_wallet(
        "Retained",
        include_str!("../../../tests/fixtures/single-sig.txt"),
        Network::Signet,
    )
    .unwrap();
    assert!(restore_backup(&source, &destination, PASSWORD.into(), KEY.to_vec()).is_err());
    drop(core);
    let before = fs::read(&destination).unwrap();
    assert!(restore_backup(&source, &destination, PASSWORD.into(), KEY.to_vec()).is_err());
    assert_eq!(fs::read(&destination).unwrap(), before);
    assert_eq!(
        Core::open_protected(&destination, KEY.to_vec())
            .unwrap()
            .wallets()
            .unwrap()[0]
            .name,
        "Retained"
    );
}

#[test]
fn unsigned_recovered_drafts_are_invalidated_and_discard_does_not_clear_user_freezes() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, _) =
        crate::signing::tests::saved_ledger(&dir.path().join("source.sqlite"));
    let input = source.drafts(&wallet).unwrap()[0].inputs[0]
        .outpoint
        .clone();
    source.set_frozen(&wallet, &input, true).unwrap();
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let destination = dir.path().join("restored.sqlite");
    restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "invalidated");
    assert!(!core.recovery_required(&wallet, &draft).unwrap());
    assert_eq!(held(&core, &wallet), 0);
    assert_eq!(
        core.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM reservations", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    core.discard_draft(&wallet, &draft).unwrap();
    assert_eq!(
        core.coins(&wallet)
            .unwrap()
            .iter()
            .find(|c| c.outpoint == input)
            .unwrap()
            .status,
        CoinStatus::Frozen
    );
}

#[test]
fn recovered_signature_revalidation_failure_retains_suspension_and_holds() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, finalized) = submitted(&dir.path().join("source.sqlite"));
    {
        let db = source.lock().unwrap();
        let value: String = db
            .query_row("SELECT psbt FROM draft_signatures", [], |r| r.get(0))
            .unwrap();
        let mut signed = crate::signing::parse_response(value.as_bytes()).unwrap();
        // A published valid DER signature for another input, never a generated signature.
        let other = *signed.inputs[1].partial_sigs.values().next().unwrap();
        *signed.inputs[0].partial_sigs.values_mut().next().unwrap() = other;
        db.execute("UPDATE draft_signatures SET psbt=?1", [signed.to_string()])
            .unwrap();
    }
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let destination = dir.path().join("restored.sqlite");
    restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    core.lock()
        .unwrap()
        .execute("UPDATE wallets SET synced_at=2", [])
        .unwrap();
    let attempt = core
        .broadcast_status(&wallet, &draft)
        .unwrap()
        .unwrap()
        .attempt_id;
    assert!(
        core.resume_recovered_submission(&wallet, &draft, &finalized.txid, attempt, true)
            .is_err()
    );
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "invalidated");
    assert_eq!(held(&core, &wallet), 2);
    assert!(core.recovery_required(&wallet, &draft).unwrap());
    assert_eq!(
        core.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM reservations", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn malformed_archived_approval_is_refused_before_installation() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, _) = submitted(&dir.path().join("source.sqlite"));
    let mut review = source.drafts(&wallet).unwrap().remove(0);
    review.outputs[0].sats += 1;
    source
        .lock()
        .unwrap()
        .execute(
            "UPDATE drafts SET review_json=?1 WHERE id=?2",
            params![engine::json(&review).unwrap(), draft],
        )
        .unwrap();
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let before = fs::read(&backup).unwrap();
    let destination = dir.path().join("restored.sqlite");
    assert!(restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).is_err());
    assert!(!destination.exists());
    assert_eq!(fs::read(&backup).unwrap(), before);
}

#[test]
fn restore_failures_and_racing_destination_creation_preserve_input_and_existing_files() {
    for stop in [
        RestoreStep::Copied,
        RestoreStep::Verified,
        RestoreStep::Installed,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = public_backup();
        let before = fs::read(&source).unwrap();
        let destination = dir.path().join("restored.sqlite");
        assert!(
            restore_snapshot(&source, &destination, PASSWORD.into(), &KEY, |step| {
                if step == stop {
                    Err(Error::Storage)
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        if stop == RestoreStep::Installed {
            assert_eq!(
                Core::open_protected(&destination, KEY.to_vec())
                    .unwrap()
                    .wallets()
                    .unwrap()
                    .len(),
                1
            );
        } else {
            assert!(!destination.exists());
        }
        assert_eq!(fs::read(source).unwrap(), before);
    }
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("restored.sqlite");
    assert!(
        restore_snapshot(
            &public_backup(),
            &destination,
            PASSWORD.into(),
            &KEY,
            |step| {
                if step == RestoreStep::Verified {
                    fs::write(&destination, b"retained document").unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(fs::read(destination).unwrap(), b"retained document");
}

#[test]
#[ignore = "child fixture invoked by kill_during_restore_never_installs_a_partial_wallet"]
fn restore_child() {
    use std::io::Write;
    let destination = std::env::var_os("TUNDRA_RESTORE_TEST_DEST").unwrap();
    let stop = std::env::var("TUNDRA_RESTORE_TEST_STOP").unwrap();
    restore_snapshot(
        &public_backup(),
        Path::new(&destination),
        PASSWORD.into(),
        &KEY,
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
fn kill_during_restore_never_installs_a_partial_wallet() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::Duration,
    };
    let before = fs::read(public_backup()).unwrap();
    for stop in [
        RestoreStep::Copied,
        RestoreStep::Verified,
        RestoreStep::Installed,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("restored.sqlite");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "backup::restore_tests::restore_child",
                "--nocapture",
            ])
            .env("TUNDRA_RESTORE_TEST_DEST", &destination)
            .env("TUNDRA_RESTORE_TEST_STOP", format!("{stop:?}"))
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
        if stop == RestoreStep::Installed {
            let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
            assert!(core.wallets().unwrap()[0].total_sats.is_none());
        } else {
            assert!(!destination.exists());
        }
        assert_eq!(fs::read(public_backup()).unwrap(), before);
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
        restore_backup(
            public_backup(),
            dir.path().join("retry.sqlite"),
            PASSWORD.into(),
            KEY.to_vec(),
        )
        .unwrap();
    }
}

#[test]
fn hostile_snapshot_descriptors_are_refused_before_restoring_a_wallet() {
    let dir = tempfile::tempdir().unwrap();
    for (index, descriptor) in [
        "raw(deadbeef)".into(),
        format!("wpkh({})", "0".repeat(32_768)),
    ]
    .into_iter()
    .enumerate()
    {
        let source = dir.path().join(format!("hostile-{index}.tundra"));
        fs::copy(public_backup(), &source).unwrap();
        let db = Connection::open(&source).unwrap();
        storage::backend(&db).unwrap();
        db.pragma_update(None, "key", password_key(PASSWORD.into()).unwrap().as_str())
            .unwrap();
        profile(&db, None).unwrap();
        let raw: String = db
            .query_row("SELECT state_json FROM wallets", [], |r| r.get(0))
            .unwrap();
        let mut state: serde_json::Value = serde_json::from_str(&raw).unwrap();
        state["descriptor"] = serde_json::Value::String(descriptor);
        db.execute("UPDATE wallets SET state_json=?1", [state.to_string()])
            .unwrap();
        drop(db);
        let before = fs::read(&source).unwrap();
        assert!(inspect_backup(&source, PASSWORD.into()).is_err());
        let destination = dir.path().join(format!("refused-{index}.sqlite"));
        assert!(restore_backup(&source, &destination, PASSWORD.into(), KEY.to_vec()).is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read(source).unwrap(), before);
    }
}

#[test]
fn concurrent_recovery_reviews_resume_the_approval_once() {
    let dir = tempfile::tempdir().unwrap();
    let (source, wallet, draft, finalized) = submitted(&dir.path().join("source.sqlite"));
    let backup = dir.path().join("source.tundra");
    source.export_backup(&backup, PASSWORD.into()).unwrap();
    let destination = dir.path().join("restored.sqlite");
    restore_backup(&backup, &destination, PASSWORD.into(), KEY.to_vec()).unwrap();
    let first = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    first
        .lock()
        .unwrap()
        .execute("UPDATE wallets SET synced_at=2", [])
        .unwrap();
    let second = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    let attempt = first
        .broadcast_status(&wallet, &draft)
        .unwrap()
        .unwrap()
        .attempt_id;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = [first, second]
        .into_iter()
        .map(|core| {
            let wallet = wallet.clone();
            let draft = draft.clone();
            let txid = finalized.txid.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                core.resume_recovered_submission(&wallet, &draft, &txid, attempt, true)
            })
        })
        .collect::<Vec<_>>();
    let outcomes = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let core = Core::open_protected(&destination, KEY.to_vec()).unwrap();
    assert_eq!(
        core.finalized_draft(&wallet, &draft).unwrap().unwrap(),
        finalized
    );
    assert_eq!(held(&core, &wallet), 0);
    assert_eq!(
        core.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM reservations", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        core.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM broadcast_attempts", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
}
