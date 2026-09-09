use super::public_fixtures::{hwi, ledger, public_multisig};
use super::*;
use bdk_wallet::bitcoin::{Amount, ScriptBuf, Sequence, Witness, absolute, psbt::raw};

#[test]
fn confirmed_public_two_of_three_signatures_verify_in_script_order() {
    let (approved, finalized, trusted) = public_multisig();
    let merged = merge_response(&approved, None, &finalized, &trusted).unwrap();
    assert!(
        signing_progress("public vector", &merged, &trusted)
            .unwrap()
            .complete
    );
    assert_eq!(
        progress(&merged, &trusted).unwrap()[0].required_signatures,
        2
    );
    assert_eq!(merged.inputs[0].partial_sigs.len(), 2);
    let mut first = merged.clone();
    let second_signature = first.inputs[0].partial_sigs.pop_last().unwrap();
    let first = merge_response(&approved, None, &first, &trusted).unwrap();
    assert!(
        !signing_progress("public vector", &first, &trusted)
            .unwrap()
            .complete
    );
    // Repeating the same response must never count a second key.
    let repeated = merge_response(&approved, Some(&first), &first, &trusted).unwrap();
    assert_eq!(repeated.inputs[0].partial_sigs.len(), 1);
    let mut second = approved.clone();
    second.inputs[0]
        .partial_sigs
        .insert(second_signature.0, second_signature.1);
    assert_eq!(
        merge_response(&approved, Some(&first), &second, &trusted).unwrap(),
        merged
    );
    let stack: Vec<Vec<u8>> = finalized.inputs[0]
        .final_script_witness
        .as_ref()
        .unwrap()
        .iter()
        .map(|x| x.to_vec())
        .collect();
    for case in 0..5 {
        let mut bad_stack = stack.clone();
        match case {
            0 => bad_stack.swap(1, 2),
            1 => bad_stack[2] = bad_stack[1].clone(),
            2 => bad_stack[0] = vec![1],
            3 => {
                bad_stack.remove(2);
            }
            4 => bad_stack[3][2] ^= 1,
            _ => unreachable!(),
        }
        let mut bad = finalized.clone();
        bad.inputs[0].final_script_witness = Some(Witness::from_slice(&bad_stack));
        assert!(
            merge_response(&approved, None, &bad, &trusted).is_err(),
            "multisig mutation {case}"
        );
    }
}

#[test]
fn high_s_and_wrong_public_key_signatures_are_rejected() {
    let (approved, mut signed, trusted) = hwi();
    let (key, signature) = signed.inputs[0].partial_sigs.pop_first().unwrap();
    let mut compact = signature.signature.serialize_compact();
    let order = bdk_wallet::bitcoin::secp256k1::constants::CURVE_ORDER;
    let mut borrow = 0i16;
    for index in (0..32).rev() {
        let difference = i16::from(order[index]) - i16::from(compact[32 + index]) - borrow;
        compact[32 + index] = difference.rem_euclid(256) as u8;
        borrow = i16::from(difference < 0);
    }
    let high = ecdsa::Signature {
        signature: bdk_wallet::bitcoin::secp256k1::ecdsa::Signature::from_compact(&compact)
            .unwrap(),
        ..signature
    };
    signed.inputs[0].partial_sigs.insert(key, high);
    assert!(merge_response(&approved, None, &signed, &trusted).is_err());
    let mut uncompressed = key;
    uncompressed.compressed = false;
    signed.inputs[0].partial_sigs.clear();
    signed.inputs[0]
        .partial_sigs
        .insert(uncompressed, signature);
    assert!(merge_response(&approved, None, &signed, &trusted).is_err());
}

#[test]
fn published_hwi_signature_verifies_and_duplicate_import_is_idempotent() {
    let (approved, signed, trusted) = hwi();
    let merged = merge_response(&approved, None, &signed, &trusted).unwrap();
    assert_eq!(progress(&merged, &trusted).unwrap()[0].valid_signatures, 1);
    assert!(
        signing_progress("fixture", &merged, &trusted)
            .unwrap()
            .complete
    );
    assert_eq!(
        merge_response(&approved, Some(&merged), &signed, &trusted).unwrap(),
        merged
    );
    assert_eq!(merged.unsigned_tx, approved.unsigned_tx);
    assert!(approved.inputs[0].partial_sigs.is_empty());
}

