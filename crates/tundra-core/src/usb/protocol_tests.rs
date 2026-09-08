use super::*;
fn context() -> Context {
    let mut context = Context::from_preview(
        &crate::descriptor::preview_import(
            include_str!("../../../../tests/fixtures/single-sig.txt"),
            crate::Network::Signet,
        )
        .unwrap(),
    )
    .unwrap();
    // A test-only policy-authentication token, not a Bitcoin signing key.
    context
        .registrations
        .insert(context.accounts[0].fingerprint, [7; 32]);
    context
}
fn answer(session: &mut Session, data: &[u8], status: [u8; 2]) -> Result<Update> {
    let step = session.progress().step;
    let mut apdu = data.to_vec();
    apdu.extend(status);
    let mut bytes = (apdu.len() as u16).to_be_bytes().to_vec();
    bytes.extend(apdu);
    let mut result = session.progress();
    for (sequence, chunk) in bytes.chunks(59).enumerate() {
        let mut report = [0; 64];
        report[..3].copy_from_slice(&[1, 1, 5]);
        report[3..5].copy_from_slice(&(sequence as u16).to_be_bytes());
        report[5..5 + chunk.len()].copy_from_slice(chunk);
        result = session.receive(step, &report)?;
    }
    Ok(result)
}
fn info() -> Vec<u8> {
    let mut data = vec![1, 12];
    data.extend(b"Bitcoin Test");
    data.push(5);
    data.extend(b"2.4.1");
    data.extend([1, 0]);
    data
}
fn identify(session: &mut Session) {
    answer(session, &info(), [0x90, 0]).unwrap();
    answer(
        session,
        context().accounts[0].fingerprint.as_bytes(),
        [0x90, 0],
    )
    .unwrap();
    answer(
        session,
        context().accounts[0].xpub.to_string().as_bytes(),
        [0x90, 0],
    )
    .unwrap();
}
#[test]
fn handshake_checks_app_fingerprint_and_the_full_public_account() {
    let mut session = Session::start(context(), Intent::Inspect).unwrap();
    assert_eq!(&session.progress().packets[0][7..12], &[0xb0, 1, 0, 0, 0]);
    identify(&mut session);
    let state = session.progress();
    assert_eq!(state.state, State::Complete);
    assert_eq!(state.app_version.as_deref(), Some("2.4.1"));
    assert_eq!(
        state.fingerprint,
        Some(context().accounts[0].fingerprint.to_string())
    );
    assert!(matches!(
        session.take_outcome().unwrap(),
        Outcome::Inspected
    ));
    assert!(session.take_outcome().is_err());
}
#[test]
fn app_metadata_is_strict_and_errors_never_echo_payloads() {
    for case in 0..5 {
        let mut session = Session::start(context(), Intent::Inspect).unwrap();
        let mut data = info();
        match case {
            0 => data[2] = b'X',
            1 => data.push(0),
            2 => data[15] = b'\n',
            3 => data[1] = 255,
            4 => data[0] = 2,
            _ => unreachable!(),
        }
        let error = answer(&mut session, &data, [0x90, 0]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Invalid input: invalid or unexpected Ledger response"
        );
        assert_eq!(session.progress().state, State::Failed);
    }
    let mut session = Session::start(context(), Intent::Inspect).unwrap();
    assert!(answer(&mut session, &info(), [0x6e, 0]).is_err());
}
#[test]
fn wrong_fingerprint_extra_fingerprint_bytes_and_wrong_xpub_close_the_session() {
    for case in 0..3 {
        let mut session = Session::start(context(), Intent::Inspect).unwrap();
        answer(&mut session, &info(), [0x90, 0]).unwrap();
        let mut fingerprint = context().accounts[0].fingerprint.as_bytes().to_vec();
        match case {
            0 => fingerprint[0] ^= 1,
            1 => fingerprint.push(0),
            _ => {}
        }
        if case < 2 {
            assert!(answer(&mut session, &fingerprint, [0x90, 0]).is_err());
        } else {
            answer(&mut session, &fingerprint, [0x90, 0]).unwrap();
            let other = Context::from_preview(
                &crate::descriptor::preview_import(
                    include_str!("../../../../tests/fixtures/two-of-three.txt"),
                    crate::Network::Signet,
                )
                .unwrap(),
            )
            .unwrap();
            assert!(
                answer(
                    &mut session,
                    other.accounts[0].xpub.to_string().as_bytes(),
                    [0x90, 0]
                )
                .is_err()
            );
        }
        assert_eq!(session.progress().state, State::Failed);
    }
}
#[test]
fn registration_requires_the_exact_policy_id_and_has_no_refusal_success() {
    for case in 0..4 {
        let context = context();
        let mut payload = context.policy.id().unwrap().to_vec();
        payload.extend([7; 32]);
        let mut session = Session::start(context, Intent::Register).unwrap();
        identify(&mut session);
        match case {
            1 => payload[0] ^= 1,
            2 => payload.push(0),
            _ => {}
        }
        if case == 0 {
            assert_eq!(
                answer(&mut session, &payload, [0x90, 0]).unwrap().state,
                State::Complete
            );
            assert!(
                matches!(session.take_outcome().unwrap(), Outcome::Registered { hmac } if hmac == [7;32])
            );
        } else {
            assert!(
                answer(
                    &mut session,
                    &payload,
                    if case == 3 { [0x69, 0x85] } else { [0x90, 0] }
                )
                .is_err()
            );
            assert!(session.take_outcome().is_err());
        }
    }
}
#[test]
fn address_result_must_equal_the_expected_issued_address() {
    let address = crate::descriptor::preview_import(
        include_str!("../../../../tests/fixtures/single-sig.txt"),
        crate::Network::Signet,
    )
    .unwrap()
    .first_address;
    for matches in [true, false] {
        let mut session = Session::start(
            context(),
            Intent::Address {
                index: 0,
                expected: address.clone(),
            },
        )
        .unwrap();
        identify(&mut session);
        let result = answer(
            &mut session,
            if matches {
                address.as_bytes()
            } else {
                b"wrong address"
            },
            [0x90, 0],
        );
        if matches {
            result.unwrap();
            assert!(matches!(
                session.take_outcome().unwrap(),
                Outcome::AddressMatches
            ));
        } else {
            assert!(result.is_err());
        }
    }
}
#[test]
fn cancellation_expiry_stale_steps_and_response_budgets_are_terminal() {
    let mut cancelled = Session::start(context(), Intent::Inspect).unwrap();
    assert_eq!(cancelled.cancel().state, State::Cancelled);
    assert!(cancelled.take_outcome().is_err());
    assert!(answer(&mut cancelled, &info(), [0x90, 0]).is_err());
    let mut expired = Session::start(context(), Intent::Inspect).unwrap();
    expired.started = Instant::now() - LIFETIME - Duration::from_secs(1);
    assert_eq!(expired.progress().state, State::Failed);
    assert!(expired.progress().packets.is_empty());
    let mut stale = Session::start(context(), Intent::Inspect).unwrap();
    assert!(stale.receive(0, &[0; 64]).is_err());
    let mut full = Session::start(context(), Intent::Inspect).unwrap();
    full.bytes = MAX_RECEIVED_BYTES;
    assert!(answer(&mut full, &info(), [0x90, 0]).is_err());
    let mut count = Session::start(context(), Intent::Inspect).unwrap();
    count.update.step = MAX_EXCHANGES;
    assert!(answer(&mut count, &info(), [0x90, 0]).is_err());
}

