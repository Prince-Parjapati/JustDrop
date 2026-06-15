//! V2 wire protocol messages using Postcard binary encoding.
//!
//! This module defines the next-generation message format that replaces
//! the bincode-based messages.rs. Designed for QUIC stream transport.
//!
//! Encoding: `[u8 tag][postcard payload]`
//! All messages are self-describing via the tag byte.

use justdrop_core::types::{ChunkAck, ChunkId, TransferManifest, TransferResponse};
use serde::{Deserialize, Serialize};

/// Protocol version for v2 wire format.
pub const PROTOCOL_VERSION: u8 = 2;

/// V2 message tags.
pub mod tags {
    pub const HANDSHAKE: u8 = 0x01;
    pub const MANIFEST: u8 = 0x02;
    pub const ACCEPT: u8 = 0x03;
    pub const REJECT: u8 = 0x04;
    pub const CHUNK: u8 = 0x05;
    pub const ACK: u8 = 0x06;
    pub const RESUME: u8 = 0x07;
    pub const CANCEL: u8 = 0x08;
    pub const PRESENCE: u8 = 0x09;
    pub const COMPLETE: u8 = 0x0A;
    pub const VERIFIED: u8 = 0x0B;
    pub const PING: u8 = 0x0C;
    pub const PONG: u8 = 0x0D;
}

/// Presence state for the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Presence {
    Idle = 0,
    Available = 1,
    Receiving = 2,
    Busy = 3,
    Invisible = 4,
}

/// V2 protocol message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageV2 {
    /// Initial version negotiation.
    Handshake {
        /// Protocol version.
        version: u8,
        /// Device fingerprint (BLAKE3, first 8 bytes).
        device_id: [u8; 8],
        /// Human-readable device name.
        device_name: String,
    },

    /// Transfer manifest.
    Manifest(TransferManifest),

    /// Accept the transfer.
    Accept,

    /// Reject the transfer.
    Reject { reason: String },

    /// A chunk of file data.
    Chunk {
        id: ChunkId,
        /// Compressed flag (zstd if true).
        compressed: bool,
        /// Raw or compressed chunk bytes.
        data: Vec<u8>,
    },

    /// Chunk acknowledgement.
    Ack(ChunkAck),

    /// Resume a previously interrupted transfer.
    Resume {
        /// Transfer ID to resume.
        transfer_id: String,
        /// Bitmap of already-received chunks (compressed).
        received_chunks: Vec<u8>,
    },

    /// Cancel the current transfer.
    Cancel { reason: String },

    /// Presence update.
    PresenceUpdate(Presence),

    /// Transfer complete — all chunks sent.
    Complete {
        /// BLAKE3 hash of the entire manifest for verification.
        manifest_hash: [u8; 32],
    },

    /// Integrity verification result.
    Verified { ok: bool, error: Option<String> },

    /// Keepalive.
    Ping,

    /// Keepalive response.
    Pong,
}

