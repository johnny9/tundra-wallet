use super::*;
use bdk_wallet::bitcoin::{consensus::serialize, hex::FromHex};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

#[derive(Clone, Copy)]
enum Reply {
    Exact,
    WrongTxid,
    Refused,
    Oversized,
    Redirect,
    Stalled,
    WrongGenesis,
    Truncated,
}
struct Server {
    endpoint: String,
    posts: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(reply: Reply, before_genesis: Option<Box<dyn FnOnce() + Send>>) -> Self {
        Self::with_hooks(reply, before_genesis, None)
    }
    fn with_hooks(
        reply: Reply,
        before_genesis: Option<Box<dyn FnOnce() + Send>>,
        after_post: Option<Box<dyn FnOnce() + Send>>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let posts = Arc::new(Mutex::new(Vec::new()));
        let captured = posts.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let handle = thread::spawn(move || {
            let mut hook = before_genesis;
            let mut post_hook = after_post;
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let (path, bytes) = read_request(&mut stream);
                        if path == "GET /block-height/0 HTTP/1.1" {
                            if let Some(hook) = hook.take() {
                                hook();
                            }
                            let network = if matches!(reply, Reply::WrongGenesis) {
                                Network::Regtest
                            } else {
                                Network::Signet
                            };
                            let genesis = bdk_wallet::bitcoin::blockdata::constants::genesis_block(
                                network.bitcoin(),
                            )
                            .block_hash()
                            .to_string();
                            respond(&mut stream, "200 OK", &genesis, "");
                        } else {
                            assert_eq!(path, "POST /tx HTTP/1.1");
                            let raw =
                                Vec::<u8>::from_hex(std::str::from_utf8(&bytes).unwrap()).unwrap();
                            let transaction: Transaction = deserialize(&raw).unwrap();
                            captured.lock().unwrap().push(raw);
                            if let Some(hook) = post_hook.take() {
                                hook();
                            }
                            match reply {
                                Reply::Exact | Reply::WrongGenesis => respond(
                                    &mut stream,
                                    "200 OK",
                                    &transaction.compute_txid().to_string(),
                                    "",
                                ),
                                Reply::WrongTxid => {
                                    respond(&mut stream, "200 OK", &"00".repeat(32), "")
                                }
                                Reply::Refused => respond(
                                    &mut stream,
                                    "400 Bad Request",
                                    "private untrusted server diagnostics",
                                    "",
                                ),
                                Reply::Oversized => {
                                    respond(&mut stream, "200 OK", &"a".repeat(129), "")
                                }
                                Reply::Redirect => respond(
                                    &mut stream,
                                    "307 Temporary Redirect",
                                    "",
                                    "Location: http://127.0.0.1:1/tx\r\n",
                                ),
                                Reply::Stalled => thread::sleep(Duration::from_millis(1500)),
                                Reply::Truncated => {
                                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 64\r\nConnection: close\r\n\r\nabc");
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("public test listener: {error}"),
                }
            }
        });
        Self {
            endpoint,
            posts,
            stop,
            handle: Some(handle),
        }
    }
    fn posted(&self) -> Vec<Vec<u8>> {
        self.posts.lock().unwrap().clone()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().unwrap().join().unwrap();
    }
}
fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 8192);
    }
    let header = String::from_utf8(bytes).unwrap();
    let length: usize = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .map(|value| value.parse().unwrap())
        .unwrap_or(0);
    assert!(length <= crate::signing::MAX_PSBT_BYTES * 2);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    (header.lines().next().unwrap().into(), body)
}
fn respond(stream: &mut TcpStream, status: &str, body: &str, extra: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    );
}
fn prepared(path: &std::path::Path) -> (Core, BroadcastRequest, FinalizedReview) {
    let (core, wallet_id, draft_id, signed) = crate::signing::tests::saved_ledger(path);
    core.accept_signed_psbt(&wallet_id, &draft_id, &signed.serialize())
        .unwrap();
    let finalized = core.finalize_draft(&wallet_id, &draft_id).unwrap();
    let request = BroadcastRequest {
        wallet_id,
        draft_id,
        endpoint: "http://127.0.0.1:1".into(),
        expected_txid: finalized.txid.clone(),
        previous_attempt: None,
        privacy_consent: true,
        retry_acknowledged: false,
    };
    (core, request, finalized)
}