#[test]
fn published_ledger_signatures_are_counted_per_input_and_preserved_across_merges() {
    let (approved, signed, trusted) = ledger();
    let mut first = signed.clone();
    first.inputs[1].partial_sigs.clear();
    let first = merge_response(&approved, None, &first, &trusted).unwrap();
    let p = signing_progress("fixture", &first, &trusted).unwrap();
    assert!(!p.complete);
    assert_eq!(
        p.inputs
            .iter()
            .map(|i| i.valid_signatures)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    let mut second = signed.clone();
    second.inputs[0].partial_sigs.clear();
    let merged = merge_response(&approved, Some(&first), &second, &trusted).unwrap();
    assert!(
        signing_progress("fixture", &merged, &trusted)
            .unwrap()
            .complete
    );
    assert_eq!(merged, signed);
    // A signature from the other input cannot be counted for this input.
    second.inputs[0].partial_sigs = second.inputs[1].partial_sigs.clone();
    assert!(merge_response(&approved, Some(&first), &second, &trusted).is_err());
}

#[test]
fn stripped_response_metadata_is_restored_only_from_approved_data() {
    let (approved, mut signed, trusted) = hwi();
    let sigs = signed.inputs[0].partial_sigs.clone();
    signed.inputs[0] = psbt::Input {
        partial_sigs: sigs,
        ..Default::default()
    };
    signed.outputs.fill(psbt::Output::default());
    signed.xpub.clear();
    let merged = merge_response(&approved, None, &signed, &trusted).unwrap();
    assert_eq!(
        merged.inputs[0].witness_utxo,
        approved.inputs[0].witness_utxo
    );
    assert_eq!(
        merged.inputs[0].bip32_derivation,
        approved.inputs[0].bip32_derivation
    );
    assert_eq!(merged.outputs, approved.outputs);
}

#[test]
fn finalized_single_sig_response_is_verified_then_normalized_to_partial_signatures() {
    let (approved, mut signed, trusted) = hwi();
    let (key, sig) = signed.inputs[0].partial_sigs.pop_first().unwrap();
    signed.inputs[0].final_script_witness =
        Some(Witness::from_slice(&[sig.to_vec(), key.to_bytes()]));
    let merged = merge_response(&approved, None, &signed, &trusted).unwrap();
    assert_eq!(merged.inputs[0].partial_sigs.get(&key), Some(&sig));
    assert!(merged.inputs[0].final_script_witness.is_none());
    let mut bad = signed.clone();
    bad.inputs[0].partial_sigs.insert(key, sig);
    assert!(merge_response(&approved, None, &bad, &trusted).is_err());
    for witness in [
        Witness::new(),
        Witness::from_slice(&[sig.to_vec()]),
        Witness::from_slice(&[key.to_bytes(), sig.to_vec()]),
        Witness::from_slice(&[sig.to_vec(), key.to_bytes(), vec![]]),
    ] {
        bad = signed.clone();
        bad.inputs[0].final_script_witness = Some(witness);
        assert!(merge_response(&approved, None, &bad, &trusted).is_err());
    }
}

#[test]
fn changed_transaction_and_policy_fields_are_rejected_atomically() {
    let (approved, signed, trusted) = hwi();
    let mutations: Vec<Psbt> = (0..21)
        .map(|case| {
            let mut bad = signed.clone();
            let input = &mut bad.inputs[0];
            match case {
                0 => bad.unsigned_tx.output[0].value += Amount::ONE_SAT,
                1 => bad.unsigned_tx.output[0].script_pubkey = ScriptBuf::new(),
                2 => bad.unsigned_tx.input[0].previous_output.vout += 1,
                3 => bad.unsigned_tx.input[0].sequence = Sequence::ZERO,
                4 => bad.unsigned_tx.lock_time = absolute::LockTime::from_consensus(42),
                5 => bad.unsigned_tx.version.0 += 1,
                6 => bad.unsigned_tx.output.reverse(),
                7 => input.witness_utxo.as_mut().unwrap().value += Amount::ONE_SAT,
                8 => input.witness_utxo.as_mut().unwrap().script_pubkey = ScriptBuf::new(),
                9 => input.sighash_type = Some(EcdsaSighashType::None.into()),
                10 => {
                    input.partial_sigs.values_mut().next().unwrap().sighash_type =
                        EcdsaSighashType::AllPlusAnyoneCanPay
                }
                11 => input.witness_script = Some(ScriptBuf::new()),
                12 => input.redeem_script = Some(ScriptBuf::new()),
                13 => input.final_script_sig = Some(ScriptBuf::from_bytes(vec![1, 1])),
                14 => input.bip32_derivation.values_mut().next().unwrap().0 = Default::default(),
                15 => {
                    input.unknown.insert(
                        raw::Key {
                            type_value: 0x80,
                            key: vec![],
                        },
                        vec![1],
                    );
                }
                16 => {
                    bad.unknown.insert(
                        raw::Key {
                            type_value: 0x80,
                            key: vec![],
                        },
                        vec![1],
                    );
                }
                17 => {
                    bad.outputs[0].unknown.insert(
                        raw::Key {
                            type_value: 0x80,
                            key: vec![],
                        },
                        vec![1],
                    );
                }
                18 => bad.version = 2,
                19 => {
                    bad.inputs.clear();
                }
                20 => {
                    bad.outputs.clear();
                }
                _ => unreachable!(),
            }
            bad
        })
        .collect();
    for (case, bad) in mutations.into_iter().enumerate() {
        assert!(
            merge_response(&approved, None, &bad, &trusted).is_err(),
            "mutation {case}"
        );
        assert!(approved.inputs[0].partial_sigs.is_empty());
    }
}

#[test]
fn signature_must_verify_against_independent_prevout_and_policy() {
    let (approved, signed, mut trusted) = hwi();
    trusted[0].prevout.value += Amount::ONE_SAT;
    let mut stripped = signed.clone();
    stripped.inputs[0].witness_utxo = None;
    let mut changed = approved.clone();
    changed.inputs[0].witness_utxo = None;
    assert!(merge_response(&changed, None, &stripped, &trusted).is_err());
    let (_, other, _) = ledger();
    trusted[0].descriptor =
        Descriptor::new_wpkh(*other.inputs[0].partial_sigs.keys().next().unwrap()).unwrap();
    assert!(merge_response(&changed, None, &stripped, &trusted).is_err());
}

#[test]
fn psbt_parser_rejects_trailing_maps_bytes_duplicates_and_truncations() {
    let (approved, _, _) = hwi();
    let binary = approved.serialize();
    assert_eq!(parse_response(&binary).unwrap(), approved);
    assert_eq!(
        parse_response(format!(" \n{approved}\n").as_bytes()).unwrap(),
        approved
    );
    for end in 0..binary.len() {
        assert!(parse_response(&binary[..end]).is_err());
    }
    for suffix in [&[0][..], b"garbage", &binary] {
        let mut bad = binary.clone();
        bad.extend_from_slice(suffix);
        assert!(parse_response(&bad).is_err());
        assert!(parse_response(BASE64_STANDARD.encode(bad).as_bytes()).is_err());
    }
    let mut bad = binary.clone();
    // Nonminimal CompactSize global key length.
    bad.splice(5..6, [253, 1, 0]);
    assert!(parse_response(&bad).is_err());
    assert!(parse_response(&vec![b'A'; MAX_PSBT_FILE_BYTES + 1]).is_err());
    let mut bad = binary;
    let mut cursor = &bad[5..];
    let key_len = count(&mut cursor).unwrap();
    take(&mut cursor, key_len).unwrap();
    let len = count(&mut cursor).unwrap();
    take(&mut cursor, len).unwrap();
    let end = bad.len() - cursor.len();
    let duplicate = bad[5..end].to_vec();
    bad.splice(end..end, duplicate);
    assert!(parse_response(&bad).is_err());
}

#[test]
fn transaction_vector_limits_are_checked_before_typed_parsing() {
    let (mut approved, _, _) = hwi();
    approved.unsigned_tx.input = vec![approved.unsigned_tx.input[0].clone(); MAX_INPUTS + 1];
    approved.inputs = vec![approved.inputs[0].clone(); MAX_INPUTS + 1];
    assert!(parse_response(&approved.serialize()).is_err());
    let (mut approved, _, _) = hwi();
    approved.unsigned_tx.output = vec![approved.unsigned_tx.output[0].clone(); MAX_OUTPUTS + 1];
    approved.outputs = vec![psbt::Output::default(); MAX_OUTPUTS + 1];
    assert!(parse_response(&approved.serialize()).is_err());
}

// Build a test-only wallet snapshot from the published Ledger public account and funding
// transactions. The saved review is the fixture's already approved transaction; no keys
// are generated/imported, and this injection is never exposed by the application API.
pub(crate) fn saved_ledger(path: &std::path::Path) -> (crate::Core, String, String, Psbt) {
    use crate::{
        engine::{json, load, save},
        *,
    };
    use bdk_wallet::{
        KeychainKind,
        bitcoin::{
            self, Block, CompactTarget, TxMerkleNode, block,
            hashes::{Hash, sha256},
        },
    };
    use rusqlite::params;
    let (approved, signed, _) = ledger();
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ledger-wpkh-two-inputs.json"
    ))
    .unwrap();
    let descriptor = format!("wpkh({}/<0;1>/*)", data["keys_info"][0].as_str().unwrap());
    let checksum = bdk_wallet::descriptor::checksum::calc_checksum(&descriptor).unwrap();
    let core = Core::open(path).unwrap();
    let wallet_id = core
        .import_wallet(
            "Published public signing fixture",
            &format!("{descriptor}#{checksum}"),
            Network::Signet,
        )
        .unwrap()
        .id;
    let draft_id = sha256::Hash::hash(
        format!("{wallet_id}:{}", approved.unsigned_tx.compute_txid()).as_bytes(),
    )
    .to_string();
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = load(&tx, &wallet_id).unwrap();
        // Ensure all publicly specified derivations are known before applying the fixture funding.
        let _ = loaded
            .wallet
            .reveal_addresses_to(KeychainKind::External, 20)
            .collect::<Vec<_>>();
        let _ = loaded
            .wallet
            .reveal_addresses_to(KeychainKind::Internal, 20)
            .collect::<Vec<_>>();
        let funding: BTreeMap<_, _> = approved
            .inputs
            .iter()
            .map(|i| {
                let tx = i.non_witness_utxo.clone().unwrap();
                (tx.compute_txid(), tx)
            })
            .collect();
        let mut block = Block {
            header: block::Header {
                version: block::Version::ONE,
                prev_blockhash: bitcoin::blockdata::constants::genesis_block(
                    bitcoin::Network::Signet,
                )
                .block_hash(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1,
                bits: CompactTarget::from_consensus(0x207fffff),
                nonce: 0,
            },
            txdata: funding.into_values().collect(),
        };
        block.header.merkle_root = block.compute_merkle_root().unwrap();
        loaded.wallet.apply_block(&block, 1).unwrap();
        let inputs: Vec<_> = approved
            .unsigned_tx
            .input
            .iter()
            .zip(&approved.inputs)
            .map(|(i, p)| ReviewedInput {
                outpoint: i.previous_output.to_string(),
                sats: p.witness_utxo.as_ref().unwrap().value.to_sat(),
                label: "Public vector".into(),
            })
            .collect();
        let outputs: Vec<_> = approved
            .unsigned_tx
            .output
            .iter()
            .map(|o| ReviewedOutput {
                address: bitcoin::Address::from_script(&o.script_pubkey, bitcoin::Network::Signet)
                    .unwrap()
                    .to_string(),
                sats: o.value.to_sat(),
                is_change: loaded.wallet.is_mine(o.script_pubkey.clone()),
                is_mine: loaded.wallet.is_mine(o.script_pubkey.clone()),
            })
            .collect();
        let fee_sats = inputs.iter().map(|i| i.sats).sum::<u64>()
            - outputs.iter().map(|o| o.sats).sum::<u64>();
        let review = DraftReview {
            id: draft_id.clone(),
            wallet_id: wallet_id.clone(),
            inputs,
            outputs,
            fee_sats,
            fee_sat_per_kwu: None,
            label: "Published vector".into(),
            is_consolidation: false,
            state: "unsigned".into(),
        };
        tx.execute("INSERT INTO drafts(id,wallet_id,psbt,review_json,label,created_at) VALUES(?1,?2,?3,?4,'Published vector',1)", params![draft_id, wallet_id, approved.to_string(), json(&review).unwrap()]).unwrap();
        for input in &review.inputs {
            tx.execute(
                "INSERT INTO reservations(wallet_id,outpoint,draft_id) VALUES(?1,?2,?3)",
                params![wallet_id, input.outpoint, draft_id],
            )
            .unwrap();
        }
        save(&tx, &wallet_id, &mut loaded).unwrap();
        tx.execute("UPDATE wallets SET synced_at=1 WHERE id=?1", [&wallet_id])
            .unwrap();
        tx.commit().unwrap();
    }
    (core, wallet_id, draft_id, signed)
}

