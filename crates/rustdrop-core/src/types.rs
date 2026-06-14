//! Core types for the RustDrop transfer protocol.
//!
//! These types are shared across all crates and define the fundamental
//! data structures for device discovery, transfer negotiation, and progress tracking.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use uuid::Uuid;

/// Protocol version. Incremented on breaking wire format changes.
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum message payload size (16 MiB). Prevents memory exhaustion from malformed frames.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Default chunk size (256 KiB).
pub const DEFAULT_CHUNK_SIZE: u32 = 256 * 1024;

/// Large file threshold (1 GiB) — above this, chunk size auto-scales.
pub const LARGE_FILE_THRESHOLD: u64 = 1024 * 1024 * 1024;

/// Unique transfer identifier.
pub type TransferId = Uuid;

/// 32-byte SHA-256 hash.
pub type Sha256Hash = [u8; 32];

/// 32-byte Curve25519 public key.
pub type PublicKey = [u8; 32];

/// 32-byte key fingerprint (BLAKE2s of public key).
pub type Fingerprint = [u8; 32];

/// Information about a discovered peer device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique device identifier (derived from public key fingerprint).
    pub id: String,
    /// Human-readable device name.
    pub name: String,
    /// Network address and port.
    pub addr: SocketAddr,
    /// Public key fingerprint for identity verification.
    pub fingerprint: Fingerprint,
    /// Platform identifier.
    pub platform: Platform,
    /// When this device was last seen.
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

/// Platform type for a peer device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    Android,
    MacOS,
    Linux,
    Windows,
    Unknown,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Android => write!(f, "Android"),
            Platform::MacOS => write!(f, "macOS"),
            Platform::Linux => write!(f, "Linux"),
            Platform::Windows => write!(f, "Windows"),
            Platform::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Transfer manifest describing all files in a transfer session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferManifest {
    /// Unique transfer session identifier.
    pub transfer_id: TransferId,
    /// Protocol version.
    pub version: u8,
    /// Files included in this transfer.
    pub files: Vec<FileEntry>,
    /// Total size of all files in bytes.
    pub total_size: u64,
    /// Chunk size in bytes.
    pub chunk_size: u32,
    /// Sender device name.
    pub sender_name: String,
    /// Sender platform.
    pub sender_platform: Platform,
}

impl TransferManifest {
    /// Create a new transfer manifest from a list of files.
    pub fn new(files: Vec<FileEntry>, sender_name: String, chunk_size: u32) -> Self {
        let total_size = files.iter().map(|f| f.size).sum();
        Self {
            transfer_id: Uuid::new_v4(),
            version: PROTOCOL_VERSION,
            files,
            total_size,
            chunk_size,
            sender_name,
            sender_platform: current_platform(),
        }
    }

    /// Total number of chunks across all files.
    pub fn total_chunks(&self) -> u64 {
        self.files.iter().map(|f| f.chunk_count(self.chunk_size)).sum()
    }
}

/// Metadata for a single file within a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File index within the transfer (0-based).
    pub index: u32,
    /// Relative path (preserves directory structure for multi-file transfers).
    pub relative_path: String,
    /// File size in bytes.
    pub size: u64,
    /// SHA-256 hash of the complete file.
    pub sha256: Sha256Hash,
    /// MIME type.
    pub mime_type: String,
}

impl FileEntry {
    /// Number of chunks needed for this file at the given chunk size.
    pub fn chunk_count(&self, chunk_size: u32) -> u64 {
        if self.size == 0 {
            return 1; // Empty files still get one chunk
        }
        (self.size + chunk_size as u64 - 1) / chunk_size as u64
    }
}

/// Response to a transfer request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferResponse {
    /// Accept the transfer.
    Accept,
    /// Reject the transfer with a reason.
    Reject(String),
    /// Resume a previously interrupted transfer from the given global chunk ID.
    ResumeAt {
        /// Bitmap of already-received chunks (compressed).
        received_chunks: Vec<u8>,
    },
}

/// State of a transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransferState {
    /// Waiting for Noise handshake to complete.
    Handshaking,
    /// Handshake complete, transfer request sent/received.
    Negotiating,
    /// Actively transferring chunks.
    Transferring,
    /// All chunks sent, verifying integrity.
    Verifying,
    /// Transfer completed successfully.
    Completed,
    /// Transfer failed.
    Failed,
    /// Transfer was cancelled.
    Cancelled,
    /// Transfer paused (connection lost, can resume).
    Paused,
}

impl fmt::Display for TransferState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferState::Handshaking => write!(f, "Handshaking"),
            TransferState::Negotiating => write!(f, "Negotiating"),
            TransferState::Transferring => write!(f, "Transferring"),
            TransferState::Verifying => write!(f, "Verifying"),
            TransferState::Completed => write!(f, "Completed"),
            TransferState::Failed => write!(f, "Failed"),
            TransferState::Cancelled => write!(f, "Cancelled"),
            TransferState::Paused => write!(f, "Paused"),
        }
    }
}

/// Progress information for an active transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    /// Transfer session ID.
    pub transfer_id: TransferId,
    /// Current state.
    pub state: TransferState,
    /// Bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total bytes to transfer.
    pub total_bytes: u64,
    /// Current file index being transferred.
    pub current_file_index: u32,
    /// Total number of files.
    pub total_files: u32,
    /// Current transfer speed in bytes per second.
    pub speed_bps: u64,
    /// Estimated time remaining in seconds.
    pub eta_secs: Option<u64>,
}

