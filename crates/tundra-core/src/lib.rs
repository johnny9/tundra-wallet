//! Tundra's UI-independent, public-descriptor-only application core.
//! Test-network development; hardware qualification and release gates remain open.
pub mod amount;
pub mod descriptor;
mod engine;
mod error;
pub mod hardware;
pub mod labels;
mod model;
pub mod signing;
mod sync;
pub use engine::Core;
pub use error::{Error, Result};
pub use model::*;
pub use sync::{SyncPhase, SyncProgress};
