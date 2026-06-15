//! # JustDrop Core
//!
//! Shared types, configuration, and error definitions used by all JustDrop crates.
//!
//! This crate contains no business logic — it exists solely to provide a common
//! vocabulary for the rest of the workspace.

pub mod config;
pub mod error;
pub mod types;

// Re-export commonly used items at the crate root.
pub use config::Config;
pub use error::JustDropError;
pub use types::*;
