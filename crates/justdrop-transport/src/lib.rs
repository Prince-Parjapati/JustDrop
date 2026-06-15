//! QUIC transport layer for JustDrop.
//!
//! Provides QUIC-based connectivity backed by Quinn.
//! Uses self-signed TLS certificates derived from the device's Ed25519 identity.
//!
//! Stream multiplexing:
//! - Stream 0: Control messages (handshake, manifest, ack)
//! - Stream 1: Metadata / previews
//! - Stream 2+: File data (one per concurrent file)

pub mod endpoint;
pub mod tls;

use bytes::Bytes;
use std::net::SocketAddr;
use thiserror::Error;

/// Well-known QUIC stream IDs.
pub const CONTROL_STREAM: u64 = 0;
pub const METADATA_STREAM: u64 = 1;
pub const FILE_STREAM_BASE: u64 = 2;

/// Send half of a bidirectional QUIC stream.
pub struct StreamSend {
    inner: quinn::SendStream,
}

/// Receive half of a bidirectional QUIC stream.
pub struct StreamRecv {
    inner: quinn::RecvStream,
}

impl StreamSend {
    pub fn new(inner: quinn::SendStream) -> Self {
        Self { inner }
    }

    /// Write all bytes to the stream.
    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.inner
            .write_all(data)
            .await
            .map_err(|e| TransportError::Write(e.to_string()))
    }

    /// Write a length-prefixed frame (u32 big-endian length + payload).
    pub async fn write_frame(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let len = (data.len() as u32).to_be_bytes();
        self.write_all(&len).await?;
        self.write_all(data).await
    }

    /// Signal that no more data will be written.
    pub async fn finish(&mut self) -> Result<(), TransportError> {
        self.inner
            .finish()
            .map_err(|e| TransportError::Write(e.to_string()))
    }
}

impl StreamRecv {
    pub fn new(inner: quinn::RecvStream) -> Self {
        Self { inner }
    }

    /// Read exactly `n` bytes.
    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), TransportError> {
        self.inner
            .read_exact(buf)
            .await
            .map_err(|e| TransportError::Read(e.to_string()))
    }

    /// Read a length-prefixed frame. Returns the payload bytes.
    pub async fn read_frame(&mut self, max_size: usize) -> Result<Bytes, TransportError> {
        let mut len_buf = [0u8; 4];
        self.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > max_size {
            return Err(TransportError::FrameTooLarge {
                size: len,
                max: max_size,
            });
        }

        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    /// Read all remaining data from the stream.
    pub async fn read_to_end(&mut self, max_size: usize) -> Result<Vec<u8>, TransportError> {
        self.inner
            .read_to_end(max_size)
            .await
            .map_err(|e| TransportError::Read(e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("write error: {0}")]
    Write(String),
    #[error("read error: {0}")]
    Read(String),
    #[error("frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: usize, max: usize },
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("bind error: {0}")]
    Bind(String),
    #[error("connection closed")]
    Closed,
}
