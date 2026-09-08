//! Application-owned hardware boundary. No device is advertised as supported in M1.
//! The development USB protocol is implemented separately; signing needs physical qualification.
use crate::{Error, Result};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Qr,
    Usb,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareCapability {
    VerifyAddress,
    RegisterPolicy,
    SignPsbt,
}
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    pub available: bool,
    pub reason: &'static str,
}
pub fn capabilities(_transport: Transport) -> HardwareCapabilities {
    HardwareCapabilities {
        available: false,
        reason: "No hardware model and firmware have been qualified",
    }
}
/// Fail closed. Never count responses, fake a signature, or fall back to software keys.
pub fn start_signing(_draft_id: &str, _transport: Transport) -> Result<()> {
    Err(Error::Unavailable("hardware signing"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neither_transport_claims_support() {
        assert!(!capabilities(Transport::Qr).available);
        assert!(!capabilities(Transport::Usb).available);
    }
    #[test]
    fn no_simulated_success() {
        assert!(start_signing("draft", Transport::Usb).is_err());
    }
}