#[test]
fn validated_signatures_survive_restart_without_changing_approval_or_reservations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let (core, wallet, draft, mut signed) = saved_ledger(&path);
    let original = core.export_unsigned_psbt(&wallet, &draft).unwrap();
    let all_signed = signed.clone();
    signed.inputs[1].partial_sigs.clear();
    assert!(
        !core
            .accept_signed_psbt(&wallet, &draft, &signed.serialize())
            .unwrap()
            .complete
    );
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "partially_signed");
    assert!(core.export_unsigned_psbt(&wallet, &draft).is_err());
    drop(core);
    let core = crate::Core::open(&path).unwrap();
    let progress = core.signing_progress(&wallet, &draft).unwrap();
    assert_eq!(progress.inputs[0].valid_signatures, 1);
    assert_eq!(progress.inputs[1].valid_signatures, 0);
    assert!(
        core.accept_signed_psbt(&wallet, &draft, all_signed.to_string().as_bytes())
            .unwrap()
            .complete
    );
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "signed");
    assert_eq!(
        core.coins(&wallet)
            .unwrap()
            .iter()
            .filter(|c| c.status == crate::CoinStatus::Reserved)
            .count(),
        2
    );
    let stored: String = core
        .lock()
        .unwrap()
        .query_row("SELECT psbt FROM drafts WHERE id=?1", [&draft], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(stored, original);
    assert!(core.signing_progress("other-wallet", &draft).is_err());
    assert!(core.signing_progress(&wallet, "other-draft").is_err());
    core.discard_draft(&wallet, &draft).unwrap();
    let count: u32 = core
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM draft_signatures", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn failed_response_and_failed_database_commit_preserve_prior_signatures() {
    let dir = tempfile::tempdir().unwrap();
    let (core, wallet, draft, signed) = saved_ledger(&dir.path().join("wallet.sqlite"));
    assert!(
        core.accept_signed_psbt(
            &wallet,
            &draft,
            core.export_unsigned_psbt(&wallet, &draft)
                .unwrap()
                .as_bytes()
        )
        .is_err()
    );
    let mut partial = signed.clone();
    partial.inputs[1].partial_sigs.clear();
    core.accept_signed_psbt(&wallet, &draft, &partial.serialize())
        .unwrap();
    let before = core.export_signing_psbt(&wallet, &draft).unwrap();
    let mut bad = signed.clone();
    bad.unsigned_tx.output[0].value += Amount::ONE_SAT;
    assert!(
        core.accept_signed_psbt(&wallet, &draft, &bad.serialize())
            .is_err()
    );
    assert_eq!(core.export_signing_psbt(&wallet, &draft).unwrap(), before);
    core.lock().unwrap().execute_batch("CREATE TRIGGER fail_review BEFORE UPDATE ON drafts BEGIN SELECT RAISE(ABORT,'test rollback'); END;").unwrap();
    assert!(matches!(
        core.accept_signed_psbt(&wallet, &draft, &signed.serialize()),
        Err(Error::Storage)
    ));
    assert_eq!(core.export_signing_psbt(&wallet, &draft).unwrap(), before);
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "partially_signed");
}

