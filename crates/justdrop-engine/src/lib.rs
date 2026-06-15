//! # JustDrop Engine
//!
//! Central orchestrator that manages the full lifecycle:
//! Discovery → Handshake → Transport Negotiation → Transfer → Cleanup
//!
//! The engine is event-driven. Platform layers (Swift/Kotlin) interact
//! with it through a callback-based interface. The engine owns:
//! - Device identity
//! - Peer database
//! - Active sessions
//! - Transfer queue

pub mod events;
pub mod peer;
pub mod session;
pub mod engine;

pub use engine::Engine;
pub use events::EngineEvent;
pub use peer::Peer;
pub use session::TransferSession;
