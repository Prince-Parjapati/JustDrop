//! # RustDrop Security
//!
//! Noise protocol framework implementation for RustDrop, providing:
//! - Curve25519 key pair management with secure storage
//! - Noise_XX mutual authentication handshake
//! - ChaCha20-Poly1305 encrypted transport sessions

pub mod handshake;
pub mod keys;
pub mod session;

pub use handshake::{NoiseInitiator, NoiseResponder};
pub use keys::IdentityKeys;
pub use session::NoiseSession;
