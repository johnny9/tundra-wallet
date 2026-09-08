//! Ledger HID framing, independent of a platform USB handle. Full 64-byte reports only.
//! Padding never enters a response; short reports cannot reuse bytes from a prior read.
use crate::{Error, Result};
pub const REPORT_BYTES: usize = 64;
pub const MAX_RESPONSE_BYTES: usize = 4096;
const CHANNEL: [u8; 2] = [0x01, 0x01];
const TAG: u8 = 0x05;

fn invalid() -> Error {
    Error::InvalidInput("invalid Ledger USB report or sequence")
}

pub fn command_reports(apdu: &[u8]) -> Result<Vec<Vec<u8>>> {
    if !(5..=260).contains(&apdu.len()) || usize::from(apdu[4]) != apdu.len() - 5 {
        return Err(invalid());
    }
    let mut data = (apdu.len() as u16).to_be_bytes().to_vec();
    data.extend_from_slice(apdu);
    Ok(data
        .chunks(REPORT_BYTES - 5)
        .enumerate()
        .map(|(sequence, chunk)| {
            let mut report = vec![0; REPORT_BYTES];
            report[..2].copy_from_slice(&CHANNEL);
            report[2] = TAG;
            report[3..5].copy_from_slice(&(sequence as u16).to_be_bytes());
            report[5..5 + chunk.len()].copy_from_slice(chunk);
            report
        })
        .collect())
}

#[derive(Default)]
pub struct ResponseReader {
    next: u16,
    expected: Option<usize>,
    data: Vec<u8>,
    closed: bool,
}
impl ResponseReader {
    pub fn receive(&mut self, report: &[u8]) -> Result<Option<Vec<u8>>> {
        let result = self.receive_inner(report);
        if result.is_err() {
            self.closed = true;
            self.data.clear();
        }
        result
    }
    fn receive_inner(&mut self, report: &[u8]) -> Result<Option<Vec<u8>>> {
        if self.closed
            || report.len() != REPORT_BYTES
            || report[..2] != CHANNEL
            || report[2] != TAG
            || u16::from_be_bytes([report[3], report[4]]) != self.next
        {
            return Err(invalid());
        }
        let offset = if self.next == 0 {
            let size = usize::from(u16::from_be_bytes([report[5], report[6]]));
            if !(2..=MAX_RESPONSE_BYTES).contains(&size) {
                return Err(invalid());
            }
            self.expected = Some(size);
            self.data.reserve_exact(size);
            7
        } else {
            5
        };
        let remaining = self
            .expected
            .ok_or_else(invalid)?
            .checked_sub(self.data.len())
            .ok_or_else(invalid)?;
        self.data
            .extend_from_slice(&report[offset..offset + remaining.min(REPORT_BYTES - offset)]);
        self.next = self.next.checked_add(1).ok_or_else(invalid)?;
        if self.data.len() == self.expected.ok_or_else(invalid)? {
            self.closed = true;
            Ok(Some(std::mem::take(&mut self.data)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reports(payload: &[u8]) -> Vec<Vec<u8>> {
        let mut data = (payload.len() as u16).to_be_bytes().to_vec();
        data.extend(payload);
        data.chunks(59)
            .enumerate()
            .map(|(sequence, bytes)| {
                let mut r = vec![0xa5; 64];
                r[..3].copy_from_slice(&[1, 1, 5]);
                r[3..5].copy_from_slice(&(sequence as u16).to_be_bytes());
                r[5..5 + bytes.len()].copy_from_slice(bytes);
                r
            })
            .collect()
    }
    #[test]
    fn public_get_version_command_matches_ledger_hid_wire_header() {
        let frames = command_reports(&[0xb0, 1, 0, 0, 0]).unwrap();
        assert_eq!(&frames[0][..12], &[1, 1, 5, 0, 0, 0, 5, 0xb0, 1, 0, 0, 0]);
        assert!(frames[0][12..].iter().all(|b| *b == 0));
        assert!(command_reports(&[0xb0, 1, 0, 0, 2]).is_err());
        assert!(command_reports(&[]).is_err());
    }
    #[test]
    fn response_boundaries_ignore_padding_and_require_complete_payloads() {
        for size in [2, 56, 57, 58, 116, 117, 4096] {
            let payload: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
            let reports = reports(&payload);
            let mut reader = ResponseReader::default();
            for (index, report) in reports.iter().enumerate() {
                let result = reader.receive(report).unwrap();
                if index + 1 == reports.len() {
                    assert_eq!(result, Some(payload.clone()));
                } else {
                    assert!(result.is_none());
                }
            }
            assert!(reader.receive(&reports[0]).is_err());
        }
    }
    #[test]
    fn malformed_short_duplicate_out_of_order_and_oversized_reports_close_the_exchange() {
        let reports = reports(&[0; 120]);
        for case in 0..7 {
            let mut reader = ResponseReader::default();
            let mut bad = reports[0].clone();
            match case {
                0 => bad[0] ^= 1,
                1 => bad[2] ^= 1,
                2 => bad[4] = 1,
                3 => {
                    bad.pop();
                }
                4 => bad.push(0),
                5 => bad[5..7].copy_from_slice(&4097u16.to_be_bytes()),
                6 => bad[5..7].copy_from_slice(&1u16.to_be_bytes()),
                _ => unreachable!(),
            }
            assert!(reader.receive(&bad).is_err());
            assert!(reader.receive(&reports[0]).is_err());
        }
        let mut reader = ResponseReader::default();
        assert!(reader.receive(&reports[0]).unwrap().is_none());
        assert!(reader.receive(&reports[0]).is_err());
        assert!(reader.receive(&reports[1]).is_err());
    }
}