impl TransferProgress {
    /// Fraction complete (0.0 to 1.0).
    pub fn fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            return 1.0;
        }
        self.bytes_transferred as f64 / self.total_bytes as f64
    }

    /// Percentage complete (0 to 100).
    pub fn percent(&self) -> u8 {
        (self.fraction() * 100.0).min(100.0) as u8
    }
}

/// Chunk identifier within a transfer (global across all files).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId {
    /// File index.
    pub file_index: u32,
    /// Chunk offset within the file.
    pub chunk_offset: u64,
}

/// A single chunk of file data.
#[derive(Debug, Clone)]
pub struct ChunkData {
    /// Chunk identifier.
    pub id: ChunkId,
    /// Raw chunk bytes (not owned — use bytes::Bytes for zero-copy).
    pub data: bytes::Bytes,
}

/// Acknowledgement for a received chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkAck {
    /// Chunk identifier.
    pub id: ChunkId,
    /// SHA-256 of the received chunk data.
    pub sha256: Sha256Hash,
}

/// Direction of a transfer from the local device's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// We are sending files.
    Sending,
    /// We are receiving files.
    Receiving,
}

/// Callback type for progress updates.
pub type ProgressCallback = Box<dyn Fn(TransferProgress) + Send + Sync>;

/// Callback type for incoming transfer requests (returns accept/reject).
pub type TransferRequestCallback =
    Box<dyn Fn(TransferManifest) -> TransferResponse + Send + Sync>;

/// Detect the current platform.
pub fn current_platform() -> Platform {
    #[cfg(target_os = "android")]
    return Platform::Android;
    #[cfg(target_os = "macos")]
    return Platform::MacOS;
    #[cfg(target_os = "linux")]
    return Platform::Linux;
    #[cfg(target_os = "windows")]
    return Platform::Windows;
    #[cfg(not(any(
        target_os = "android",
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )))]
    return Platform::Unknown;
}

/// Compute effective chunk size based on file size.
pub fn effective_chunk_size(file_size: u64, base_chunk_size: u32) -> u32 {
    if file_size > LARGE_FILE_THRESHOLD {
        // Double the chunk size for large files to reduce per-chunk overhead
        base_chunk_size.saturating_mul(2).min(4 * 1024 * 1024) // Cap at 4 MiB
    } else {
        base_chunk_size
    }
}

/// Format byte count as human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    if bytes >= TIB {
        format!("{:.2} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_count_calculation() {
        let entry = FileEntry {
            index: 0,
            relative_path: "test.bin".into(),
            size: 1024 * 1024, // 1 MiB
            sha256: [0u8; 32],
            mime_type: "application/octet-stream".into(),
        };
        // 1 MiB / 256 KiB = 4 chunks
        assert_eq!(entry.chunk_count(DEFAULT_CHUNK_SIZE), 4);
    }

    #[test]
    fn chunk_count_with_remainder() {
        let entry = FileEntry {
            index: 0,
            relative_path: "test.bin".into(),
            size: 1024 * 1024 + 1, // 1 MiB + 1 byte
            sha256: [0u8; 32],
            mime_type: "application/octet-stream".into(),
        };
        assert_eq!(entry.chunk_count(DEFAULT_CHUNK_SIZE), 5);
    }

    #[test]
    fn empty_file_has_one_chunk() {
        let entry = FileEntry {
            index: 0,
            relative_path: "empty.txt".into(),
            size: 0,
            sha256: [0u8; 32],
            mime_type: "text/plain".into(),
        };
        assert_eq!(entry.chunk_count(DEFAULT_CHUNK_SIZE), 1);
    }

    #[test]
    fn progress_percentage() {
        let progress = TransferProgress {
            transfer_id: Uuid::new_v4(),
            state: TransferState::Transferring,
            bytes_transferred: 50,
            total_bytes: 100,
            current_file_index: 0,
            total_files: 1,
            speed_bps: 1000,
            eta_secs: Some(0),
        };
        assert_eq!(progress.percent(), 50);
        assert!((progress.fraction() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn effective_chunk_size_scales() {
        // Below threshold: same size
        assert_eq!(effective_chunk_size(500 * 1024 * 1024, DEFAULT_CHUNK_SIZE), DEFAULT_CHUNK_SIZE);
        // Above threshold: doubled
        assert_eq!(
            effective_chunk_size(2 * 1024 * 1024 * 1024, DEFAULT_CHUNK_SIZE),
            DEFAULT_CHUNK_SIZE * 2
        );
    }

    #[test]
    fn format_bytes_display() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
    }

    #[test]
    fn manifest_total_chunks() {
        let manifest = TransferManifest::new(
            vec![
                FileEntry {
                    index: 0,
                    relative_path: "a.bin".into(),
                    size: 1024 * 1024,
                    sha256: [0u8; 32],
                    mime_type: "application/octet-stream".into(),
                },
                FileEntry {
                    index: 1,
                    relative_path: "b.bin".into(),
                    size: 512 * 1024,
                    sha256: [0u8; 32],
                    mime_type: "application/octet-stream".into(),
                },
            ],
            "test-device".into(),
            DEFAULT_CHUNK_SIZE,
        );
        // 4 chunks + 2 chunks = 6
        assert_eq!(manifest.total_chunks(), 6);
    }
}
