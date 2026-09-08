use super::*;
use std::io::Write;

fn psbt() -> Vec<u8> {
    signing::parse_response(include_bytes!(
        "../../../tests/fixtures/hwi-signed-wpkh.psbt"
    ))
    .unwrap()
    .serialize()
}
fn bbqr_vectors() -> Vec<Vec<String>> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/qr-bbqr-vectors.json")).unwrap();
    data["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v["frames"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().into())
                .collect()
        })
        .collect()
}
fn cbor(payload: &[u8]) -> Vec<u8> {
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder.bytes(payload).unwrap();
    encoder.into_writer()
}
fn deflate(payload: &[u8]) -> Vec<u8> {
    let compressor = flate2::Compress::new_with_window_bits(flate2::Compression::best(), false, 10);
    let mut encoder = flate2::write::ZlibEncoder::new_with_compress(Vec::new(), compressor);
    encoder.write_all(payload).unwrap();
    encoder.finish().unwrap()
}
fn fountain_frame(
    sequence: u32,
    fragments: u32,
    length: u32,
    checksum: u32,
    data: &[u8],
) -> String {
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder
        .array(5)
        .unwrap()
        .u32(sequence)
        .unwrap()
        .u32(fragments)
        .unwrap()
        .u32(length)
        .unwrap()
        .u32(checksum)
        .unwrap()
        .bytes(data)
        .unwrap();
    format!(
        "ur:crypto-psbt/{sequence}-{fragments}/{}",
        ur::bytewords::encode(&encoder.into_writer(), ur::bytewords::Style::Minimal)
    )
}

#[test]
fn official_psbt_ur_vector_and_legacy_type_are_supported() {
    let vector = include_str!("../../../tests/fixtures/qr-registry-psbt.ur").trim();
    for value in [
        vector.to_owned(),
        vector.to_uppercase(),
        vector.replacen("ur:psbt/", "ur:crypto-psbt/", 1),
    ] {
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        assert_eq!(decoder.receive(&value).unwrap().phase, QrPhase::Complete);
        let payload = decoder.payload().unwrap();
        assert_eq!(
            signing::parse_response(&payload)
                .unwrap()
                .unsigned_tx
                .input
                .len(),
            2
        );
        assert_eq!(
            encode_psbt(&payload, QrFormat::Ur).unwrap(),
            [vector.replacen("ur:psbt/", "ur:crypto-psbt/", 1)]
        );
    }
}

#[test]
fn independent_bbqr_hex_base32_and_deflate_vectors_reassemble_out_of_order() {
    for frames in bbqr_vectors() {
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        for frame in frames.iter().rev() {
            let progress = decoder.receive(frame).unwrap();
            if progress.phase == QrPhase::Scanning {
                // A repeated frame is not a new fragment.
                assert_eq!(decoder.receive(frame).unwrap(), progress);
            }
        }
        assert_eq!(decoder.payload().unwrap(), psbt());
        assert_eq!(decoder.progress().resolved_fragments, frames.len() as u32);
    }
}

#[test]
fn exports_roundtrip_through_both_codecs_and_preserve_public_descriptors() {
    for format in [QrFormat::Ur, QrFormat::Bbqr] {
        let frames = encode_psbt(&psbt(), format).unwrap();
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        for frame in frames.iter().rev() {
            if decoder.receive(frame).unwrap().phase == QrPhase::Complete {
                break;
            }
        }
        assert_eq!(decoder.payload().unwrap(), psbt());
        let descriptor = include_str!("../../../tests/fixtures/two-of-three.txt");
        let frames = encode_descriptor(descriptor, Network::Signet, format).unwrap();
        let mut decoder = QrDecoder::new(QrPurpose::Descriptor {
            network: Network::Signet,
        });
        for frame in frames {
            if decoder.receive(&frame).unwrap().phase == QrPhase::Complete {
                break;
            }
        }
        assert_eq!(decoder.payload().unwrap(), descriptor.as_bytes());
    }
}

#[test]
fn fountain_redundancy_recovers_without_systematic_frames() {
    let message = cbor(&psbt());
    let mut encoder = ur::Encoder::new(&message, 35, "crypto-psbt").unwrap();
    let count = encoder.fragment_count();
    for _ in 0..count {
        encoder.next_part().unwrap();
    }
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    for _ in 0..count * 20 {
        let frame = encoder.next_part().unwrap();
        let progress = decoder.receive(&frame).unwrap();
        assert!(progress.resolved_fragments <= progress.total_fragments.unwrap());
        if progress.phase == QrPhase::Complete {
            break;
        }
    }
    assert_eq!(decoder.payload().unwrap(), psbt());
}

#[test]
fn conflicts_and_mixed_sessions_fail_closed() {
    let frames = bbqr_vectors().remove(0);
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.receive(&frames[0]).unwrap();
    let mut conflict = frames[0].clone();
    conflict.replace_range(8..10, "FF");
    assert!(decoder.receive(&conflict).is_err());
    assert_eq!(decoder.progress().phase, QrPhase::Failed);
    assert!(decoder.payload().is_err());
    assert!(decoder.receive(&frames[1]).is_err());
    let first = encode_psbt(&psbt(), QrFormat::Ur).unwrap();
    let second = encode_psbt(
        include_bytes!("../../../tests/fixtures/ledger-wpkh-two-inputs.psbt"),
        QrFormat::Ur,
    )
    .unwrap();
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.receive(&first[0]).unwrap();
    assert!(decoder.receive(&second[0]).is_err());
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.receive(&first[0]).unwrap();
    assert!(decoder.receive(&frames[0]).is_err());
}

