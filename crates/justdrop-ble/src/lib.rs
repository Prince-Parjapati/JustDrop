//! BLE advertisement and handshake protocol definitions for JustDrop.
//!
//! This crate defines the data structures and serialization formats used
//! for Bluetooth Low Energy communication. It contains NO platform-specific
//! BLE code — that lives in the native Swift/Kotlin layers.
//!
//! Rust owns the payload format. Platforms own the radio.

pub mod advertisement;
pub mod handshake;
pub mod hotspot;