#[test]
fn freeze_invalidation_missing_reservation_and_tampered_storage_block_signature_use() {
    let dir = tempfile::tempdir().unwrap();
    let (core, wallet, draft, signed) = saved_ledger(&dir.path().join("wallet.sqlite"));
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    let outpoint = signed.unsigned_tx.input[0].previous_output.to_string();
    core.set_frozen(&wallet, &outpoint, true).unwrap();
    assert!(core.signing_progress(&wallet, &draft).is_err());
    assert!(core.export_signing_psbt(&wallet, &draft).is_err());
    core.set_frozen(&wallet, &outpoint, false).unwrap();
    assert!(core.signing_progress(&wallet, &draft).unwrap().complete);
    let mut tampered = signed.clone();
    tampered.inputs[0].witness_utxo.as_mut().unwrap().value += Amount::ONE_SAT;
    core.lock()
        .unwrap()
        .execute(
            "UPDATE draft_signatures SET psbt=?1",
            [tampered.to_string()],
        )
        .unwrap();
    assert!(core.signing_progress(&wallet, &draft).is_err());
    core.lock()
        .unwrap()
        .execute("UPDATE draft_signatures SET psbt=?1", [signed.to_string()])
        .unwrap();
    core.lock()
        .unwrap()
        .execute("DELETE FROM reservations WHERE outpoint=?1", [&outpoint])
        .unwrap();
    assert!(core.export_signing_psbt(&wallet, &draft).is_err());
}

