use super::*;
const SINGLE: &str = include_str!("../../../../tests/fixtures/single-sig.txt");
fn setup(path: &std::path::Path) -> (Core, String) {
    let core = Core::open(path).unwrap();
    let wallet = core
        .import_wallet(
            "Private label never sent to hardware",
            SINGLE,
            crate::Network::Signet,
        )
        .unwrap();
    (core, wallet.id)
}
fn answer(session: &mut UsbSession, data: &[u8]) -> Result<Update> {
    let step = session.progress().step;
    let mut payload = ((data.len() + 2) as u16).to_be_bytes().to_vec();
    payload.extend(data);
    payload.extend([0x90, 0]);
    let mut update = session.progress();
    for (sequence, bytes) in payload.chunks(59).enumerate() {
        let mut report = [0; 64];
        report[..3].copy_from_slice(&[1, 1, 5]);
        report[3..5].copy_from_slice(&(sequence as u16).to_be_bytes());
        report[5..5 + bytes.len()].copy_from_slice(bytes);
        update = session.receive(step, &report)?;
    }
    Ok(update)
}
fn identify(session: &mut UsbSession, core: &Core, wallet: &str) {
    let context = context(&core.lock().unwrap(), wallet).unwrap();
    let mut info = vec![1, 12];
    info.extend(b"Bitcoin Test");
    info.push(5);
    info.extend(b"2.4.1");
    info.extend([1, 0]);
    answer(session, &info).unwrap();
    answer(session, context.accounts[0].fingerprint.as_bytes()).unwrap();
    answer(session, context.accounts[0].xpub.to_string().as_bytes()).unwrap();
}
fn register(core: &Core, wallet: &str) {
    let mut session = core.prepare_usb(wallet, Operation::RegisterPolicy).unwrap();
    identify(&mut session, core, wallet);
    let mut response = session.policy_id.to_vec();
    response.extend([7; 32]);
    assert_eq!(
        answer(&mut session, &response).unwrap().state,
        State::Complete
    );
}
#[test]
fn registration_survives_restart_and_enables_only_matching_issued_address_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let (core, wallet) = setup(&path);
    assert!(
        core.prepare_usb(&wallet, Operation::VerifyReceive { index: 0 })
            .is_err()
    );
    let address = core.receive_address(&wallet).unwrap();
    assert!(!address.hardware_verified);
    register(&core, &wallet);
    assert_eq!(
        context(&core.lock().unwrap(), &wallet).unwrap().policy.name,
        "Tundra"
    );
    drop(core);
    let core = Core::open(&path).unwrap();
    let mut session = core
        .prepare_usb(
            &wallet,
            Operation::VerifyReceive {
                index: address.index,
            },
        )
        .unwrap();
    identify(&mut session, &core, &wallet);
    assert_eq!(
        answer(&mut session, address.address.as_bytes())
            .unwrap()
            .state,
        State::Complete
    );
    assert!(
        core.prepare_usb(
            &wallet,
            Operation::VerifyReceive {
                index: address.index + 1
            }
        )
        .is_err()
    );
    assert!(!crate::hardware::capabilities(crate::hardware::Transport::Usb).available);
    assert!(matches!(
        core.prepare_usb(
            &wallet,
            Operation::SignDraft {
                draft_id: "anything".into()
            }
        ),
        Err(Error::Unavailable("hardware signing"))
    ));
}
#[test]
fn failed_persistence_does_not_publish_registration_success() {
    let dir = tempfile::tempdir().unwrap();
    let (core, wallet) = setup(&dir.path().join("wallet.sqlite"));
    let mut session = core
        .prepare_usb(&wallet, Operation::RegisterPolicy)
        .unwrap();
    identify(&mut session, &core, &wallet);
    core.lock().unwrap().execute_batch("CREATE TRIGGER reject_registration BEFORE INSERT ON hardware_registrations BEGIN SELECT RAISE(ABORT,'test rollback'); END;").unwrap();
    let mut response = session.policy_id.to_vec();
    response.extend([7; 32]);
    assert!(matches!(
        answer(&mut session, &response),
        Err(Error::Storage)
    ));
    assert_eq!(session.progress().state, State::Failed);
    assert!(
        context(&core.lock().unwrap(), &wallet)
            .unwrap()
            .registrations
            .is_empty()
    );
}
#[test]
fn cancelled_or_deleted_wallet_cannot_accept_a_late_registration() {
    for cancel in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (core, wallet) = setup(&dir.path().join("wallet.sqlite"));
        let mut session = core
            .prepare_usb(&wallet, Operation::RegisterPolicy)
            .unwrap();
        identify(&mut session, &core, &wallet);
        let mut response = session.policy_id.to_vec();
        response.extend([7; 32]);
        if cancel {
            session.cancel();
        } else {
            core.lock()
                .unwrap()
                .execute("DELETE FROM wallets WHERE id=?1", [&wallet])
                .unwrap();
        }
        assert!(answer(&mut session, &response).is_err());
        let count: u32 = core
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM hardware_registrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
#[test]
fn schema_four_migration_and_wallet_isolation_preserve_private_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let (core, wallet) = setup(&path);
    let address = core.receive_address(&wallet).unwrap();
    core.set_label(&wallet, "addr", &address.address, "Keep this label 🧊")
        .unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DROP TABLE hardware_registrations; PRAGMA user_version=4;")
        .unwrap();
    drop(core);
    let core = Core::open(&path).unwrap();
    register(&core, &wallet);
    let other = core
        .import_wallet(
            "Other",
            include_str!("../../../../tests/fixtures/two-of-three.txt"),
            crate::Network::Signet,
        )
        .unwrap();
    assert!(
        context(&core.lock().unwrap(), &other.id)
            .unwrap()
            .registrations
            .is_empty()
    );
    assert!(
        core.export_labels(&wallet)
            .unwrap()
            .contains("Keep this label 🧊")
    );
    assert_eq!(core.receive_address(&wallet).unwrap().index, 1);
    core.lock()
        .unwrap()
        .execute("DELETE FROM wallets WHERE id=?1", [&wallet])
        .unwrap();
    assert_eq!(core.wallets().unwrap().len(), 1);
    let count: u32 = core
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM hardware_registrations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}