#[test]
fn exact_bytes_acknowledgement_and_restart_are_not_chain_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("public.sqlite");
    let (core, mut request, finalized) = prepared(&path);
    let server = Server::new(Reply::Exact, None);
    request.endpoint = server.endpoint.clone();
    let result = core.broadcast_draft(request.clone()).unwrap();
    assert!(result.acknowledged);
    assert_eq!(result.observation, BroadcastObservation::NotSeen);
    assert_eq!(
        server.posted(),
        std::slice::from_ref(&finalized.transaction_bytes)
    );
    assert_eq!(
        core.drafts(&request.wallet_id).unwrap()[0].state,
        "finalized"
    );
    let reserved: std::collections::BTreeSet<_> = core
        .coins(&request.wallet_id)
        .unwrap()
        .into_iter()
        .filter(|coin| coin.status == CoinStatus::Reserved)
        .map(|coin| coin.outpoint)
        .collect();
    let expected = core.drafts(&request.wallet_id).unwrap()[0]
        .inputs
        .iter()
        .map(|input| input.outpoint.clone())
        .collect();
    assert_eq!(reserved, expected);
    assert!(
        core.discard_draft(&request.wallet_id, &request.draft_id)
            .is_err()
    );
    drop(core);
    let reopened = Core::open(path).unwrap();
    assert_eq!(
        reopened
            .broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap(),
        Some(result)
    );
    assert_eq!(server.posted().len(), 1);
}

#[test]
fn wrong_or_missing_server_acknowledgements_remain_uncertain_without_automatic_retry() {
    for reply in [
        Reply::WrongTxid,
        Reply::Refused,
        Reply::Oversized,
        Reply::Redirect,
        Reply::Stalled,
        Reply::Truncated,
    ] {
        let (core, mut request, finalized) = prepared(std::path::Path::new(":memory:"));
        let server = Server::new(reply, None);
        request.endpoint = server.endpoint.clone();
        let result = core.broadcast_draft(request.clone()).unwrap();
        assert!(!result.acknowledged);
        assert_eq!(result.observation, BroadcastObservation::NotSeen);
        assert_eq!(server.posted(), [finalized.transaction_bytes]);
        assert_eq!(
            core.broadcast_status(&request.wallet_id, &request.draft_id)
                .unwrap(),
            Some(result)
        );
    }
}

#[test]
fn consent_identity_current_attempt_and_test_network_are_required_before_post() {
    let (core, mut request, _) = prepared(std::path::Path::new(":memory:"));
    let server = Server::new(Reply::Exact, None);
    request.endpoint = server.endpoint.clone();
    for case in 0..5 {
        let mut bad = request.clone();
        match case {
            0 => bad.privacy_consent = false,
            1 => bad.expected_txid = "00".repeat(32),
            2 => bad.previous_attempt = Some(1),
            3 => bad.endpoint = "http://example.com".into(),
            4 => bad.endpoint = "https://user:password@example.com".into(),
            _ => unreachable!(),
        }
        assert!(core.broadcast_draft(bad).is_err());
    }
    let wrong = Server::new(Reply::WrongGenesis, None);
    request.endpoint = wrong.endpoint.clone();
    assert!(matches!(
        core.broadcast_draft(request.clone()),
        Err(Error::NetworkMismatch)
    ));
    assert!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .is_none()
    );
    assert!(server.posted().is_empty());
    assert!(wrong.posted().is_empty());
}