#[test]
fn unexpected_types_networks_and_non_psbt_payloads_are_rejected() {
    let descriptor = include_str!("../../../tests/fixtures/single-sig.txt");
    let mut decoder = QrDecoder::new(QrPurpose::Descriptor {
        network: Network::Mainnet,
    });
    assert!(decoder.receive(descriptor).is_err());
    for value in [
        "ur:crypto-seed/aaaa",
        "ur:crypto-account/aaaa",
        "B$HR0100FF",
        "B$HT0100FF",
        "B$HP0100FF",
        "not a transaction",
        "B$HP0é00FF",
    ] {
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        assert!(decoder.receive(value).is_err(), "unexpected payload");
    }
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    assert!(decoder.receive(descriptor).is_err());
    let mut decoder = QrDecoder::new(QrPurpose::Descriptor {
        network: Network::Signet,
    });
    assert!(
        decoder
            .receive(include_str!("../../../tests/fixtures/qr-registry-psbt.ur").trim())
            .is_err()
    );
}

#[test]
fn cbor_shape_lengths_and_uri_indices_are_checked_before_fountain_allocation() {
    for (sequence, fragments, length, data) in [
        (1, u32::MAX, 1, vec![0]),
        (0, 1, 1, vec![0]),
        (1, 0, 1, vec![0]),
        (1, 1, u32::MAX, vec![0]),
        (1, 2, 1, vec![0]),
        (1, 2, 3, vec![0]),
        (1, 1, 0, vec![0]),
        (1, 1, 1, vec![]),
    ] {
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        assert!(
            decoder
                .receive(&fountain_frame(sequence, fragments, length, 0, &data))
                .is_err()
        );
        assert_eq!(decoder.progress().phase, QrPhase::Failed);
    }
    let frame = fountain_frame(1, 2, 2, 0, &[0]).replacen("/1-2/", "/2-2/", 1);
    assert!(
        QrDecoder::new(QrPurpose::SignedPsbt)
            .receive(&frame)
            .is_err()
    );
    for bytes in [
        vec![0x9f, 1, 1, 1, 1, 0x41, 0, 0xff],
        vec![0x84, 1, 1, 1, 1],
        vec![0x85, 1, 1, 1, 1, 0x41, 0, 0],
    ] {
        let frame = format!(
            "ur:crypto-psbt/1-1/{}",
            ur::bytewords::encode(&bytes, ur::bytewords::Style::Minimal)
        );
        assert!(
            QrDecoder::new(QrPurpose::SignedPsbt)
                .receive(&frame)
                .is_err()
        );
    }
}

#[test]
fn compressed_payloads_are_bounded_and_require_a_complete_stream() {
    let compressed = deflate(&psbt());
    assert_eq!(inflate(&compressed).unwrap(), psbt());
    for length in 0..compressed.len() {
        assert!(inflate(&compressed[..length]).is_err());
    }
    let mut trailing = compressed.clone();
    trailing.push(0);
    assert!(inflate(&trailing).is_err());
    let bomb = deflate(&vec![0; MAX_QR_PAYLOAD_BYTES + 1]);
    assert!(matches!(
        inflate(&bomb),
        Err(Error::InvalidInput(
            "QR session exceeded a size, frame or time limit"
        ))
    ));
    let frame = format!("B$ZP0100{}", BASE32_NOPAD.encode(&bomb));
    assert!(frame.len() < MAX_FRAME_CHARACTERS);
    assert!(
        QrDecoder::new(QrPurpose::SignedPsbt)
            .receive(&frame)
            .is_err()
    );
}

#[test]
fn definite_cbor_byte_strings_have_no_trailing_or_tagged_payload() {
    let bytes = cbor(&psbt());
    assert_eq!(cbor_bytes(&bytes).unwrap(), psbt());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(cbor_bytes(&trailing).is_err());
    let mut tagged = vec![0xd9, 1, 0x36];
    tagged.extend_from_slice(&bytes);
    assert!(cbor_bytes(&tagged).is_err());
    assert!(cbor_bytes(&[0x5f, 0x41, 0, 0xff]).is_err());
}

#[test]
fn cancellation_expiry_frame_and_byte_limits_discard_reassembly() {
    let frames = bbqr_vectors().remove(0);
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.receive(&frames[0]).unwrap();
    assert_eq!(decoder.cancel().phase, QrPhase::Cancelled);
    assert!(decoder.receive(&frames[1]).is_err());
    assert!(decoder.payload().is_err());
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.receive(&frames[0]).unwrap();
    decoder.started = Instant::now() - SESSION_LIFETIME - Duration::from_secs(1);
    assert_eq!(decoder.progress().phase, QrPhase::Failed);
    assert!(matches!(decoder.stream, Stream::Empty));
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    assert!(
        decoder
            .receive(&"A".repeat(MAX_FRAME_CHARACTERS + 1))
            .is_err()
    );
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    for _ in 0..MAX_FRAMES {
        decoder.receive(&frames[0]).unwrap();
    }
    assert_eq!(decoder.progress().resolved_fragments, 1);
    assert!(decoder.receive(&frames[0]).is_err());
    let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
    decoder.bytes = MAX_SCANNED_BYTES;
    assert!(decoder.receive(&frames[0]).is_err());
}

#[test]
fn invalid_hex_and_base32_are_never_silently_dropped() {
    for frame in [
        "B$HP0200GG",
        "B$2P020099999999",
        "B$ZP0200========",
        "B$HP000070",
        "B$HP020270",
        "B$HPZZ0070",
        "B$HP0200",
    ] {
        let mut decoder = QrDecoder::new(QrPurpose::SignedPsbt);
        assert!(decoder.receive(frame).is_err());
        assert!(decoder.payload().is_err());
    }
}