#[test]
fn schema_two_migration_preserves_approved_draft_and_reservations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let (core, wallet, draft, signed) = saved_ledger(&path);
    let original = core.export_unsigned_psbt(&wallet, &draft).unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DROP TABLE draft_signatures; PRAGMA user_version=2;")
        .unwrap();
    drop(core);
    let core = crate::Core::open(&path).unwrap();
    assert_eq!(
        core.export_unsigned_psbt(&wallet, &draft).unwrap(),
        original
    );
    assert!(
        core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
            .unwrap()
            .complete
    );
}

#[test]
fn sync_invalidation_permanently_blocks_previously_valid_signatures() {
    for finalize in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (core, wallet, draft, signed) = saved_ledger(&dir.path().join("wallet.sqlite"));
        core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
            .unwrap();
        if finalize {
            core.finalize_draft(&wallet, &draft).unwrap();
        }
        {
            let db = core.lock().unwrap();
            let loaded = crate::engine::load(&db, &wallet).unwrap();
            let empty = bdk_wallet::Wallet::create(
                loaded
                    .wallet
                    .public_descriptor(bdk_wallet::KeychainKind::External)
                    .to_string(),
                loaded
                    .wallet
                    .public_descriptor(bdk_wallet::KeychainKind::Internal)
                    .to_string(),
            )
            .network(bdk_wallet::bitcoin::Network::Signet)
            .create_wallet_no_persist()
            .unwrap();
            crate::sync::invalidate_drafts(&db, &wallet, &empty).unwrap();
        }
        assert_eq!(core.drafts(&wallet).unwrap()[0].state, "invalidated");
        assert!(core.export_signing_psbt(&wallet, &draft).is_err());
        assert!(
            core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
                .is_err()
        );
        assert!(core.signing_progress(&wallet, &draft).is_err());
        assert!(core.finalized_draft(&wallet, &draft).is_err());
        assert!(core.finalize_draft(&wallet, &draft).is_err());
    }
}