#[test]
fn unsigned_incomplete_tampered_or_frozen_drafts_cannot_submit() {
    for case in 0..4 {
        let (core, wallet_id, draft_id, signed) =
            crate::signing::tests::saved_ledger(std::path::Path::new(":memory:"));
        if case != 0 {
            let mut response = signed.clone();
            if case == 1 {
                response.inputs[0].partial_sigs.clear();
            }
            core.accept_signed_psbt(&wallet_id, &draft_id, &response.serialize())
                .unwrap();
        }
        if case >= 2 {
            core.finalize_draft(&wallet_id, &draft_id).unwrap();
        }
        if case == 2 {
            core.lock()
                .unwrap()
                .execute("UPDATE finalized_drafts SET transaction_bytes=x'00'", [])
                .unwrap();
        }
        if case == 3 {
            core.edit_coins(
                &wallet_id,
                vec![signed.unsigned_tx.input[0].previous_output.to_string()],
                None,
                Some(true),
            )
            .unwrap();
        }
        let server = Server::new(Reply::Exact, None);
        let request = BroadcastRequest {
            wallet_id,
            draft_id,
            endpoint: server.endpoint.clone(),
            expected_txid: signed.unsigned_tx.compute_txid().to_string(),
            previous_attempt: None,
            privacy_consent: true,
            retry_acknowledged: false,
        };
        assert!(core.broadcast_draft(request).is_err());
        assert!(server.posted().is_empty());
    }
}

#[test]
fn a_change_during_network_preflight_is_revalidated_before_durable_submission() {
    let (core, mut request, _) = prepared(std::path::Path::new(":memory:"));
    let changed = core.clone();
    let wallet = request.wallet_id.clone();
    let outpoint = core.coins(&wallet).unwrap()[0].outpoint.clone();
    let server = Server::new(
        Reply::Exact,
        Some(Box::new(move || {
            changed
                .edit_coins(&wallet, vec![outpoint], None, Some(true))
                .unwrap();
        })),
    );
    request.endpoint = server.endpoint.clone();
    assert!(matches!(
        core.broadcast_draft(request.clone()),
        Err(Error::UnavailableCoin)
    ));
    assert!(server.posted().is_empty());
    assert!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn deliberate_retry_requires_latest_identity_and_preserves_the_exact_transaction() {
    let (core, mut request, finalized) = prepared(std::path::Path::new(":memory:"));
    let server = Server::new(Reply::Exact, None);
    request.endpoint = server.endpoint.clone();
    let first = core.broadcast_draft(request.clone()).unwrap();
    assert!(core.broadcast_draft(request.clone()).is_err());
    request.previous_attempt = Some(first.attempt_id);
    assert!(core.broadcast_draft(request.clone()).is_err());
    request.retry_acknowledged = true;
    let second = core.broadcast_draft(request.clone()).unwrap();
    assert!(second.attempt_id > first.attempt_id);
    assert!(core.broadcast_draft(request).is_err());
    assert_eq!(
        server.posted(),
        [
            finalized.transaction_bytes.clone(),
            finalized.transaction_bytes
        ]
    );
}

#[test]
fn persistence_failure_before_post_sends_nothing_and_after_post_retains_uncertainty() {
    for after_post in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("public.sqlite");
        let (core, mut request, _) = prepared(&path);
        let sql = if after_post {
            "CREATE TRIGGER fail_ack BEFORE UPDATE ON broadcast_attempts BEGIN SELECT RAISE(ABORT,'public test'); END;"
        } else {
            "CREATE TRIGGER fail_attempt BEFORE INSERT ON broadcast_attempts BEGIN SELECT RAISE(ABORT,'public test'); END;"
        };
        core.lock().unwrap().execute_batch(sql).unwrap();
        let server = Server::new(Reply::Exact, None);
        request.endpoint = server.endpoint.clone();
        assert!(matches!(
            core.broadcast_draft(request.clone()),
            Err(Error::Storage)
        ));
        assert_eq!(server.posted().len(), usize::from(after_post));
        drop(core);
        let reopened = Core::open(path).unwrap();
        let result = reopened
            .broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap();
        assert_eq!(result.is_some(), after_post);
        if let Some(result) = result {
            assert!(!result.acknowledged);
        }
        assert_eq!(server.posted().len(), usize::from(after_post));
    }
}

