//! Real Bitcoin Core integration. Run explicitly with scripts/check-regtest.sh.
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tundra_core::{CoinStatus, Core, DraftRequest, Network, Payment, SyncPhase};

struct Node {
    node: Child,
    server: Option<Child>,
    dir: tempfile::TempDir,
}
impl Drop for Node {
    fn drop(&mut self) {
        if let Some(server) = &mut self.server {
            let _ = server.kill();
            let _ = server.wait();
        }
        let _ = self.node.kill();
        let _ = self.node.wait();
    }
}
impl Node {
    fn rpc(&self, method: &str, args: &[&str]) -> Value {
        let output = Command::new("bitcoin-cli")
            .arg(format!("-datadir={}", self.dir.path().display()))
            .args(["-regtest", method])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "RPC {method} failed");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            Value::String(String::from_utf8(output.stdout).unwrap().trim().into())
        })
    }
    fn start() -> (Self, String) {
        let dir = tempfile::tempdir().unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let node = Command::new("bitcoind")
            .arg(format!("-datadir={}", dir.path().display()))
            .arg(format!("-rpcport={port}"))
            .args([
                "-regtest",
                "-server",
                "-listen=0",
                "-disablewallet",
                "-nosettings",
                "-persistmempool=0",
                "-fallbackfee=0.00001",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("bitcoind required");
        // bitcoin-cli and the facade share only a local test RPC port/cookie.
        std::fs::write(
            dir.path().join("bitcoin.conf"),
            format!("[regtest]\nrpcport={port}\n"),
        )
        .unwrap();
        let mut result = Self {
            node,
            server: None,
            dir,
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        while !result.dir.path().join("regtest/.cookie").exists() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(50));
        }
        let ready = Command::new("bitcoin-cli")
            .arg(format!("-datadir={}", result.dir.path().display()))
            .args(["-regtest", "-rpcwait", "getblockcount"])
            .output()
            .unwrap();
        assert!(ready.status.success());
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let port_file = result.dir.path().join("esplora-port");
        result.server = Some(
            Command::new("python3")
                .arg(root.join("tests/esplora_regtest.py"))
                .arg("--datadir")
                .arg(result.dir.path())
                .arg("--port-file")
                .arg(&port_file)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        while !port_file.exists() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(50));
        }
        let source = format!(
            "http://127.0.0.1:{}",
            std::fs::read_to_string(port_file).unwrap()
        );
        (result, source)
    }
}
fn sync(core: &Core, id: &str, endpoint: &str) {
    let op = core.prepare_sync(id, endpoint, true).unwrap();
    core.run_sync(op.id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let progress = core.sync_progress(op.id).unwrap();
        match progress.phase {
            SyncPhase::Complete => return,
            SyncPhase::Failed | SyncPhase::Cancelled => panic!("sync failed: {:?}", progress.error),
            _ => {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
fn import(core: &Core) -> String {
    core.import_wallet(
        "Regtest",
        include_str!("../../../tests/fixtures/single-sig.txt"),
        Network::Regtest,
    )
    .unwrap()
    .id
}
#[test]
#[ignore = "requires local Bitcoin Core and loopback sockets; scripts/check-regtest.sh"]
fn real_chain_maturity_persistence_shorter_reorg_and_draft_invalidation() {
    let (node, endpoint) = Node::start();
    let file = node.dir.path().join("wallet.sqlite");
    let core = Core::open(&file).unwrap();
    let id = import(&core);
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
    sync(&core, &id, &endpoint);
    assert_eq!(core.wallets().unwrap()[0].total_sats, Some(0));
    let address = core.receive_address(&id).unwrap().address;
    let elsewhere = core
        .preview_import(
            include_str!("../../../tests/fixtures/two-of-three.txt"),
            Network::Regtest,
        )
        .unwrap()
        .first_address;
    let first = node.rpc("generatetoaddress", &["1", &address])[0]
        .as_str()
        .unwrap()
        .to_owned();
    node.rpc("generatetoaddress", &["98", &elsewhere]);
    sync(&core, &id, &endpoint);
    let coin = core.coins(&id).unwrap().remove(0);
    assert_eq!(coin.status, CoinStatus::Immature);
    assert_eq!(core.wallets().unwrap()[0].available_sats, Some(0));
    node.rpc("generatetoaddress", &["1", &elsewhere]);
    sync(&core, &id, &endpoint);
    assert_eq!(core.coins(&id).unwrap()[0].status, CoinStatus::Available);
    core.set_label(&id, "output", &coin.outpoint, "Mined 🧊")
        .unwrap();
    let draft = core
        .create_draft(DraftRequest {
            wallet_id: id.clone(),
            payment: Payment::Send {
                address: elsewhere.clone(),
                sats: 100_000,
            },
            selected_outpoints: Some(vec![coin.outpoint.clone()]),
            fee_sat_per_kwu: 500,
            label: "Reorg test".into(),
        })
        .unwrap();
    core.set_frozen(&id, &coin.outpoint, true).unwrap();
    let psbt = core.export_unsigned_psbt(&id, &draft.id).unwrap();
    drop(core);
    let core = Core::open(&file).unwrap();
    assert_eq!(core.export_unsigned_psbt(&id, &draft.id).unwrap(), psbt);
    assert_eq!(core.coins(&id).unwrap()[0].label, "Mined 🧊");
    // Truncate to genesis: the old confirmation must not remain canonical.
    node.rpc("invalidateblock", &[&first]);
    sync(&core, &id, &endpoint);
    assert_eq!(core.wallets().unwrap()[0].total_sats, Some(0));
    assert_eq!(core.drafts(&id).unwrap()[0].state, "invalidated");
    assert!(core.export_unsigned_psbt(&id, &draft.id).is_err());
    // Reinstating the original branch restores durable user metadata, not draft approval.
    node.rpc("reconsiderblock", &[&first]);
    sync(&core, &id, &endpoint);
    assert_eq!(core.coins(&id).unwrap()[0].status, CoinStatus::Frozen);
    assert_eq!(core.coins(&id).unwrap()[0].label, "Mined 🧊");
    assert_eq!(core.drafts(&id).unwrap()[0].state, "invalidated");
    core.discard_draft(&id, &draft.id).unwrap();
    assert_eq!(core.coins(&id).unwrap()[0].status, CoinStatus::Frozen);
    // Replace the funded branch with a different, longer chain.
    node.rpc("invalidateblock", &[&first]);
    node.rpc("generatetoaddress", &["102", &elsewhere]);
    sync(&core, &id, &endpoint);
    assert!(core.coins(&id).unwrap().is_empty());
    assert_eq!(core.wallets().unwrap()[0].total_sats, Some(0));
    assert_eq!(
        core.sync_endpoint(&id).unwrap().as_deref(),
        Some(endpoint.as_str())
    );
    assert!(Path::new(&file).is_file());
}
