//! # JustDrop Security
//!
//! Cryptographic framework for JustDrop, providing:
//! - Ed25519 identity signing/verification (ring)
//! - X25519 ephemeral key exchange for session keys
//! - ChaCha20-Poly1305 AEAD encryption
//! - HKDF-SHA256 key derivation
//!
//! Legacy Noise_XX modules retained for backward compatibility.

pub mod crypto;
pub mod handshake;
pub mod keys;
pub mod session;

pub use crypto::{CryptoError, KeyExchangeInitiator, SessionCipher, SessionKeys};
pub use handshake::{NoiseInitiator, NoiseResponder};
pub use keys::IdentityKeys;
pub use session::NoiseSession;
