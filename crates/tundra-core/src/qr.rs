//! Ephemeral, bounded QR transport. Reassembly never authorizes a transaction or a key.
use crate::{Error, Network, Result, descriptor, signing};
use bbqr::{encode::Encoding, file_type::FileType, header::Header};
use bdk_wallet::bitcoin::hashes::{Hash, sha256};
use data_encoding::{BASE32_NOPAD, HEXUPPER};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const MAX_QR_PAYLOAD_BYTES: usize = 131_072;
pub const MAX_FRAME_CHARACTERS: usize = 4_296;
const MAX_FRAGMENTS: usize = 512;
const MAX_FRAMES: u32 = 4_096;
const MAX_SCANNED_BYTES: usize = 8 * 1_048_576;
const SESSION_LIFETIME: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrPurpose {
    SignedPsbt,
    Descriptor { network: Network },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrFormat {
    Ur,
    Bbqr,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrPhase {
    Scanning,
    Complete,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrProgress {
    pub phase: QrPhase,
    pub resolved_fragments: u32,
    pub total_fragments: Option<u32>,
}
pub struct QrMatrix {
    /// Includes a four-module white quiet zone on every edge; row-major 0/1 bytes.
    pub side: u32,
    pub modules: Vec<u8>,
}

pub fn render_frame(frame: &str) -> Result<QrMatrix> {
    if frame.is_empty() || frame.len() > MAX_FRAME_CHARACTERS {
        return Err(limit());
    }
    let encoded = if frame
        .get(..3)
        .is_some_and(|s| s.eq_ignore_ascii_case("ur:"))
    {
        frame.to_ascii_uppercase()
    } else {
        frame.to_owned()
    };
    let code = qrcode::QrCode::with_error_correction_level(encoded.as_bytes(), qrcode::EcLevel::M)
        .map_err(|_| invalid())?;
    let width = code.width();
    let side = width + 8;
    let mut modules = vec![0; side * side];
    for y in 0..width {
        for x in 0..width {
            modules[(y + 4) * side + x + 4] = u8::from(code[(x, y)] == qrcode::Color::Dark);
        }
    }
    Ok(QrMatrix {
        side: side as u32,
        modules,
    })
}
#[derive(PartialEq, Eq)]
struct UrIdentity {
    kind: String,
    fragments: usize,
    length: usize,
    checksum: u32,
    fragment_bytes: usize,
}
struct UrStream {
    identity: UrIdentity,
    decoder: ur::Decoder,
    seen: BTreeMap<u32, sha256::Hash>,
}
struct BbqrStream {
    header: Header,
    parts: BTreeMap<usize, Vec<u8>>,
    bytes: usize,
}
enum Stream {
    Empty,
    Ur(UrStream),
    Bbqr(BbqrStream),
    Payload(Vec<u8>),
}

/// One scanner session. Recreate after cancellation/failure, inactivity or a changed draft.
/// This object retains neither a database handle nor camera images.
pub struct QrDecoder {
    purpose: QrPurpose,
    started: Instant,
    frames: u32,
    bytes: usize,
    progress: QrProgress,
    stream: Stream,
}
fn invalid() -> Error {
    Error::InvalidInput("invalid, conflicting or unsupported QR payload")
}
fn limit() -> Error {
    Error::InvalidInput("QR session exceeded a size, frame or time limit")
}

impl QrDecoder {
    pub fn new(purpose: QrPurpose) -> Self {
        Self {
            purpose,
            started: Instant::now(),
            frames: 0,
            bytes: 0,
            progress: QrProgress {
                phase: QrPhase::Scanning,
                resolved_fragments: 0,
                total_fragments: None,
            },
            stream: Stream::Empty,
        }
    }
    pub fn progress(&mut self) -> QrProgress {
        if self.progress.phase == QrPhase::Scanning && self.started.elapsed() > SESSION_LIFETIME {
            self.stream = Stream::Empty;
            self.progress.phase = QrPhase::Failed;
        }
        self.progress.clone()
    }
    pub fn cancel(&mut self) -> QrProgress {
        self.stream = Stream::Empty;
        self.progress.phase = QrPhase::Cancelled;
        self.progress()
    }
    pub fn payload(&self) -> Result<Vec<u8>> {
        if self.progress.phase != QrPhase::Complete {
            return Err(Error::Unavailable("complete QR payload"));
        }
        match &self.stream {
            Stream::Payload(bytes) => Ok(bytes.clone()),
            _ => Err(Error::CorruptState),
        }
    }
    pub fn receive(&mut self, frame: &str) -> Result<QrProgress> {
        if self.progress.phase != QrPhase::Scanning {
            return Err(Error::InvalidInput("QR session is closed"));
        }
        let result = self.receive_inner(frame);
        if result.is_err() {
            self.stream = Stream::Empty;
            self.progress.phase = QrPhase::Failed;
        }
        result.map(|()| self.progress())
    }
    fn receive_inner(&mut self, frame: &str) -> Result<()> {
        if self.started.elapsed() > SESSION_LIFETIME
            || self.frames >= MAX_FRAMES
            || frame.len() > MAX_FRAME_CHARACTERS
        {
            return Err(limit());
        }
        self.frames += 1;
        self.bytes = self.bytes.checked_add(frame.len()).ok_or_else(limit)?;
        if self.bytes > MAX_SCANNED_BYTES {
            return Err(limit());
        }
        if frame.is_empty() {
            return Err(invalid());
        }
        let normalized = frame.to_ascii_lowercase();
        if normalized.starts_with("ur:") {
            self.receive_ur(&normalized)
        } else if frame.starts_with("B$") {
            self.receive_bbqr(frame)
        } else if matches!(self.stream, Stream::Empty) {
            self.finish(frame.as_bytes().to_vec(), false)
        } else {
            Err(invalid())
        }
    }
    fn finish(&mut self, bytes: Vec<u8>, binary_psbt: bool) -> Result<()> {
        if bytes.len() > MAX_QR_PAYLOAD_BYTES {
            return Err(limit());
        }
        let bytes = match self.purpose {
            QrPurpose::SignedPsbt => {
                if binary_psbt && !bytes.starts_with(b"psbt\xff") {
                    return Err(invalid());
                }
                signing::parse_response(&bytes)
                    .map_err(|_| invalid())?
                    .serialize()
            }
            QrPurpose::Descriptor { network } => {
                let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
                descriptor::preview_import(text, network).map_err(|_| invalid())?;
                bytes
            }
        };
        self.stream = Stream::Payload(bytes);
        self.progress.phase = QrPhase::Complete;
        if self.progress.total_fragments.is_none() {
            self.progress.total_fragments = Some(1);
        }
        self.progress.resolved_fragments = self.progress.total_fragments.unwrap_or(1);
        Ok(())
    }
    fn receive_ur(&mut self, frame: &str) -> Result<()> {
        let kind = frame
            .get(3..)
            .and_then(|s| s.split('/').next())
            .ok_or_else(invalid)?;
        let allowed = match self.purpose {
            QrPurpose::SignedPsbt => matches!(kind, "crypto-psbt" | "psbt" | "bytes"),
            QrPurpose::Descriptor { .. } => kind == "bytes",
        };
        if !allowed {
            return Err(invalid());
        }
        let (mode, bytes) = ur::decode(frame).map_err(|_| invalid())?;
        if mode == ur::ur::Kind::SinglePart {
            if !matches!(self.stream, Stream::Empty) {
                return Err(invalid());
            }
            return self.finish(
                cbor_bytes(&bytes)?,
                matches!(self.purpose, QrPurpose::SignedPsbt),
            );
        }
        let (identity, sequence) = ur_identity(kind, &bytes)?;
        if matches!(self.stream, Stream::Empty) {
            self.stream = Stream::Ur(UrStream {
                identity,
                decoder: ur::Decoder::default(),
                seen: BTreeMap::new(),
            });
        } else if !matches!(&self.stream, Stream::Ur(stream) if stream.identity == identity) {
            return Err(invalid());
        }
        let Stream::Ur(stream) = &mut self.stream else {
            return Err(invalid());
        };
        let hash = sha256::Hash::hash(&bytes);
        if stream.seen.get(&sequence).is_some_and(|old| old != &hash) {
            return Err(invalid());
        }
        // Bounds precede the fountain decoder's allocation and XOR elimination.
        stream.decoder.receive(frame).map_err(|_| invalid())?;
        stream.seen.insert(sequence, hash);
        self.progress.total_fragments = Some(stream.decoder.fragment_count() as u32);
        self.progress.resolved_fragments =
            stream.decoder.resolved_fragment_count().unwrap_or(0) as u32;
        if stream.decoder.complete() {
            let message = stream
                .decoder
                .message()
                .map_err(|_| invalid())?
                .ok_or_else(invalid)?;
            self.finish(
                cbor_bytes(&message)?,
                matches!(self.purpose, QrPurpose::SignedPsbt),
            )?;
        }
        Ok(())
    }
    fn receive_bbqr(&mut self, frame: &str) -> Result<()> {
        if !frame.is_ascii() {
            return Err(invalid());
        }
        let header = Header::try_from_str(frame).map_err(|_| invalid())?;
        if !(1..=MAX_FRAGMENTS).contains(&header.num_parts) {
            return Err(limit());
        }
        let allowed = match self.purpose {
            QrPurpose::SignedPsbt => header.file_type == FileType::Psbt,
            QrPurpose::Descriptor { .. } => {
                matches!(header.file_type, FileType::UnicodeText | FileType::Json)
            }
        };
        if !allowed {
            return Err(invalid());
        }
        let index = usize::from_str_radix(frame.get(6..8).ok_or_else(invalid)?, 36)
            .map_err(|_| invalid())?;
        if index >= header.num_parts {
            return Err(invalid());
        }
        let payload = frame
            .get(8..)
            .filter(|p| !p.is_empty())
            .ok_or_else(invalid)?;
        let decoded = match header.encoding {
            Encoding::Hex => HEXUPPER.decode(payload.as_bytes()),
            Encoding::Base32 | Encoding::Zlib => BASE32_NOPAD.decode(payload.as_bytes()),
        }
        .map_err(|_| invalid())?;
        if matches!(self.stream, Stream::Empty) {
            self.stream = Stream::Bbqr(BbqrStream {
                header,
                parts: BTreeMap::new(),
                bytes: 0,
            });
        }
        let Stream::Bbqr(stream) = &mut self.stream else {
            return Err(invalid());
        };
        if stream.header != header {
            return Err(invalid());
        }
        if let Some(existing) = stream.parts.get(&index) {
            if existing != &decoded {
                return Err(invalid());
            }
            return Ok(());
        }
        stream.bytes = stream.bytes.checked_add(decoded.len()).ok_or_else(limit)?;
        if stream.bytes > MAX_QR_PAYLOAD_BYTES {
            return Err(limit());
        }
        stream.parts.insert(index, decoded);
        self.progress.resolved_fragments = stream.parts.len() as u32;
        self.progress.total_fragments = Some(header.num_parts as u32);
        if stream.parts.len() == header.num_parts {
            let mut data = Vec::with_capacity(stream.bytes);
            for part in stream.parts.values() {
                data.extend_from_slice(part);
            }
            if header.encoding == Encoding::Zlib {
                data = inflate(&data)?;
            }
            self.finish(data, matches!(self.purpose, QrPurpose::SignedPsbt))?;
        }
        Ok(())
    }
}

fn cbor_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = minicbor::Decoder::new(bytes);
    // The UR type replaces the CBOR tag; only a definite byte string is accepted.
    let payload = decoder.bytes().map_err(|_| invalid())?;
    if decoder.position() != bytes.len() || payload.len() > MAX_QR_PAYLOAD_BYTES {
        return Err(invalid());
    }
    Ok(payload.to_vec())
}
fn ur_identity(kind: &str, bytes: &[u8]) -> Result<(UrIdentity, u32)> {
    let mut decoder = minicbor::Decoder::new(bytes);
    if decoder.array().map_err(|_| invalid())? != Some(5) {
        return Err(invalid());
    }
    let sequence = decoder.u32().map_err(|_| invalid())?;
    let fragments = decoder.u32().map_err(|_| invalid())? as usize;
    let length = decoder.u32().map_err(|_| invalid())? as usize;
    let checksum = decoder.u32().map_err(|_| invalid())?;
    let fragment_bytes = decoder.bytes().map_err(|_| invalid())?.len();
    if sequence == 0 || decoder.position() != bytes.len() || fragment_bytes == 0 {
        return Err(invalid());
    }
    if !(1..=MAX_FRAGMENTS).contains(&fragments) || length > MAX_QR_PAYLOAD_BYTES + 5 {
        return Err(limit());
    }
    if length <= (fragments - 1) * fragment_bytes || length > fragments * fragment_bytes {
        return Err(invalid());
    }
    Ok((
        UrIdentity {
            kind: kind.into(),
            fragments,
            length,
            checksum,
            fragment_bytes,
        },
        sequence,
    ))
}
fn inflate(bytes: &[u8]) -> Result<Vec<u8>> {
    // BBQr Z is raw DEFLATE with a 10-bit window. Fixed capacity bounds allocation;
    // StreamEnd and total_in reject truncated streams and trailing compressed data.
    let mut decoder = flate2::Decompress::new_with_window_bits(false, 10);
    let mut output = Vec::with_capacity(MAX_QR_PAYLOAD_BYTES + 1);
    let status = decoder
        .decompress_vec(bytes, &mut output, flate2::FlushDecompress::Finish)
        .map_err(|_| invalid())?;
    if output.len() > MAX_QR_PAYLOAD_BYTES {
        return Err(limit());
    }
    if status != flate2::Status::StreamEnd || decoder.total_in() != bytes.len() as u64 {
        return Err(invalid());
    }
    Ok(output)
}

/// Saved-draft callers obtain bytes through Core::export_signing_psbt first, revalidating
/// current reservations and policies. Encoding itself grants no signing authority.
pub fn encode_psbt(payload: &[u8], format: QrFormat) -> Result<Vec<String>> {
    let binary = signing::parse_response(payload)?.serialize();
    if binary.len() > MAX_QR_PAYLOAD_BYTES {
        return Err(limit());
    }
    encode_payload(&binary, format, FileType::Psbt, "crypto-psbt")
}
pub fn encode_descriptor(payload: &str, network: Network, format: QrFormat) -> Result<Vec<String>> {
    descriptor::preview_import(payload, network)?;
    encode_payload(payload.as_bytes(), format, FileType::UnicodeText, "bytes")
}
fn encode_payload(
    payload: &[u8],
    format: QrFormat,
    file_type: FileType,
    ur_type: &'static str,
) -> Result<Vec<String>> {
    if payload.is_empty() || payload.len() > MAX_QR_PAYLOAD_BYTES {
        return Err(limit());
    }
    let frames = match format {
        QrFormat::Ur => {
            let mut cbor = minicbor::Encoder::new(Vec::new());
            cbor.bytes(payload).map_err(|_| invalid())?;
            let cbor = cbor.into_writer();
            if cbor.len() <= 250 {
                vec![ur::ur::try_encode(&cbor, &ur::Type::Custom(ur_type)).map_err(|_| invalid())?]
            } else {
                let fragment_bytes = 250.max(cbor.len().div_ceil(MAX_FRAGMENTS));
                let mut encoder =
                    ur::Encoder::new(&cbor, fragment_bytes, ur_type).map_err(|_| invalid())?;
                if encoder.fragment_count() > MAX_FRAGMENTS {
                    return Err(limit());
                }
                // Systematic fragments followed by fountain redundancy; the display cycles.
                (0..encoder.fragment_count() * 2)
                    .map(|_| encoder.next_part().map_err(|_| invalid()))
                    .collect::<Result<Vec<_>>>()?
            }
        }
        QrFormat::Bbqr => {
            bbqr::split::Split::try_from_data(
                payload,
                file_type,
                bbqr::split::SplitOptions {
                    encoding: Encoding::Zlib,
                    min_split_number: 1,
                    max_split_number: MAX_FRAGMENTS,
                    min_version: bbqr::qr::Version::V05,
                    max_version: bbqr::qr::Version::V15,
                },
            )
            .map_err(|_| invalid())?
            .parts
        }
    };
    if frames.iter().any(|f| f.len() > MAX_FRAME_CHARACTERS) {
        return Err(limit());
    }
    Ok(frames)
}

#[cfg(test)]
#[path = "qr_tests.rs"]
mod tests;
