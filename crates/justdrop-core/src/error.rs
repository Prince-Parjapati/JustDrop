//! Error types for the JustDrop system.
//!
//! Each subsystem has its own error variant, all unified under [`JustDropError`].

use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Top-level error type encompassing all JustDrop subsystems.
#[derive(Debug, Error)]
pub enum JustDropError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error(transparent)]
    Security(#[from] SecurityError),

    #[error(transparent)]
    Network(#[from] NetworkError),

    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Configuration loading and validation errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("failed to parse configuration: {source}")]
    Parse {
        #[source]
        source: toml::de::Error,
    },

    #[error("invalid configuration value: {field} = {value}: {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },
}

/// mDNS discovery errors.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("failed to create mDNS daemon: {0}")]
    DaemonCreation(String),

    #[error("failed to register service: {0}")]
    Registration(String),

    #[error("failed to browse for services: {0}")]
    Browse(String),

    #[error("peer not found: {0}")]
    PeerNotFound(String),

    #[error("discovery service shutting down")]
    Shutdown,
}

/// Noise protocol and cryptographic errors.
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("noise handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("invalid key material: {0}")]
    InvalidKey(String),

    #[error("key storage error: {0}")]
    KeyStorage(String),

    #[error("peer identity verification failed")]
    PeerVerificationFailed,
}

/// TCP transport and connection errors.
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("connection failed to {addr}: {source}")]
    ConnectionFailed {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("connection timed out to {addr}")]
    Timeout { addr: String },

    #[error("connection reset by peer")]
    ConnectionReset,

    #[error("listener bind failed on port {port}: {source}")]
    BindFailed {
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("sendfile failed: {0}")]
    SendfileFailed(String),

    #[error("transport closed")]
    Closed,

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Wire protocol and state machine errors.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid message tag: 0x{tag:02x}")]
    InvalidTag { tag: u8 },

    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("unexpected message in state {state}: got {tag}")]
    UnexpectedMessage { state: String, tag: String },

    #[error("transfer {id} not found")]
    TransferNotFound { id: Uuid },

    #[error("transfer rejected by peer: {reason}")]
    Rejected { reason: String },

    #[error("transfer cancelled: {reason}")]
    Cancelled { reason: String },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("protocol version mismatch: local={local}, remote={remote}")]
    VersionMismatch { local: u8, remote: u8 },
}

/// File I/O, chunking, and integrity errors.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("chunk {chunk_id} integrity mismatch for transfer {transfer_id}")]
    ChunkIntegrityFailed { transfer_id: Uuid, chunk_id: u64 },

    #[error("file integrity mismatch: expected {expected}, got {actual}")]
    FileIntegrityFailed { expected: String, actual: String },

    #[error("resume state corrupted for transfer {transfer_id}")]
    ResumeStateCorrupted { transfer_id: Uuid },

    #[error("insufficient disk space: need {needed} bytes, have {available} bytes")]
    InsufficientSpace { needed: u64, available: u64 },

    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formatting() {
        let err = SecurityError::HandshakeFailed("bad key".into());
        assert_eq!(err.to_string(), "noise handshake failed: bad key");

        let err = ProtocolError::InvalidTag { tag: 0xFF };
        assert_eq!(err.to_string(), "invalid message tag: 0xff");

        let err = NetworkError::Timeout {
            addr: "192.168.1.1:42420".into(),
        };
        assert_eq!(err.to_string(), "connection timed out to 192.168.1.1:42420");
    }

    #[test]
    fn error_conversion() {
        let sec_err = SecurityError::HandshakeFailed("test".into());
        let _top: JustDropError = sec_err.into();

        let net_err = NetworkError::ConnectionReset;
        let _top: JustDropError = net_err.into();
    }
}