#[test]
fn finalization_matches_the_confirmed_multisig_transaction_byte_for_byte() {
    use bdk_wallet::bitcoin::consensus::encode::deserialize_hex;
    let (approved, response, trusted) = public_multisig();
    let aggregate = merge_response(&approved, None, &response, &trusted).unwrap();
    let transaction = finalize_transaction(&approved, Some(&aggregate), &trusted).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/public-signed-multisig.json"
    ))
    .unwrap();
    let expected: Transaction =
        deserialize_hex(fixture["raw_transaction"].as_str().unwrap()).unwrap();
    assert_eq!(transaction, expected);
    assert_eq!(
        transaction.compute_txid(),
        approved.unsigned_tx.compute_txid()
    );
    assert_ne!(
        transaction.compute_wtxid().to_string(),
        transaction.compute_txid().to_string()
    );
    let mut incomplete = aggregate;
    incomplete.inputs[0].partial_sigs.pop_last();
    assert!(finalize_transaction(&approved, Some(&incomplete), &trusted).is_err());
}

#[test]
fn finalization_requires_all_inputs_and_preserves_public_single_sig_witnesses() {
    for (approved, signed, trusted) in [hwi(), ledger()] {
        let transaction = finalize_transaction(&approved, Some(&signed), &trusted).unwrap();
        assert_eq!(transaction.output, approved.unsigned_tx.output);
        for (index, input) in transaction.input.iter().enumerate() {
            let (key, signature) = signed.inputs[index].partial_sigs.first_key_value().unwrap();
            assert_eq!(
                input.witness,
                Witness::from_slice(&[signature.to_vec(), key.to_bytes()])
            );
            assert_eq!(
                input.previous_output,
                approved.unsigned_tx.input[index].previous_output
            );
            assert!(input.script_sig.is_empty());
            let mut partial = signed.clone();
            partial.inputs[index].partial_sigs.clear();
            assert!(finalize_transaction(&approved, Some(&partial), &trusted).is_err());
        }
        let mut modified = signed.clone();
        modified.unsigned_tx.output[0].value += Amount::ONE_SAT;
        assert!(finalize_transaction(&approved, Some(&modified), &trusted).is_err());
    }
}

