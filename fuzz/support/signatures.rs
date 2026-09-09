//! Fuzz-only access to pure validators, with immutable published approvals/policies.
use super::*;
use bdk_wallet::bitcoin::Witness;
use std::sync::OnceLock;

struct Fixture {
    approved: Psbt,
    signed: Psbt,
    trusted: Vec<TrustedInput>,
    partial: Psbt,
}
fn fixtures() -> &'static [Fixture; 3] {
    static FIXTURES: OnceLock<[Fixture; 3]> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        [
            public_fixtures::hwi(),
            public_fixtures::ledger(),
            public_fixtures::public_multisig(),
        ]
        .map(|(approved, signed, trusted)| {
            let mut partial = merge_response(&approved, None, &signed, &trusted).unwrap();
            for input in &mut partial.inputs {
                input.partial_sigs.pop_last();
            }
            assert!(
                !signing_progress("public fixture", &partial, &trusted)
                    .unwrap()
                    .complete
            );
            Fixture {
                approved,
                signed,
                trusted,
                partial,
            }
        })
    })
}

pub fn check(data: &[u8]) {
    let Some((&control, data)) = data.split_first() else {
        return;
    };
    let fixture = &fixtures()[usize::from(control & 3) % 3];
    let encoded;
    let bytes = match (control >> 2) & 3 {
        0 => data,
        1 => {
            // Repair the container by starting from a valid public response, then mutate
            // up to 64 individual bytes. This reaches DER, sighash and witness validation.
            let mut candidate = fixture.signed.serialize();
            for edit in data.chunks_exact(3).take(64) {
                let offset = usize::from(u16::from_be_bytes([edit[0], edit[1]])) % candidate.len();
                candidate[offset] ^= edit[2];
            }
            encoded = candidate;
            &encoded
        }
        2 => {
            let mut candidate = fixture.signed.clone();
            let index = usize::from(control >> 4) % candidate.inputs.len();
            let stack: Vec<_> = data.chunks(80).take(16).collect();
            candidate.inputs[index].final_script_witness = Some(Witness::from_slice(&stack));
            encoded = candidate.serialize();
            &encoded
        }
        _ => {
            let mut candidate = fixture.signed.clone();
            let index = usize::from(control >> 4) % candidate.inputs.len();
            candidate.inputs[index].partial_sigs.clear();
            candidate.inputs[index].final_script_witness = None;
            encoded = candidate.serialize();
            &encoded
        }
    };
    let Ok(response) = parse_response(bytes) else {
        return;
    };
    let stored = (control & 0x80 != 0).then_some(&fixture.partial);
    let merged = match merge_response(&fixture.approved, stored, &response, &fixture.trusted) {
        Ok(merged) => merged,
        Err(error) => {
            assert!(error.to_string().len() <= 200);
            return;
        }
    };
    assert_eq!(merged.unsigned_tx, fixture.approved.unsigned_tx);
    assert_eq!(
        merge_response(
            &fixture.approved,
            Some(&merged),
            &response,
            &fixture.trusted
        )
        .unwrap(),
        merged
    );
    let progress = signing_progress("public fixture", &merged, &fixture.trusted).unwrap();
    for (input, policy) in progress.inputs.iter().zip(&fixture.trusted) {
        assert!(input.valid_signatures as usize <= policy.keys().unwrap().0.len());
    }
    let finalized = finalize_transaction(&fixture.approved, Some(&merged), &fixture.trusted);
    if !progress.complete {
        assert!(finalized.is_err());
        return;
    }
    let transaction = finalized.unwrap();
    assert_eq!(
        transaction.compute_txid(),
        fixture.approved.unsigned_tx.compute_txid()
    );
    let mut final_response = fixture.approved.clone();
    let mut unsigned = transaction.clone();
    for (index, input) in transaction.input.iter().enumerate() {
        assert!(!input.witness.is_empty());
        final_response.inputs[index].final_script_witness = Some(input.witness.clone());
        unsigned.input[index].witness = Witness::new();
    }
    assert_eq!(unsigned, fixture.approved.unsigned_tx);
    assert!(
        signing_progress(
            "public fixture",
            &merge_response(&fixture.approved, None, &final_response, &fixture.trusted).unwrap(),
            &fixture.trusted
        )
        .unwrap()
        .complete
    );
}
