use thiserror::Error;

/// Errors deliberately do not include descriptors, labels, addresses, PSBTs or SQL values.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("Invalid or unsupported public descriptor")]
    Descriptor,
    #[error(
        "Only public extended keys with origins and unhardened receive/change ranges are supported"
    )]
    UnsupportedKeys,
    #[error("Only native SegWit single-sig and 2-of-3 sorted multisig are supported")]
    UnsupportedPolicy,
    #[error("Descriptor network does not match the selected network")]
    NetworkMismatch,
    #[error(
        "Receive and change must describe the same wallet with distinct /0/* and /1/* branches"
    )]
    BranchMismatch,
    #[error("Wallet already exists")]
    AlreadyExists,
    #[error("Wallet, draft or coin was not found")]
    NotFound,
    #[error("Coin is frozen, reserved, unconfirmed, immature or no longer spendable")]
    UnavailableCoin,
    #[error("Selected coins cannot fund this transaction, including its fee")]
    CannotBuild,
    #[error("Sync was cancelled; the previous wallet state was retained")]
    Cancelled,
    #[error("Sync failed: check the endpoint, network and connection, then retry")]
    SyncFailed,
    #[error("Sync exceeded a bounded response, scan or time limit")]
    SyncLimit,
    #[error("Wallet changed during sync; retry with the current state")]
    StaleSync,
    #[error("Broadcast preflight failed; this request did not submit a transaction")]
    BroadcastPreflight,
    #[error("Another sync is already in progress")]
    SyncBusy,
    #[error("Not implemented in this development milestone: {0}")]
    Unavailable(&'static str),
    #[error("Wallet database operation failed")]
    Storage,
    #[error("Protected wallet storage could not be opened; the existing files were retained")]
    StorageLocked,
    #[error("Wallet storage is in use; close other wallet sessions before upgrading storage")]
    StorageBusy,
    #[error("Wallet state is corrupt or incompatible")]
    CorruptState,
    #[error("Application state is unavailable; restart the app")]
    Poisoned,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
