//! Local disposable-fixture smoke runner; not a transaction-signing tool.
use std::{env, fs};
use tundra_core::{Core, Network};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or(
        "usage: cargo run -p tundra-core --example inspect -- <public-test-descriptor-file>",
    )?;
    let bytes = fs::read(path)?;
    if bytes.len() > 32_768 {
        return Err("descriptor file too large".into());
    }
    let payload = String::from_utf8(bytes)?;
    let core = Core::open(":memory:")?;
    let preview = core.preview_import(&payload, Network::Signet)?;
    let wallet = core.import_wallet("Disposable fixture", &payload, Network::Signet)?;
    let first = core.receive_address(&wallet.id)?;
    let second = core.receive_address(&wallet.id)?;
    assert_ne!(first.address, second.address);
    assert_eq!(wallet.total_sats, None);
    println!(
        "Imported {:?}; receive indexes {} and {}; balance unknown. No addresses printed or funded.",
        preview.policy, first.index, second.index
    );
    Ok(())
}