#[test]
fn finalized_bytes_survive_restart_and_freeze_signature_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallet.sqlite");
    let (core, wallet, draft, signed) = saved_ledger(&path);
    let original = core.export_unsigned_psbt(&wallet, &draft).unwrap();
    assert!(core.finalized_draft(&wallet, &draft).unwrap().is_none());
    assert!(core.finalize_draft(&wallet, &draft).is_err());
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    let finalized = core.finalize_draft(&wallet, &draft).unwrap();
    assert_eq!(core.finalize_draft(&wallet, &draft).unwrap(), finalized);
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "finalized");
    assert_eq!(
        finalized.txid,
        signed.unsigned_tx.compute_txid().to_string()
    );
    assert_eq!(finalized.vsize, finalized.weight_wu.div_ceil(4));
    assert_eq!(
        finalized.fee_sats,
        core.drafts(&wallet).unwrap()[0].fee_sats
    );
    assert_eq!(
        core.coins(&wallet)
            .unwrap()
            .into_iter()
            .filter(|coin| coin.status == crate::CoinStatus::Reserved)
            .map(|coin| coin.outpoint)
            .collect::<BTreeSet<_>>(),
        signed
            .unsigned_tx
            .input
            .iter()
            .map(|input| input.previous_output.to_string())
            .collect::<BTreeSet<_>>()
    );
    assert!(
        core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
            .is_err()
    );
    let saved: String = core
        .lock()
        .unwrap()
        .query_row("SELECT psbt FROM drafts WHERE id=?1", [&draft], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(saved, original);
    drop(core);
    let core = crate::Core::open(&path).unwrap();
    assert_eq!(
        core.finalized_draft(&wallet, &draft).unwrap(),
        Some(finalized)
    );
    assert!(core.signing_progress(&wallet, &draft).unwrap().complete);
    assert!(core.finalized_draft("other-wallet", &draft).is_err());
    core.discard_draft(&wallet, &draft).unwrap();
    let count: u32 = core
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM finalized_drafts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn finalization_rollback_preserves_signed_state_and_reservations() {
    let directory = tempfile::tempdir().unwrap();
    let (core, wallet, draft, signed) = saved_ledger(&directory.path().join("wallet.sqlite"));
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    core.lock().unwrap().execute_batch("CREATE TRIGGER fail_final BEFORE UPDATE ON drafts BEGIN SELECT RAISE(ABORT,'rollback test'); END;").unwrap();
    assert!(matches!(
        core.finalize_draft(&wallet, &draft),
        Err(Error::Storage)
    ));
    assert!(core.finalized_draft(&wallet, &draft).unwrap().is_none());
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "signed");
    assert!(core.signing_progress(&wallet, &draft).unwrap().complete);
    assert_eq!(
        core.coins(&wallet)
            .unwrap()
            .into_iter()
            .filter(|coin| coin.status == crate::CoinStatus::Reserved)
            .map(|coin| coin.outpoint)
            .collect::<BTreeSet<_>>(),
        signed
            .unsigned_tx
            .input
            .iter()
            .map(|input| input.previous_output.to_string())
            .collect::<BTreeSet<_>>()
    );
    core.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_final")
        .unwrap();
    assert!(core.finalize_draft(&wallet, &draft).is_ok());
}

