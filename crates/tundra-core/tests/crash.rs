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
    let protected = std::env::var_os("TUNDRA_CRASH_PROTECTED").is_some();
    let core = open(&path, protected);
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
    if protected {
        // Public storage-encryption test key, not a Bitcoin signing key.
        db.pragma_update(None, "key", format!("x'{}'", "11".repeat(32)))
            .unwrap();
    }
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
    kill_at_write_boundary(false);
}

#[test]
fn kill_during_encrypted_write_preserves_last_commit() {
    kill_at_write_boundary(true);
}

fn open(path: impl AsRef<std::path::Path>, protected: bool) -> Core {
    if protected {
        Core::open_protected(path, vec![0x11; 32]).unwrap()
    } else {
        Core::open(path).unwrap()
    }
}

fn kill_at_write_boundary(protected: bool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallet.sqlite");
    let mut command = Command::new(std::env::current_exe().unwrap());
    if protected {
        command.env("TUNDRA_CRASH_PROTECTED", "1");
    } else {
        command.env_remove("TUNDRA_CRASH_PROTECTED");
    }
    let mut child = command
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
    let core = open(&path, protected);
    let wallet = core.wallets().unwrap().remove(0);
    assert!(wallet.total_sats.is_none());
    assert_eq!(core.receive_address(&wallet.id).unwrap().index, 1);
    let labels = core.export_labels(&wallet.id).unwrap();
    assert!(labels.contains("Committed 🧊"));
    assert!(!labels.contains("uncommitted metadata"));
    let db = rusqlite::Connection::open(path).unwrap();
    if protected {
        db.pragma_update(None, "key", format!("x'{}'", "11".repeat(32)))
            .unwrap();
    }
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