impl MessageV2 {
    /// Encode to bytes: `[tag][postcard payload]`.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolEncodeError> {
        let tag = self.tag();
        let payload = postcard::to_allocvec(self)
            .map_err(|e| ProtocolEncodeError(format!("postcard encode: {e}")))?;
        let mut buf = Vec::with_capacity(1 + payload.len());
        buf.push(tag);
        buf.extend_from_slice(&payload);
        Ok(buf)
    }

    /// Decode from bytes: `[tag][postcard payload]`.
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolDecodeError> {
        if data.is_empty() {
            return Err(ProtocolDecodeError("empty message".into()));
        }
        // Postcard deserializes the enum variant from the full payload
        // (tag is redundant but useful for routing before full decode).
        postcard::from_bytes(data.get(1..).unwrap_or(&[]))
            .map_err(|e| ProtocolDecodeError(format!("postcard decode: {e}")))
    }

    /// Get the tag byte.
    pub fn tag(&self) -> u8 {
        match self {
            MessageV2::Handshake { .. } => tags::HANDSHAKE,
            MessageV2::Manifest(_) => tags::MANIFEST,
            MessageV2::Accept => tags::ACCEPT,
            MessageV2::Reject { .. } => tags::REJECT,
            MessageV2::Chunk { .. } => tags::CHUNK,
            MessageV2::Ack(_) => tags::ACK,
            MessageV2::Resume { .. } => tags::RESUME,
            MessageV2::Cancel { .. } => tags::CANCEL,
            MessageV2::PresenceUpdate(_) => tags::PRESENCE,
            MessageV2::Complete { .. } => tags::COMPLETE,
            MessageV2::Verified { .. } => tags::VERIFIED,
            MessageV2::Ping => tags::PING,
            MessageV2::Pong => tags::PONG,
        }
    }

    /// Whether this message carries file data.
    pub fn is_data(&self) -> bool {
        matches!(self, MessageV2::Chunk { .. })
    }

    /// Data payload size (for bandwidth tracking).
    pub fn data_size(&self) -> usize {
        match self {
            MessageV2::Chunk { data, .. } => data.len(),
            _ => 0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("protocol encode error: {0}")]
pub struct ProtocolEncodeError(String);

#[derive(Debug, thiserror::Error)]
#[error("protocol decode error: {0}")]
pub struct ProtocolDecodeError(String);

#[cfg(test)]
mod tests {
    use super::*;
    use justdrop_core::types::FileEntry;

    #[test]
    fn handshake_roundtrip() {
        let msg = MessageV2::Handshake {
            version: PROTOCOL_VERSION,
            device_id: [0xAA; 8],
            device_name: "TestMac".to_string(),
        };
        let encoded = msg.encode().unwrap();
        let decoded = MessageV2::decode(&encoded).unwrap();
        match decoded {
            MessageV2::Handshake { version, device_id, device_name } => {
                assert_eq!(version, 2);
                assert_eq!(device_id, [0xAA; 8]);
                assert_eq!(device_name, "TestMac");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn manifest_roundtrip() {
        let manifest = TransferManifest::new(
            vec![FileEntry {
                index: 0,
                relative_path: "photo.jpg".into(),
                size: 4_000_000,
                sha256: [0xBB; 32],
                mime_type: "image/jpeg".into(),
            }],
            "Android Phone".into(),
            256 * 1024,
        );
        let msg = MessageV2::Manifest(manifest);
        let encoded = msg.encode().unwrap();
        let decoded = MessageV2::decode(&encoded).unwrap();
        match decoded {
            MessageV2::Manifest(m) => {
                assert_eq!(m.sender_name, "Android Phone");
                assert_eq!(m.files.len(), 1);
                assert_eq!(m.files[0].size, 4_000_000);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn chunk_roundtrip() {
        let msg = MessageV2::Chunk {
            id: ChunkId {
                file_index: 0,
                chunk_offset: 1024,
            },
            compressed: true,
            data: vec![0xCC; 512],
        };
        let encoded = msg.encode().unwrap();
        let decoded = MessageV2::decode(&encoded).unwrap();
        match decoded {
            MessageV2::Chunk { id, compressed, data } => {
                assert_eq!(id.chunk_offset, 1024);
                assert!(compressed);
                assert_eq!(data.len(), 512);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn resume_roundtrip() {
        let msg = MessageV2::Resume {
            transfer_id: "abc-123".into(),
            received_chunks: vec![0xFF, 0x0F],
        };
        let encoded = msg.encode().unwrap();
        let decoded = MessageV2::decode(&encoded).unwrap();
        match decoded {
            MessageV2::Resume { transfer_id, received_chunks } => {
                assert_eq!(transfer_id, "abc-123");
                assert_eq!(received_chunks, vec![0xFF, 0x0F]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn presence_roundtrip() {
        let msg = MessageV2::PresenceUpdate(Presence::Busy);
        let encoded = msg.encode().unwrap();
        let decoded = MessageV2::decode(&encoded).unwrap();
        match decoded {
            MessageV2::PresenceUpdate(p) => assert_eq!(p, Presence::Busy),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn ping_pong() {
        for msg in [MessageV2::Ping, MessageV2::Pong] {
            let encoded = msg.encode().unwrap();
            let decoded = MessageV2::decode(&encoded).unwrap();
            assert_eq!(msg.tag(), decoded.tag());
        }
    }

    #[test]
    fn postcard_more_compact_than_bincode() {
        let msg = MessageV2::Handshake {
            version: 2,
            device_id: [0xAA; 8],
            device_name: "Test".into(),
        };
        let postcard_size = msg.encode().unwrap().len();
        // Postcard should be very compact for simple structs
        assert!(postcard_size < 30, "postcard too large: {postcard_size}");
    }
}
