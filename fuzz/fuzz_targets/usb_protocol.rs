#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;
use tundra_core::{
    Core, Network,
    usb::{Operation, State, UsbSession},
};

struct Fixture {
    core: Core,
    wallet: String,
    fingerprint: [u8; 4],
    xpub: Vec<u8>,
}
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let descriptor = include_str!("../../tests/fixtures/two-of-three.txt");
        let core = Core::open(":memory:").unwrap();
        let wallet = core
            .import_wallet("Public USB fuzz fixture", descriptor, Network::Signet)
            .unwrap()
            .id;
        let origin = descriptor.split('[').nth(1).unwrap();
        let mut fingerprint = [0; 4];
        for (i, b) in fingerprint.iter_mut().enumerate() {
            *b = u8::from_str_radix(&origin[i * 2..i * 2 + 2], 16).unwrap();
        }
        let xpub = origin
            .split(']')
            .nth(1)
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .as_bytes()
            .to_vec();
        Fixture {
            core,
            wallet,
            fingerprint,
            xpub,
        }
    })
}
fn response(session: &mut UsbSession, payload: &[u8]) -> tundra_core::Result<()> {
    let step = session.progress().step;
    let mut bytes = (payload.len() as u16).to_be_bytes().to_vec();
    bytes.extend(payload);
    for (sequence, part) in bytes.chunks(59).enumerate() {
        let mut report = [0; 64];
        report[..3].copy_from_slice(&[1, 1, 5]);
        report[3..5].copy_from_slice(&(sequence as u16).to_be_bytes());
        report[5..5 + part.len()].copy_from_slice(part);
        session.receive(step, &report)?;
    }
    Ok(())
}
fuzz_target!(|data: &[u8]| {
    let Some((&stage, data)) = data.split_first() else {
        return;
    };
    let f = fixture();
    let mut session = f
        .core
        .prepare_usb(&f.wallet, Operation::RegisterPolicy)
        .unwrap();
    let stage = stage % 4;
    if stage >= 1 {
        response(
            &mut session,
            b"\x01\x0cBitcoin Test\x052.4.1\x01\x00\x90\x00",
        )
        .unwrap();
    }
    if stage >= 2 {
        response(
            &mut session,
            &[f.fingerprint.as_slice(), &[0x90, 0]].concat(),
        )
        .unwrap();
    }
    if stage >= 3 {
        response(&mut session, &[f.xpub.as_slice(), &[0x90, 0]].concat()).unwrap();
    }
    // Correct HID headers make mutations reach each APDU parser, including Merkle requests.
    // Multiple bounded responses exercise continuations; no real device or signing key exists.
    for part in data.chunks(256).take(16) {
        let Some((&status, payload)) = part.split_first() else {
            break;
        };
        let status = match status % 3 {
            0 => [0x90, 0],
            1 => [0xe0, 0],
            _ => [0x69, 0x85],
        };
        if response(&mut session, &[payload, &status].concat()).is_err() {
            assert_eq!(session.progress().state, State::Failed);
            break;
        }
        if session.progress().state != State::Waiting {
            break;
        }
    }
    assert!(session.progress().packets.iter().all(|p| p.len() == 64));
    assert!(session.cancel().packets.is_empty());
    assert!(f.core.wallets().unwrap()[0].total_sats.is_none());
    assert!(f.core.drafts(&f.wallet).unwrap().is_empty());
});
