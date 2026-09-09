//! Tundra's UI-independent, public-descriptor-only application core.
//! Test-network development; hardware qualification and release gates remain open.
pub mod amount;
mod backup;
pub use backup::{BackupSummary, inspect_backup, restore_backup};
mod broadcast;
pub use broadcast::{BroadcastInfo, BroadcastObservation, BroadcastRequest};
pub mod descriptor;
mod engine;
mod error;
pub mod hardware;
pub mod labels;
mod model;
pub mod qr;
mod recovery;
pub mod signing;
mod storage;
pub use storage::{StorageFormat, migrate_plaintext_storage, storage_format};
mod store_selection;
pub use store_selection::{StorageLocation, StorageRestore, selected_storage};
mod sync;
pub mod usb;
pub use engine::Core;
pub use error::{Error, Result};
pub use model::*;
pub use sync::{SyncPhase, SyncProgress};

#[cfg(test)]
mod native_fixtures;
