//! Adversarial loopback servers; no public network is contacted.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tundra_core::{Core, Network, SyncPhase};

#[derive(Clone, Copy)]
enum Mode {
    Empty,
    WrongNetwork,
    Redirect,
    Oversized,
    Malformed,
    Stall,
}
struct Server {
    endpoint: String,
    mode: Arc<Mutex<Mode>>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(mode: Mode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let mode = Arc::new(Mutex::new(mode));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let (m, r, s, released) = (
            mode.clone(),
            requests.clone(),
            stop.clone(),
            release.clone(),
        );
        let worker = thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = [0u8; 4096];
                        let size = stream.read(&mut request).unwrap_or(0);
                        let request = String::from_utf8_lossy(&request[..size]);
                        let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
                        r.lock().unwrap().push(path.clone());
                        let mode = *m.lock().unwrap();
                        if matches!(mode, Mode::Stall) {
                            while !released.load(Ordering::SeqCst) && !s.load(Ordering::SeqCst) {
                                thread::sleep(Duration::from_millis(5));
                            }
                        }
                        match mode {
                            Mode::Redirect => send(
                                &mut stream,
                                "302 Found",
                                "Location: http://127.0.0.1:1/should-never-follow\r\n",
                                b"",
                            ),
                            Mode::Oversized => {
                                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\nConnection: close\r\n\r\n");
                            }
                            Mode::WrongNetwork => send(
                                &mut stream,
                                "200 OK",
                                "",
                                b"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
                            ),
                            _ => {
                                let body = match path.as_str() {
                                    "/block-height/0" => {
                                        bdk_wallet::bitcoin::blockdata::constants::genesis_block(
                                            bdk_wallet::bitcoin::Network::Signet,
                                        )
                                        .block_hash()
                                        .to_string()
                                    }
                                    "/blocks/tip/height" => "0".into(),
                                    _ if matches!(mode, Mode::Malformed) => "{malformed".into(),
                                    _ => "[]".into(),
                                };
                                send(&mut stream, "200 OK", "", body.as_bytes());
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            endpoint,
            mode,
            requests,
            stop,
            release,
            worker: Some(worker),
        }
    }
    fn await_request(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.requests.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.release.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn send(stream: &mut TcpStream, status: &str, headers: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}
fn setup(path: &str) -> (Core, String) {
    let c = Core::open(path).unwrap();
    let w = c
        .import_wallet(
            "Secret label 🧊",
            include_str!("../../../tests/fixtures/single-sig.txt"),
            Network::Signet,
        )
        .unwrap();
    (c, w.id)
}
fn wait(core: &Core, op: u64) -> SyncPhase {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let phase = core.sync_progress(op).unwrap().phase;
        if matches!(
            phase,
            SyncPhase::Complete | SyncPhase::Cancelled | SyncPhase::Failed
        ) {
            return phase;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
}
fn start(core: &Core, id: &str, server: &Server) -> u64 {
    let op = core.prepare_sync(id, &server.endpoint, true).unwrap();
    core.run_sync(op.id).unwrap();
    op.id
}
#[test]
fn failures_and_redirects_never_convert_unknown_to_zero() {
    for mode in [
        Mode::WrongNetwork,
        Mode::Redirect,
        Mode::Oversized,
        Mode::Malformed,
    ] {
        let server = Server::new(mode);
        let (core, id) = setup(":memory:");
        let op = start(&core, &id, &server);
        assert_eq!(wait(&core, op), SyncPhase::Failed);
        assert!(core.wallets().unwrap()[0].total_sats.is_none());
        assert!(core.sync_endpoint(&id).unwrap().is_none());
        let error = core.sync_progress(op).unwrap().error.unwrap();
        assert!(!error.contains("127.0.0.1") && !error.contains("Secret label"));
        if matches!(mode, Mode::Redirect) {
            assert_eq!(server.requests.lock().unwrap().len(), 1);
        }
    }
}
#[test]
fn cancellation_during_io_is_prompt_and_does_not_hold_database_lock() {
    let server = Server::new(Mode::Stall);
    let (core, id) = setup(":memory:");
    let op = start(&core, &id, &server);
    server.await_request();
    assert_eq!(core.receive_address(&id).unwrap().index, 0);
    let time = Instant::now();
    assert_eq!(core.cancel_sync(op).unwrap().phase, SyncPhase::Cancelled);
    assert!(time.elapsed() < Duration::from_secs(1));
    server.release.store(true, Ordering::SeqCst);
    assert!(core.wallets().unwrap()[0].synced_at.is_none());
}
#[test]
fn changing_wallet_state_during_scan_rejects_stale_update() {
    let server = Server::new(Mode::Stall);
    let (core, id) = setup(":memory:");
    let op = start(&core, &id, &server);
    server.await_request();
    core.receive_address(&id).unwrap();
    server.release.store(true, Ordering::SeqCst);
    assert_eq!(wait(&core, op), SyncPhase::Failed);
    assert_eq!(core.receive_address(&id).unwrap().index, 1);
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
}
#[test]
fn successful_zero_is_cached_and_failure_preserves_its_timestamp() {
    let server = Server::new(Mode::Empty);
    let (core, id) = setup(":memory:");
    assert_eq!(wait(&core, start(&core, &id, &server)), SyncPhase::Complete);
    let previous = core.wallets().unwrap().remove(0);
    assert_eq!(previous.total_sats, Some(0));
    *server.mode.lock().unwrap() = Mode::Malformed;
    assert_eq!(wait(&core, start(&core, &id, &server)), SyncPhase::Failed);
    assert_eq!(core.wallets().unwrap()[0].synced_at, previous.synced_at);
    let requests = server.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .all(|r| r.starts_with("/block") || r.starts_with("/scripthash/"))
    );
    assert!(
        requests
            .iter()
            .all(|r| !r.contains("tpub") && !r.contains("Secret"))
    );
}
#[test]
fn failed_snapshot_commit_rolls_back_freshness_and_endpoint() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let (core, id) = setup(path.to_str().unwrap());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_save BEFORE UPDATE OF state_json ON wallets BEGIN SELECT RAISE(ABORT,'test crash'); END;").unwrap();
    let server = Server::new(Mode::Empty);
    assert_eq!(wait(&core, start(&core, &id, &server)), SyncPhase::Failed);
    drop(core);
    let core = Core::open(&path).unwrap();
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
    assert!(core.sync_endpoint(&id).unwrap().is_none());
}