#[test]
fn finalized_review_rejects_changed_bytes_signatures_and_missing_reservations() {
    let directory = tempfile::tempdir().unwrap();
    let (core, wallet, draft, signed) = saved_ledger(&directory.path().join("wallet.sqlite"));
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    let finalized = core.finalize_draft(&wallet, &draft).unwrap();
    let mut bytes = finalized.transaction_bytes.clone();
    bytes[0] ^= 1;
    core.lock()
        .unwrap()
        .execute("UPDATE finalized_drafts SET transaction_bytes=?1", [bytes])
        .unwrap();
    assert!(matches!(
        core.finalized_draft(&wallet, &draft),
        Err(Error::CorruptState)
    ));
    assert!(core.finalize_draft(&wallet, &draft).is_err());
    core.lock()
        .unwrap()
        .execute(
            "UPDATE finalized_drafts SET transaction_bytes=?1",
            [finalized.transaction_bytes],
        )
        .unwrap();
    let mut partial = signed.clone();
    partial.inputs[0].partial_sigs.clear();
    core.lock()
        .unwrap()
        .execute("UPDATE draft_signatures SET psbt=?1", [partial.to_string()])
        .unwrap();
    assert!(core.finalized_draft(&wallet, &draft).is_err());
    core.lock()
        .unwrap()
        .execute("UPDATE draft_signatures SET psbt=?1", [signed.to_string()])
        .unwrap();
    let outpoint = signed.unsigned_tx.input[0].previous_output.to_string();
    core.set_frozen(&wallet, &outpoint, true).unwrap();
    assert!(core.finalized_draft(&wallet, &draft).is_err());
    core.set_frozen(&wallet, &outpoint, false).unwrap();
    assert!(core.finalized_draft(&wallet, &draft).unwrap().is_some());
    core.lock()
        .unwrap()
        .execute("DELETE FROM reservations WHERE outpoint=?1", [&outpoint])
        .unwrap();
    assert!(core.finalized_draft(&wallet, &draft).is_err());
}

#[test]
fn schema_three_migration_preserves_signatures_before_finalization() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallet.sqlite");
    let (core, wallet, draft, signed) = saved_ledger(&path);
    core.accept_signed_psbt(&wallet, &draft, &signed.serialize())
        .unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DROP TABLE finalized_drafts; PRAGMA user_version=3;")
        .unwrap();
    drop(core);
    let core = crate::Core::open(&path).unwrap();
    assert!(core.signing_progress(&wallet, &draft).unwrap().complete);
    assert!(core.finalized_draft(&wallet, &draft).unwrap().is_none());
    core.finalize_draft(&wallet, &draft).unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DELETE FROM finalized_drafts")
        .unwrap();
    assert!(matches!(
        core.finalized_draft(&wallet, &draft),
        Err(Error::CorruptState)
    ));
    assert!(core.finalize_draft(&wallet, &draft).is_err());
}