#[test]
fn observed_own_spend_releases_reservations_and_eviction_never_revives_the_approval() {
    let (core, mut request, finalized) = prepared(std::path::Path::new(":memory:"));
    let server = Server::new(Reply::Exact, None);
    request.endpoint = server.endpoint.clone();
    core.broadcast_draft(request.clone()).unwrap();
    let transaction: Transaction = deserialize(&finalized.transaction_bytes).unwrap();
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = load(&tx, &request.wallet_id).unwrap();
        let mut update = bdk_wallet::Update::default();
        update.tx_update.txs.push(Arc::new(transaction.clone()));
        update
            .tx_update
            .seen_ats
            .insert((transaction.compute_txid(), 10));
        loaded.wallet.apply_update(update).unwrap();
        crate::sync::invalidate_drafts(&tx, &request.wallet_id, &loaded.wallet).unwrap();
        crate::engine::save(&tx, &request.wallet_id, &mut loaded).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::Mempool
    );
    assert_eq!(
        core.drafts(&request.wallet_id).unwrap()[0].state,
        "observed"
    );
    assert!(
        !core
            .coins(&request.wallet_id)
            .unwrap()
            .iter()
            .any(|c| c.status == CoinStatus::Reserved)
    );
    assert!(core.broadcast_draft(request.clone()).is_err());
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = load(&tx, &request.wallet_id).unwrap();
        let mut update = bdk_wallet::Update::default();
        update
            .tx_update
            .evicted_ats
            .insert((transaction.compute_txid(), 11));
        loaded.wallet.apply_update(update).unwrap();
        crate::sync::invalidate_drafts(&tx, &request.wallet_id, &loaded.wallet).unwrap();
        crate::engine::save(&tx, &request.wallet_id, &mut loaded).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::NotSeen
    );
    assert_eq!(
        core.drafts(&request.wallet_id).unwrap()[0].state,
        "invalidated"
    );
    assert!(core.broadcast_draft(request).is_err());
    assert_eq!(server.posted(), [serialize(&transaction)]);
}

#[test]
fn independent_connections_racing_the_same_approval_submit_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("public.sqlite");
    let (first, request, _) = prepared(&path);
    let second = Core::open(path).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let first_barrier = barrier.clone();
    let first_server = Server::new(
        Reply::Exact,
        Some(Box::new(move || {
            first_barrier.wait();
        })),
    );
    let second_server = Server::new(
        Reply::Exact,
        Some(Box::new(move || {
            barrier.wait();
        })),
    );
    let mut first_request = request.clone();
    first_request.endpoint = first_server.endpoint.clone();
    let mut second_request = request;
    second_request.endpoint = second_server.endpoint.clone();
    let a = thread::spawn(move || first.broadcast_draft(first_request));
    let b = thread::spawn(move || second.broadcast_draft(second_request));
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        first_server.posted().len() + second_server.posted().len(),
        1
    );
}

#[test]
fn schema_five_migration_preserves_final_bytes_and_wallet_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("public.sqlite");
    let (core, request, finalized) = prepared(&path);
    let other = core
        .import_wallet(
            "Other public wallet",
            include_str!("../../../tests/fixtures/single-sig.txt"),
            Network::Signet,
        )
        .unwrap();
    core.lock()
        .unwrap()
        .execute_batch("DROP TABLE broadcast_attempts; PRAGMA user_version=5;")
        .unwrap();
    drop(core);
    let reopened = Core::open(path).unwrap();
    assert_eq!(
        reopened
            .finalized_draft(&request.wallet_id, &request.draft_id)
            .unwrap(),
        Some(finalized)
    );
    assert!(
        reopened
            .broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .is_none()
    );
    let server = Server::new(Reply::Exact, None);
    let mut wrong = request.clone();
    wrong.wallet_id = other.id;
    wrong.endpoint = server.endpoint.clone();
    assert!(reopened.broadcast_draft(wrong).is_err());
    assert!(server.posted().is_empty());
    let mut correct = request;
    correct.endpoint = server.endpoint.clone();
    assert!(reopened.broadcast_draft(correct).unwrap().acknowledged);
}

