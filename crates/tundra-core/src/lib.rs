//! Tundra's UI-independent, public-descriptor-only application core.
//! Development milestone: no chain backend, signature acceptance, finalization, or broadcast.
pub mod amount;
pub mod descriptor;
mod engine;
mod error;
pub mod hardware;
pub mod labels;
mod model;
pub use engine::Core;
pub use error::{Error,Result};
pub use model::*;
