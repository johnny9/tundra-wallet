//! Public published signatures over a synthetic local chain; NOT a Signet broadcast.
//! This verifies the fixtures shared with native tests using the production Core API.
use bdk_wallet::bitcoin::hex::DisplayHex;
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tundra_core::{
    BroadcastObservation, BroadcastRequest, CoinStatus, Core, SyncPhase, restore_backup,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}
fn manifest() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/native-signing.json")).unwrap()
}
struct Server {
    child: Child,
    endpoint: String,
}
impl Server {
    fn start(directory: &Path) -> Self {
        let port_file = directory.join("port");
        let child = Command::new("python3")
            .arg(fixtures().join("../esplora_published.py"))
            .args(["--port", "0", "--port-file"])
            .arg(&port_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Python is required for the native public fixture server");
        let mut server = Self {
            child,
            endpoint: String::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !port_file.exists() {
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "Public fixture server exited"
            );
            assert!(
                Instant::now() < deadline,
                "Public fixture server did not start"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let port: u16 = fs::read_to_string(port_file).unwrap().parse().unwrap();
        server.endpoint = format!("http://127.0.0.1:{port}");
        server
    }
    fn posts(&self) -> u64 {
        let mut stream = TcpStream::connect(self.endpoint.trim_start_matches("http://")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /_fixture_state HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut bytes = String::new();
        stream.take(4096).read_to_string(&mut bytes).unwrap();
        let body = bytes.split_once("\r\n\r\n").unwrap().1;
        serde_json::from_str::<Value>(body).unwrap()["posts"]
            .as_u64()
            .unwrap()
    }
    fn sync(&self, core: &Core, wallet: &str) {
        let operation = core.prepare_sync(wallet, &self.endpoint, true).unwrap();
        core.run_sync(operation.id).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let progress = core.sync_progress(operation.id).unwrap();
            match progress.phase {
                SyncPhase::Complete => break,
                SyncPhase::Failed | SyncPhase::Cancelled => {
                    panic!("Public fixture scan failed: {:?}", progress.error)
                }
                _ => assert!(Instant::now() < deadline, "Public fixture scan timed out"),
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn published_response_completes_every_input_and_finalizes_exact_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unsigned.sqlite");
    fs::copy(fixtures().join("native-unsigned.sqlite"), &path).unwrap();
    let fixture = manifest();
    let wallet = fixture["wallet_id"].as_str().unwrap();
    let draft = fixture["draft_id"].as_str().unwrap();
    let core = Core::open(&path).unwrap();
    assert_eq!(core.drafts(wallet).unwrap()[0].state, "unsigned");
    assert!(!core.signing_progress(wallet, draft).unwrap().complete);
    assert!(core.finalize_draft(wallet, draft).is_err());
    let progress = core
        .accept_signed_psbt(
            wallet,
            draft,
            include_bytes!("../../../tests/fixtures/native-signed-response.psbt"),
        )
        .unwrap();
    assert!(progress.complete);
    assert_eq!(progress.inputs.len(), 2);
    assert!(
        progress
            .inputs
            .iter()
            .all(|input| input.valid_signatures == input.required_signatures)
    );
    let final_tx = core.finalize_draft(wallet, draft).unwrap();
    assert_eq!(final_tx.txid, fixture["final"]["txid"]);
    assert_eq!(final_tx.wtxid, fixture["final"]["wtxid"]);
    assert_eq!(
        final_tx.transaction_bytes.to_lower_hex_string(),
        fixture["final"]["raw"]
    );
    assert!(core.broadcast_status(wallet, draft).unwrap().is_none());
    drop(core);
    let core = Core::open(path).unwrap();
    assert_eq!(
        core.finalized_draft(wallet, draft).unwrap().unwrap(),
        final_tx
    );
}

#[test]
fn recovered_published_payment_requires_sync_review_and_separate_exact_submission() {
    let directory = tempfile::tempdir().unwrap();
    let server = Server::start(directory.path());
    let fixture = manifest();
    let wallet = fixture["wallet_id"].as_str().unwrap();
    let draft = fixture["draft_id"].as_str().unwrap();
    let txid = fixture["final"]["txid"].as_str().unwrap();
    let path = directory.path().join("protected.sqlite");
    // Public test STORAGE key; never a Bitcoin signing key.
    restore_backup(
        fixtures().join("native-signed-backup.tundra"),
        &path,
        fixture["password"].as_str().unwrap().into(),
        vec![0x33; 32],
    )
    .unwrap();
    let core = Core::open_protected(&path, vec![0x33; 32]).unwrap();
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
    assert!(core.wallets().unwrap()[0].synced_at.is_none());
    assert!(core.recovery_required(wallet, draft).unwrap());
    assert_eq!(core.drafts(wallet).unwrap()[0].state, "invalidated");
    let previous = core.broadcast_status(wallet, draft).unwrap().unwrap();
    assert_eq!(previous.attempt_id, 1);
    assert!(!previous.acknowledged);
    assert!(
        core.resume_recovered_submission(wallet, draft, txid, 1, true)
            .is_err()
    );
    assert_eq!(server.posts(), 0);
    server.sync(&core, wallet);
    assert!(core.wallets().unwrap()[0].synced_at.unwrap() > 1);
    assert_eq!(
        core.coins(wallet)
            .unwrap()
            .iter()
            .filter(|coin| coin.status == CoinStatus::Reserved)
            .count(),
        2
    );
    assert!(
        core.resume_recovered_submission(wallet, draft, txid, 1, false)
            .is_err()
    );
    assert!(
        core.resume_recovered_submission(wallet, draft, txid, 2, true)
            .is_err()
    );
    let final_tx = core
        .resume_recovered_submission(wallet, draft, txid, 1, true)
        .unwrap();
    assert_eq!(
        final_tx.transaction_bytes.to_lower_hex_string(),
        fixture["final"]["raw"]
    );
    assert!(!core.recovery_required(wallet, draft).unwrap());
    assert_eq!(server.posts(), 0);
    let mut request = BroadcastRequest {
        wallet_id: wallet.into(),
        draft_id: draft.into(),
        endpoint: server.endpoint.clone(),
        expected_txid: txid.into(),
        previous_attempt: Some(1),
        privacy_consent: false,
        retry_acknowledged: false,
    };
    assert!(core.broadcast_draft(request.clone()).is_err());
    request.privacy_consent = true;
    assert!(core.broadcast_draft(request.clone()).is_err());
    assert_eq!(server.posts(), 0);
    request.retry_acknowledged = true;
    let sent = core.broadcast_draft(request.clone()).unwrap();
    assert!(sent.acknowledged);
    assert_eq!(sent.observation, BroadcastObservation::NotSeen);
    assert_eq!(server.posts(), 1);
    assert!(core.broadcast_draft(request).is_err()); // Stale attempt cannot retry.
    drop(core);
    let core = Core::open_protected(&path, vec![0x33; 32]).unwrap();
    assert_eq!(core.broadcast_status(wallet, draft).unwrap().unwrap(), sent);
    assert_eq!(server.posts(), 1); // Opening never retries.
    server.sync(&core, wallet);
    assert_eq!(
        core.broadcast_status(wallet, draft)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::Mempool
    );
    assert_eq!(core.drafts(wallet).unwrap()[0].state, "observed");
    let coins = core.coins(wallet).unwrap();
    assert!(!coins.iter().any(|coin| coin.status == CoinStatus::Reserved));
    let owned = fixture["final"]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .position(|output| output["is_mine"] == true)
        .unwrap();
    let outpoint = format!("{txid}:{owned}");
    let source = core.output_source(wallet, &outpoint).unwrap().unwrap();
    assert_eq!(source.id, draft);
    assert_eq!(source.inputs.len(), 2);
    assert_eq!(
        coins
            .iter()
            .find(|coin| coin.outpoint == outpoint)
            .unwrap()
            .label,
        source.label
    );
    core.set_label(
        wallet,
        "output",
        &outpoint,
        "Public user edit after observation",
    )
    .unwrap();
    server.sync(&core, wallet);
    assert_eq!(
        core.coins(wallet)
            .unwrap()
            .iter()
            .find(|coin| coin.outpoint == outpoint)
            .unwrap()
            .label,
        "Public user edit after observation"
    );
    assert_eq!(server.posts(), 1);
}