#[test]
fn process_death_during_post_reopens_with_uncertainty_and_no_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("public.sqlite");
    let (core, request, finalized) = prepared(&path);
    drop(core);
    let server = Server::new(Reply::Stalled, None);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "broadcast::tests::broadcast_crash_child",
            "--ignored",
        ])
        .env("TUNDRA_PUBLIC_BROADCAST_DB", &path)
        .env("TUNDRA_PUBLIC_BROADCAST_ENDPOINT", &server.endpoint)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while server.posted().is_empty() && std::time::Instant::now() < deadline {
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before public submission"
        );
        thread::sleep(Duration::from_millis(2));
    }
    if server.posted().is_empty() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("public POST not received");
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    let reopened = Core::open(path).unwrap();
    let info = reopened
        .broadcast_status(&request.wallet_id, &request.draft_id)
        .unwrap()
        .unwrap();
    assert!(!info.acknowledged);
    assert_eq!(info.observation, BroadcastObservation::NotSeen);
    assert_eq!(server.posted(), [finalized.transaction_bytes]);
    assert!(reopened.broadcast_draft(request).is_err()); // No implicit retry after restart.
}

#[test]
#[ignore = "process-kill helper invoked only by its parent test"]
fn broadcast_crash_child() {
    let core = Core::open(std::env::var("TUNDRA_PUBLIC_BROADCAST_DB").unwrap()).unwrap();
    let wallet = core.wallets().unwrap().into_iter().next().unwrap();
    let draft = core.drafts(&wallet.id).unwrap().into_iter().next().unwrap();
    let finalized = core
        .finalized_draft(&wallet.id, &draft.id)
        .unwrap()
        .unwrap();
    let _ = core.broadcast_draft(BroadcastRequest {
        wallet_id: wallet.id,
        draft_id: draft.id,
        endpoint: std::env::var("TUNDRA_PUBLIC_BROADCAST_ENDPOINT").unwrap(),
        expected_txid: finalized.txid,
        previous_attempt: None,
        privacy_consent: true,
        retry_acknowledged: false,
    });
    panic!("parent must kill child during public POST");
}

#[test]
fn mainnet_watch_only_import_does_not_enable_broadcast() {
    use bdk_wallet::bitcoin::{NetworkKind, bip32::Xpub};
    use std::str::FromStr;
    let descriptor = include_str!("../../../tests/fixtures/single-sig.txt")
        .trim()
        .split('#')
        .next()
        .unwrap();
    let start = descriptor.find("tpub").unwrap();
    let end = start + descriptor[start..].find('/').unwrap();
    let mut xpub = Xpub::from_str(&descriptor[start..end]).unwrap();
    xpub.network = NetworkKind::Main; // Re-encode only the published public account.
    let descriptor = descriptor.replace(&descriptor[start..end], &xpub.to_string());
    let checksum = bdk_wallet::descriptor::checksum::calc_checksum(&descriptor).unwrap();
    let core = Core::open(":memory:").unwrap();
    let wallet = core
        .import_wallet(
            "Public mainnet watch-only",
            &format!("{descriptor}#{checksum}"),
            Network::Mainnet,
        )
        .unwrap();
    let server = Server::new(Reply::Exact, None);
    let result = core.broadcast_draft(BroadcastRequest {
        wallet_id: wallet.id,
        draft_id: "missing".into(),
        endpoint: server.endpoint.clone(),
        expected_txid: "00".repeat(32),
        previous_attempt: None,
        privacy_consent: true,
        retry_acknowledged: false,
    });
    assert!(matches!(result, Err(Error::Unavailable(_))));
    assert!(server.posted().is_empty());
}

