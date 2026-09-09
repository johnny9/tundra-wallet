//! Public signature fixtures shared only by unit tests and fuzz builds.
use super::*;
use bdk_wallet::bitcoin::{Amount, ScriptBuf, Witness};
use std::str::FromStr;

pub(super) fn public_multisig() -> (Psbt, Psbt, Vec<TrustedInput>) {
    use bdk_wallet::bitcoin::{consensus::encode::deserialize_hex, hex::FromHex};
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/public-signed-multisig.json")).unwrap();
    let transaction: Transaction =
        deserialize_hex(data["raw_transaction"].as_str().unwrap()).unwrap();
    assert_eq!(
        transaction.compute_txid().to_string(),
        data["txid"].as_str().unwrap()
    );
    let mut unsigned = transaction.clone();
    for input in &mut unsigned.input {
        input.witness = Witness::new();
    }
    let mut approved = Psbt::from_unsigned_tx(unsigned).unwrap();
    let mut trusted = Vec::new();
    for (index, input) in transaction.input.iter().enumerate() {
        let script = ScriptBuf::from_bytes(input.witness.last().unwrap().to_vec());
        let keys = [2..35, 36..69, 70..103]
            .map(|range| PublicKey::from_slice(&script.as_bytes()[range]).unwrap());
        let descriptor = Descriptor::new_wsh_sortedmulti(2, keys.to_vec()).unwrap();
        assert_eq!(descriptor.explicit_script().unwrap(), script);
        let prev = &data["prevouts"][index];
        let prevout = TxOut {
            value: Amount::from_sat(prev["sats"].as_u64().unwrap()),
            script_pubkey: ScriptBuf::from_bytes(
                Vec::<u8>::from_hex(prev["script_pubkey"].as_str().unwrap()).unwrap(),
            ),
        };
        let metadata = psbt::Input {
            witness_utxo: Some(prevout.clone()),
            witness_script: Some(script),
            ..Default::default()
        };
        approved.inputs[index] = metadata.clone();
        trusted.push(TrustedInput {
            prevout,
            metadata,
            descriptor,
        });
    }
    let mut signed = approved.clone();
    for (index, input) in transaction.input.iter().enumerate() {
        signed.inputs[index].final_script_witness = Some(input.witness.clone());
    }
    (approved, signed, trusted)
}

pub(super) fn hwi() -> (Psbt, Psbt, Vec<TrustedInput>) {
    let signed = parse_response(include_bytes!("fixtures/hwi-signed-wpkh.psbt")).unwrap();
    let mut approved = signed.clone();
    let mut trusted = Vec::new();
    for input in &mut approved.inputs {
        let key = *input.partial_sigs.keys().next().unwrap();
        input.partial_sigs.clear();
        trusted.push(TrustedInput {
            prevout: input.witness_utxo.clone().unwrap(),
            metadata: input.clone(),
            descriptor: Descriptor::new_wpkh(key).unwrap(),
        });
    }
    (approved, signed, trusted)
}

pub(super) fn ledger() -> (Psbt, Psbt, Vec<TrustedInput>) {
    let approved = parse_response(include_bytes!("fixtures/ledger-wpkh-two-inputs.psbt")).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ledger-wpkh-two-inputs.json")).unwrap();
    let mut signed = approved.clone();
    let mut trusted = Vec::new();
    for row in data["expected_signatures"].as_array().unwrap() {
        let index = row["input_index"].as_u64().unwrap() as usize;
        let key = PublicKey::from_str(row["pubkey"].as_str().unwrap()).unwrap();
        let signature = ecdsa::Signature::from_str(row["signature"].as_str().unwrap()).unwrap();
        signed.inputs[index].partial_sigs.insert(key, signature);
        trusted.push(TrustedInput {
            prevout: approved.inputs[index].witness_utxo.clone().unwrap(),
            metadata: approved.inputs[index].clone(),
            descriptor: Descriptor::new_wpkh(key).unwrap(),
        });
    }
    (approved, signed, trusted)
}
