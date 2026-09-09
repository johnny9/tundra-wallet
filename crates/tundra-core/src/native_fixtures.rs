//! Explicit public native-test fixture generation. Never linked into the production API.
use crate::{Core, signing::tests::saved_ledger};
use bdk_wallet::bitcoin::{
    self, Block, CompactTarget, TxMerkleNode, block,
    consensus::serialize,
    hashes::{Hash, sha256},
    hex::DisplayHex,
};
use rusqlite::params;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[test]
#[ignore = "explicit public fixture generator requiring a new TUNDRA_NATIVE_SIGNED_FIXTURES directory"]
fn export_published_signing_fixtures() {
    let output = PathBuf::from(std::env::var("TUNDRA_NATIVE_SIGNED_FIXTURES").unwrap());
    fs::create_dir(&output).expect("fixture output must be a new directory");
    let temporary = tempfile::tempdir().unwrap();
    let working = temporary.path().join("public.sqlite");
    let (core, wallet, draft, signed) = saved_ledger(&working);
    assert_eq!(core.drafts(&wallet).unwrap()[0].state, "unsigned");
    assert!(!core.signing_progress(&wallet, &draft).unwrap().complete);
    drop(core);
    fs::copy(&working, output.join("native-unsigned.sqlite")).unwrap();
    let core = Core::open(&working).unwrap();
    let signed_bytes = signed.serialize();
    fs::write(output.join("native-signed-response.psbt"), &signed_bytes).unwrap();
    assert!(
        core.accept_signed_psbt(&wallet, &draft, &signed_bytes)
            .unwrap()
            .complete
    );
    let finalized = core.finalize_draft(&wallet, &draft).unwrap();
    // A synthetic historical uncertainty record, never an actual network request.
    core.lock().unwrap().execute("INSERT INTO broadcast_attempts(wallet_id,draft_id,endpoint,txid,wtxid,requested_at) VALUES(?1,?2,'http://127.0.0.1:1',?3,?4,1)", params![wallet,draft,finalized.txid,finalized.wtxid]).unwrap();
    let password = "Public native signing backup 2026";
    core.export_backup(output.join("native-signed-backup.tundra"), password.into())
        .unwrap();

    let funding: BTreeMap<_, _> = signed
        .inputs
        .iter()
        .map(|input| {
            let transaction = input.non_witness_utxo.clone().unwrap();
            (transaction.compute_txid(), transaction)
        })
        .collect();
    let genesis = bitcoin::blockdata::constants::genesis_block(bitcoin::Network::Signet);
    let mut block = Block {
        header: block::Header {
            version: block::Version::ONE,
            prev_blockhash: genesis.block_hash(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 1,
            bits: CompactTarget::from_consensus(0x207fffff),
            nonce: 0,
        },
        txdata: funding.into_values().collect(),
    };
    block.header.merkle_root = block.compute_merkle_root().unwrap();
    assert_eq!(
        crate::engine::load(&core.lock().unwrap(), &wallet)
            .unwrap()
            .wallet
            .local_chain()
            .tip()
            .hash(),
        block.block_hash()
    );
    let transactions = block.txdata.iter().map(|transaction| json!({
        "txid": transaction.compute_txid().to_string(),
        "raw": serialize(transaction).to_lower_hex_string(),
        "outputs": transaction.output.iter().map(|output| json!({
            "sats": output.value.to_sat(), "script": output.script_pubkey.to_hex_string(),
            "scripthash": sha256::Hash::hash(output.script_pubkey.as_bytes()).to_string(),
        })).collect::<Vec<_>>(),
        "inputs": transaction.input.iter().map(|input| input.previous_output.to_string()).collect::<Vec<_>>(),
    })).collect::<Vec<_>>();
    let approved = core.drafts(&wallet).unwrap().remove(0);
    assert!(approved.outputs.iter().any(|output| output.is_mine));
    let manifest = json!({
        "purpose": "Public published-signature native fixtures; NEVER FUND; no Bitcoin signing keys",
        "generator": "native_fixtures::export_published_signing_fixtures",
        "schema": 8,
        "password": password,
        "sources": ["ledger-wpkh-two-inputs.psbt", "ledger-wpkh-two-inputs.json", "signing-provenance.json"],
        "limits": ["Synthetic Signet block is not a valid Signet chain", "Timestamp 1 is fixture metadata, not real synchronization", "Historical attempt is synthetic; no real broadcast occurred", "Signatures were copied from published Ledger vectors, never generated"],
        "wallet_id": wallet, "draft_id": draft,
        "funded_btc": crate::amount::format_btc(core.wallets().unwrap()[0].total_sats.unwrap()),
        "genesis": {"hash": genesis.block_hash().to_string(), "header": serialize(&genesis.header).to_lower_hex_string()},
        "block": {"height": 1, "hash": block.block_hash().to_string(), "header": serialize(&block.header).to_lower_hex_string(), "time": 1, "transactions": transactions},
        "final": {"txid": finalized.txid, "wtxid": finalized.wtxid, "raw": finalized.transaction_bytes.to_lower_hex_string(), "fee_sats": finalized.fee_sats, "vsize": finalized.vsize,
            "inputs": signed.unsigned_tx.input.iter().map(|input| input.previous_output.to_string()).collect::<Vec<_>>(),
            "outputs": signed.unsigned_tx.output.iter().zip(&approved.outputs).map(|(output,review)| json!({"sats": output.value.to_sat(), "script": output.script_pubkey.to_hex_string(), "scripthash": sha256::Hash::hash(output.script_pubkey.as_bytes()).to_string(), "is_mine": review.is_mine})).collect::<Vec<_>>()},
    });
    fs::write(
        output.join("native-signing.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    // Hash exactly the generated artifacts; random SQLCipher salts make backup regeneration
    // produce new ciphertext. Review/pin these files rather than silently regenerating them.
    let hashes: BTreeMap<_, _> = [
        "native-unsigned.sqlite",
        "native-signed-response.psbt",
        "native-signed-backup.tundra",
        "native-signing.json",
    ]
    .into_iter()
    .map(|name| {
        let bytes = fs::read(output.join(name)).unwrap();
        (
            name,
            json!({"size": bytes.len(), "sha256": sha256::Hash::hash(&bytes).to_string()}),
        )
    })
    .collect();
    fs::write(
        output.join("native-signing-hashes.json"),
        serde_json::to_vec_pretty(&hashes).unwrap(),
    )
    .unwrap();
}
