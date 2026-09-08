#![no_main]
use libfuzzer_sys::fuzz_target;
use tundra_core::{Network, amount, descriptor, labels};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = amount::parse_btc(text);
        let _ = amount::parse_fee_rate(text);
        let _ = labels::parse_labels(text);
        // Repair a mutated descriptor's checksum so fuzzing also reaches typed key/
        // derivation validation instead of stopping at checksum rejection.
        let body = text.split('#').next().unwrap_or("");
        if body.len() < descriptor::MAX_DESCRIPTOR_BYTES - 9
            && let Ok(checksum) = bdk_wallet::descriptor::checksum::calc_checksum(body)
        {
            let checked = format!("{body}#{checksum}");
            let _ = descriptor::preview_import(&checked, Network::Signet);
        }
        for network in [Network::Signet, Network::Regtest, Network::Mainnet] {
            if let Ok(preview) = descriptor::preview_import(text, network) {
                assert!(!preview.receive_descriptor.contains("prv"));
                assert!(!preview.change_descriptor.contains("prv"));
                assert_ne!(preview.receive_descriptor, preview.change_descriptor);
                let pair = format!(
                    "{}\n{}",
                    preview.receive_descriptor, preview.change_descriptor
                );
                assert_eq!(
                    descriptor::preview_import(&pair, network).unwrap().id,
                    preview.id
                );
            }
        }
    }
});
