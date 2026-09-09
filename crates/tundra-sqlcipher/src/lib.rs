//! Build boundary for the pinned SQLCipher C implementation. No wallet or key API.
// Keep the native cryptographic provider linked into downstream static/shared libraries.
#[cfg(not(target_vendor = "apple"))]
extern crate openssl_sys as _;

pub const VERSION: &str = "4.19.0 community";
pub const SQLITE_VERSION: &str = "3.53.4";
pub const PROVIDER: &str = if cfg!(target_vendor = "apple") {
    "commoncrypto"
} else {
    "openssl"
};
