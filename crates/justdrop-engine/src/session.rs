//! Transfer session management.
//!
//! A TransferSession tracks the lifecycle of a single file transfer
//! between two devices, from negotiation through completion or failure.

use justdrop_core::types::{TransferId, TransferManifest, TransferState};
use std::path::PathBuf;
use uuid::Uuid;

/// A single transfer session between two peers.
#[derive(Debug)]
pub struct TransferSession {
    /// Unique transfer identifier.
    pub id: TransferId,
    /// Peer device fingerprint.
    pub peer_fingerprint: String,
    /// Peer device name.
    pub peer_name: String,
    /// Transfer direction.
    pub direction: Direction,
    /// Current state.
    pub state: TransferState,
    /// Transfer manifest.
    pub manifest: Option<TransferManifest>,
    /// Files to send (for outgoing).
    pub source_paths: Vec<PathBuf>,
    /// Destination directory (for incoming).
    pub dest_dir: Option<PathBuf>,
    /// Bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total bytes.
    pub total_bytes: u64,
    /// Transfer speed in bytes/second.
    pub speed_bps: u64,
    /// When the session was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the session last had activity.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Send,
    Receive,
}

impl TransferSession {
    /// Create a new outgoing transfer session.
    pub fn new_send(peer_fingerprint: String, peer_name: String, paths: Vec<PathBuf>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            peer_fingerprint,
            peer_name,
            direction: Direction::Send,
            state: TransferState::Negotiating,
            manifest: None,
            source_paths: paths,
            dest_dir: None,
            bytes_transferred: 0,
            total_bytes: 0,
            speed_bps: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new incoming transfer session.
    pub fn new_receive(
        transfer_id: TransferId,
        peer_fingerprint: String,
        peer_name: String,
        manifest: TransferManifest,
        dest_dir: PathBuf,
    ) -> Self {
        let now = chrono::Utc::now();
        let total = manifest.total_size;
        Self {
            id: transfer_id,
            peer_fingerprint,
            peer_name,
            direction: Direction::Receive,
            state: TransferState::Negotiating,
            manifest: Some(manifest),
            source_paths: Vec::new(),
            dest_dir: Some(dest_dir),
            bytes_transferred: 0,
            total_bytes: total,
            speed_bps: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update progress.
    pub fn update_progress(&mut self, bytes: u64, speed: u64) {
        self.bytes_transferred = bytes;
        self.speed_bps = speed;
        self.state = TransferState::Transferring;
        self.updated_at = chrono::Utc::now();
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.state = TransferState::Completed;
        self.updated_at = chrono::Utc::now();
    }

    /// Mark as failed.
    pub fn fail(&mut self) {
        self.state = TransferState::Failed;
        self.updated_at = chrono::Utc::now();
    }

    /// Mark as cancelled.
    pub fn cancel(&mut self) {
        self.state = TransferState::Cancelled;
        self.updated_at = chrono::Utc::now();
    }

    /// Progress as percentage (0-100).
    pub fn percent(&self) -> u8 {
        if self.total_bytes == 0 {
            return 100;
        }
        ((self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0).min(100.0) as u8
    }

    /// Estimated time remaining in seconds.
    pub fn eta_secs(&self) -> Option<u64> {
        if self.speed_bps == 0 || self.bytes_transferred >= self.total_bytes {
            return None;
        }
        let remaining = self.total_bytes - self.bytes_transferred;
        Some(remaining / self.speed_bps)
    }

    /// Whether this session is terminal (completed, failed, or cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TransferState::Completed | TransferState::Failed | TransferState::Cancelled
        )
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Send => write!(f, "send"),
            Direction::Receive => write!(f, "receive"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use justdrop_core::types::FileEntry;

    #[test]
    fn send_session_lifecycle() {
        let mut session = TransferSession::new_send(
            "abc123".into(),
            "Phone".into(),
            vec![PathBuf::from("/tmp/test.txt")],
        );
        assert_eq!(session.state, TransferState::Negotiating);
        assert_eq!(session.direction, Direction::Send);
        assert!(!session.is_terminal());

        session.update_progress(500, 1000);
        assert_eq!(session.state, TransferState::Transferring);

        session.complete();
        assert_eq!(session.state, TransferState::Completed);
        assert!(session.is_terminal());
    }

    #[test]
    fn receive_session_progress() {
        let manifest = TransferManifest::new(
            vec![FileEntry {
                index: 0,
                relative_path: "photo.jpg".into(),
                size: 1_000_000,
                sha256: [0; 32],
                mime_type: "image/jpeg".into(),
            }],
            "Sender".into(),
            256 * 1024,
        );
        let mut session = TransferSession::new_receive(
            Uuid::new_v4(),
            "xyz789".into(),
            "MacBook".into(),
            manifest,
            PathBuf::from("/tmp/justdrop"),
        );
        assert_eq!(session.total_bytes, 1_000_000);
        assert_eq!(session.percent(), 0);

        session.update_progress(500_000, 100_000);
        assert_eq!(session.percent(), 50);
        assert_eq!(session.eta_secs(), Some(5));

        session.update_progress(1_000_000, 100_000);
        assert_eq!(session.percent(), 100);
    }

    #[test]
    fn cancel_is_terminal() {
        let mut session = TransferSession::new_send(
            "abc".into(),
            "Dev".into(),
            vec![],
        );
        session.cancel();
        assert!(session.is_terminal());
        assert_eq!(session.state, TransferState::Cancelled);
    }
}