#[test]
fn a_late_acknowledgement_cannot_overwrite_a_newer_uncertain_attempt() {
    let (core, mut request, finalized) = prepared(std::path::Path::new(":memory:"));
    let (release, waiting) = std::sync::mpsc::channel();
    let slow = Server::with_hooks(
        Reply::Exact,
        None,
        Some(Box::new(move || {
            waiting.recv_timeout(Duration::from_secs(3)).unwrap();
        })),
    );
    request.endpoint = slow.endpoint.clone();
    let first_core = core.clone();
    let first_request = request.clone();
    let first = thread::spawn(move || first_core.broadcast_draft(first_request));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while slow.posted().is_empty() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        slow.posted(),
        std::slice::from_ref(&finalized.transaction_bytes)
    );
    let pending = core
        .broadcast_status(&request.wallet_id, &request.draft_id)
        .unwrap()
        .unwrap();
    assert!(!pending.acknowledged);
    let newer = Server::new(Reply::Refused, None);
    request.endpoint = newer.endpoint.clone();
    request.previous_attempt = Some(pending.attempt_id);
    request.retry_acknowledged = true;
    let second = core.broadcast_draft(request.clone());
    release.send(()).unwrap();
    let first_result = first.join().unwrap().unwrap();
    let second = second.unwrap();
    assert!(second.attempt_id > pending.attempt_id);
    assert!(!second.acknowledged);
    assert_eq!(first_result, second);
    let old_ack: bool = core
        .lock()
        .unwrap()
        .query_row(
            "SELECT acknowledged FROM broadcast_attempts WHERE id=?1",
            [pending.attempt_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(old_ack);
    assert_eq!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap(),
        Some(second)
    );
    assert_eq!(newer.posted(), [finalized.transaction_bytes]);
}

#[test]
fn confirmed_observation_is_derived_from_chain_and_removed_by_reorg() {
    use bdk_wallet::bitcoin::{Block, CompactTarget, TxMerkleNode, block, hashes::Hash};
    let (core, mut request, finalized) = prepared(std::path::Path::new(":memory:"));
    let server = Server::new(Reply::Exact, None);
    request.endpoint = server.endpoint.clone();
    core.broadcast_draft(request.clone()).unwrap();
    let transaction: Transaction = deserialize(&finalized.transaction_bytes).unwrap();
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = load(&tx, &request.wallet_id).unwrap();
        // Public signed transaction in a test-only graph, not a mined/signature-device gate.
        let mut block = Block {
            header: block::Header {
                version: block::Version::ONE,
                prev_blockhash: loaded.wallet.local_chain().tip().hash(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 20,
                bits: CompactTarget::from_consensus(0x207fffff),
                nonce: 0,
            },
            txdata: vec![transaction.clone()],
        };
        block.header.merkle_root = block.compute_merkle_root().unwrap();
        loaded.wallet.apply_block(&block, 2).unwrap();
        crate::sync::invalidate_drafts(&tx, &request.wallet_id, &loaded.wallet).unwrap();
        crate::engine::save(&tx, &request.wallet_id, &mut loaded).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::Confirmed
    );
    assert_eq!(
        core.drafts(&request.wallet_id).unwrap()[0].state,
        "observed"
    );
    {
        let mut db = core.lock().unwrap();
        let tx = db.transaction().unwrap();
        let mut loaded = load(&tx, &request.wallet_id).unwrap();
        loaded.state.local_chain.blocks.remove(&2);
        loaded.wallet = bdk_wallet::Wallet::load()
            .load_wallet_no_persist(loaded.state.clone())
            .unwrap()
            .unwrap();
        let mut update = bdk_wallet::Update::default();
        update
            .tx_update
            .evicted_ats
            .insert((transaction.compute_txid(), 21));
        loaded.wallet.apply_update(update).unwrap();
        crate::sync::invalidate_drafts(&tx, &request.wallet_id, &loaded.wallet).unwrap();
        crate::engine::save(&tx, &request.wallet_id, &mut loaded).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        core.broadcast_status(&request.wallet_id, &request.draft_id)
            .unwrap()
            .unwrap()
            .observation,
        BroadcastObservation::NotSeen
    );
    assert_eq!(
        core.drafts(&request.wallet_id).unwrap()[0].state,
        "invalidated"
    );
    assert!(core.broadcast_draft(request).is_err());
    assert_eq!(server.posted().len(), 1);
}
