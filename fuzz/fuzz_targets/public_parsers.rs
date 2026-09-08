#![no_main]
use libfuzzer_sys::fuzz_target;
use tundra_core::{Network, amount, descriptor, labels};

fuzz_target!(|data: &[u8]| {
    // USB framing receives arbitrary report sequences and short final reads. A failed or
    // complete reader must never accept a subsequent response in the same exchange.
    let mut reader = tundra_core::usb::framing::ResponseReader::default();
    for report in data.chunks(64).take(72) {
        match reader.receive(report) {
            Ok(Some(payload)) => {
                assert!((2..=4096).contains(&payload.len()));
                assert!(reader.receive(report).is_err());
                break;
            }
            Err(_) => {
                assert!(reader.receive(report).is_err());
                break;
            }
            Ok(None) => {}
        }
    }
    let _ = tundra_core::usb::framing::command_reports(data);
    if let Ok(psbt) = tundra_core::signing::parse_response(data) {
        assert_eq!(psbt.version, 0);
        assert_eq!(
            tundra_core::signing::parse_response(&psbt.serialize()).unwrap(),
            psbt
        );
    }
    if let Ok(text) = std::str::from_utf8(data) {
        for purpose in [
            tundra_core::qr::QrPurpose::SignedPsbt,
            tundra_core::qr::QrPurpose::Descriptor {
                network: Network::Signet,
            },
        ] {
            let mut decoder = tundra_core::qr::QrDecoder::new(purpose);
            // Newlines model sequential scanner frames without unbounded per-case work.
            for frame in text.split('\n').take(32) {
                match decoder.receive(frame) {
                    Ok(progress) if progress.phase == tundra_core::qr::QrPhase::Complete => {
                        let payload = decoder.payload().unwrap();
                        assert!(payload.len() <= tundra_core::qr::MAX_QR_PAYLOAD_BYTES);
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
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
