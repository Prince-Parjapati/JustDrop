//! Engine events emitted to the platform layer.
//!
//! The platform (Swift/Kotlin) registers a callback to receive these events.
//! Events are the sole communication channel from engine → platform.

use crate::peer::Peer;
use justdrop_core::types::{TransferId, TransferManifest, TransferProgress};
use std::path::PathBuf;

/// Events emitted by the engine to the platform layer.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    // ── Discovery ───────────────────────────────────────────────────
    /// A new peer was discovered via BLE or mDNS.
    PeerDiscovered(Peer),

    /// A previously discovered peer is no longer visible.
    PeerLost { device_id: String },

    /// Peer presence state changed (idle, busy, etc).
    PeerUpdated(Peer),

    // ── Transfer Lifecycle ──────────────────────────────────────────
    /// Incoming transfer request from a peer. Platform must accept/reject.
    IncomingTransferRequest {
        transfer_id: TransferId,
        peer: Peer,
        manifest: TransferManifest,
        /// Preview thumbnails (file paths), if available.
        previews: Vec<PathBuf>,
    },

    /// Transfer accepted by the remote peer.
    TransferAccepted { transfer_id: TransferId },

    /// Transfer rejected by the remote peer.
    TransferRejected {
        transfer_id: TransferId,
        reason: String,
    },

    /// Transfer progress update.
    TransferProgress(TransferProgress),

    /// Transfer completed successfully.
    TransferCompleted {
        transfer_id: TransferId,
        /// Paths to received files (for receiver) or empty (for sender).
        saved_paths: Vec<PathBuf>,
    },

    /// Transfer failed.
    TransferFailed {
        transfer_id: TransferId,
        error: String,
    },

    /// Transfer was cancelled.
    TransferCancelled {
        transfer_id: TransferId,
        reason: String,
    },

    // ── Transport ───────────────────────────────────────────────────
    /// Hotspot credentials received — platform should join this Wi-Fi.
    JoinHotspot { ssid: String, passphrase: String },

    /// Request platform to create a LocalOnlyHotspot.
    CreateHotspot,

    // ── Engine State ────────────────────────────────────────────────
    /// Engine started successfully.
    Started,

    /// Engine stopped.
    Stopped,

    /// Non-fatal error occurred.
    Error { message: String },
}

/// Callback trait for receiving engine events.
///
/// Platform layers implement this trait. The engine calls these methods
/// on a background thread — implementations must handle thread safety.
pub trait EngineEventHandler: Send + Sync {
    fn on_event(&self, event: EngineEvent);
}

/// Simple channel-based event handler for testing.
pub struct ChannelEventHandler {
    tx: tokio::sync::mpsc::UnboundedSender<EngineEvent>,
}

impl ChannelEventHandler {
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedReceiver<EngineEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl EngineEventHandler for ChannelEventHandler {
    fn on_event(&self, event: EngineEvent) {
        let _ = self.tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_handler_delivers_events() {
        let (handler, mut rx) = ChannelEventHandler::new();
        handler.on_event(EngineEvent::Started);
        handler.on_event(EngineEvent::Stopped);

        let evt = rx.try_recv().unwrap();
        assert!(matches!(evt, EngineEvent::Started));

        let evt = rx.try_recv().unwrap();
        assert!(matches!(evt, EngineEvent::Stopped));
    }
}
