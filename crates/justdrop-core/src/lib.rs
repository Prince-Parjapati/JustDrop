//! # JustDrop Core
//!
//! Shared types, configuration, error definitions, device identity,
//! trust system, and persistence layer used by all JustDrop crates.

pub mod config;
pub mod db;
pub mod error;
pub mod identity;
pub mod trust;
pub mod types;

// Re-export commonly used items at the crate root.
pub use config::Config;
pub use error::JustDropError;
pub use identity::DeviceIdentity;
pub use trust::TrustLevel;
pub use types::*;
