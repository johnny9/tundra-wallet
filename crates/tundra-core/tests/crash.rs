//! Abrupt-process WAL recovery. This does not claim physical power-loss testing.
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    time::Duration,
};
use tundra_core::{Core, Network};

#[test]
#[ignore = "child process fixture, invoked by kill_during_write_preserves_last_commit"]
fn crash_writer() {
    let path = std::env::var("TUNDRA_CRASH_DB").unwrap();
    let core = Core::open(&path).unwrap();
    let wallet = core
        .import_wallet(
            "Crash test",
            include_str!("../../../tests/fixtures/single-sig.txt"),
            Network::Signet,
        )
        .unwrap();
    let address = core.receive_address(&wallet.id).unwrap();
    core.set_label(&wallet.id, "addr", &address.address, "Committed 🧊")
        .unwrap();
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("PRAGMA synchronous=FULL; BEGIN IMMEDIATE;")
        .unwrap();
    db.execute(
        "UPDATE wallets SET state_json='incomplete snapshot' WHERE id=?1",
        [&wallet.id],
    )
    .unwrap();
    db.execute(
        "UPDATE labels SET label='uncommitted metadata' WHERE wallet_id=?1",
        [&wallet.id],
    )
    .unwrap();
    println!("uncommitted");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn kill_during_write_preserves_last_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_writer", "--nocapture"])
        .env("TUNDRA_CRASH_DB", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == "uncommitted" {
                let _ = sender.send(());
                break;
            }
        }
    });
    let ready = receiver.recv_timeout(Duration::from_secs(10)).is_ok();
    child.kill().unwrap();
    child.wait().unwrap();
    reader.join().unwrap();
    assert!(ready, "child failed to reach write boundary");
    let core = Core::open(&path).unwrap();
    let wallet = core.wallets().unwrap().remove(0);
    assert!(wallet.total_sats.is_none());
    assert_eq!(core.receive_address(&wallet.id).unwrap().index, 1);
    let labels = core.export_labels(&wallet.id).unwrap();
    assert!(labels.contains("Committed 🧊"));
    assert!(!labels.contains("uncommitted metadata"));
    let db = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