#[test]
fn missing_registration_and_signing_failure_never_complete_a_signing_session() {
    let psbt = crate::signing::parse_response(include_bytes!(
        "../../../../tests/fixtures/hwi-signed-wpkh.psbt"
    ))
    .unwrap();
    let mut missing = context();
    missing.registrations.clear();
    let mut session = Session::start(
        missing,
        Intent::Sign {
            psbt: Box::new(psbt.clone()),
        },
    )
    .unwrap();
    answer(&mut session, &info(), [0x90, 0]).unwrap();
    answer(
        &mut session,
        context().accounts[0].fingerprint.as_bytes(),
        [0x90, 0],
    )
    .unwrap();
    assert!(
        answer(
            &mut session,
            context().accounts[0].xpub.to_string().as_bytes(),
            [0x90, 0]
        )
        .is_err()
    );
    for status in [[0x6e, 0], [0xb0, 8], [0x69, 0x82], [0x69, 0x85]] {
        let mut session = Session::start(
            context(),
            Intent::Sign {
                psbt: Box::new(psbt.clone()),
            },
        )
        .unwrap();
        identify(&mut session);
        assert!(answer(&mut session, &[], status).is_err());
        assert_eq!(session.progress().state, State::Failed);
        assert!(session.take_outcome().is_err());
    }
}

#[test]
fn bhwi_policy_derives_the_same_receive_and_change_scripts_as_bdk() {
    use bdk_wallet::miniscript::{Descriptor, descriptor::DescriptorPublicKey};
    use std::str::FromStr;
    for payload in [
        include_str!("../../../../tests/fixtures/single-sig.txt"),
        include_str!("../../../../tests/fixtures/two-of-three.txt"),
    ] {
        let preview = crate::descriptor::preview_import(payload, crate::Network::Signet).unwrap();
        let hardware = Context::from_preview(&preview).unwrap();
        let branches = hardware
            .policy
            .policy
            .into_descriptor()
            .unwrap()
            .into_single_descriptors()
            .unwrap();
        assert_eq!(branches.len(), 2);
        for (branch, expected) in branches
            .iter()
            .zip([preview.receive_descriptor, preview.change_descriptor])
        {
            let expected = Descriptor::<DescriptorPublicKey>::from_str(&expected).unwrap();
            for index in [0, 1, 17, 2_147_483_647] {
                let actual = branch
                    .derive_at_index(index)
                    .unwrap()
                    .derived_descriptor(&bhwi::bitcoin::secp256k1::Secp256k1::verification_only());
                let expected = expected
                    .at_derivation_index(index)
                    .unwrap()
                    .derived_descriptor(
                        &bdk_wallet::bitcoin::secp256k1::Secp256k1::verification_only(),
                    )
                    .unwrap();
                assert_eq!(actual.script_pubkey(), expected.script_pubkey());
            }
        }
    }
}

#[test]
fn merkle_continuations_return_only_known_policy_preimages() {
    use bdk_wallet::bitcoin::hashes::{Hash, sha256};
    let template = b"wpkh(@0/**)";
    let mut session = Session::start(context(), Intent::Register).unwrap();
    identify(&mut session);
    let mut request = vec![0x40, 0];
    request.extend(sha256::Hash::hash(template).to_byte_array());
    let update = answer(&mut session, &request, [0xe0, 0]).unwrap();
    assert_eq!(update.state, State::Waiting);
    // Ledger ContinueInterrupted: compact-size preimage length, returned length, bytes.
    let packet = &update.packets[0];
    assert_eq!(&packet[7..11], &[0xf8, 1, 0, 1]);
    assert_eq!(packet[11], (template.len() + 2) as u8);
    assert_eq!(
        &packet[12..14],
        &[template.len() as u8, template.len() as u8]
    );
    assert_eq!(&packet[14..14 + template.len()], template);
    request[2] ^= 1;
    assert!(answer(&mut session, &request, [0xe0, 0]).is_err());
    assert_eq!(session.progress().state, State::Failed);
}
