//! Wire protocol message definitions and serialization.
//!
//! All messages are serialized with bincode for compact binary encoding,
//! then encrypted and framed by the transport layer.

use justdrop_core::types::{
    ChunkAck, ChunkId, Sha256Hash, TransferManifest, TransferResponse,
};
use serde::{Deserialize, Serialize};

/// Message tag constants.
pub mod tags {
    pub const TRANSFER_REQUEST: u8 = 0x01;
    pub const TRANSFER_RESPONSE: u8 = 0x02;
    pub const CHUNK_DATA: u8 = 0x03;
    pub const CHUNK_ACK: u8 = 0x04;
    pub const TRANSFER_COMPLETE: u8 = 0x05;
    pub const TRANSFER_VERIFIED: u8 = 0x06;
    pub const PROGRESS: u8 = 0x07;
    pub const CANCEL: u8 = 0x08;
    pub const PING: u8 = 0x09;
    pub const PONG: u8 = 0x0A;
}

/// A protocol message that can be sent over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Transfer request with manifest.
    TransferRequest(TransferManifest),

    /// Response to a transfer request.
    TransferResponse(TransferResponse),

    /// A chunk of file data.
    ChunkData {
        /// Chunk identifier.
        id: ChunkId,
        /// Raw chunk bytes.
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },

    /// Acknowledgement for a received chunk.
    ChunkAck(ChunkAck),

    /// Transfer complete signal with overall manifest hash.
    TransferComplete {
        /// SHA-256 of the serialized manifest (for verification).
        manifest_hash: Sha256Hash,
    },

    /// Transfer verified response.
    TransferVerified {
        /// Whether all files passed integrity checks.
        ok: bool,
        /// Optional error message if verification failed.
        error: Option<String>,
    },

    /// Progress update.
    Progress {
        bytes_transferred: u64,
        total_bytes: u64,
    },

    /// Cancel the transfer.
    Cancel { reason: String },

    /// Keepalive ping.
    Ping,

    /// Keepalive pong.
    Pong,
}

impl Message {
    /// Get the tag byte for this message type.
    pub fn tag(&self) -> u8 {
        match self {
            Message::TransferRequest(_) => tags::TRANSFER_REQUEST,
            Message::TransferResponse(_) => tags::TRANSFER_RESPONSE,
            Message::ChunkData { .. } => tags::CHUNK_DATA,
            Message::ChunkAck(_) => tags::CHUNK_ACK,
            Message::TransferComplete { .. } => tags::TRANSFER_COMPLETE,
            Message::TransferVerified { .. } => tags::TRANSFER_VERIFIED,
            Message::Progress { .. } => tags::PROGRESS,
            Message::Cancel { .. } => tags::CANCEL,
            Message::Ping => tags::PING,
            Message::Pong => tags::PONG,
        }
    }

    /// Serialize this message to bytes (tag + bincode payload).
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let payload = bincode::serialize(self).map_err(|e| format!("serialization failed: {e}"))?;

        let mut buf = Vec::with_capacity(1 + payload.len());
        buf.push(self.tag());
        buf.extend_from_slice(&payload);
        Ok(buf)
    }

    /// Deserialize a message from bytes (tag + bincode payload).
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.is_empty() {
            return Err("empty message".into());
        }

        // The tag is embedded in the bincode serialization of the enum,
        // so we deserialize the whole thing.
        bincode::deserialize(data).map_err(|e| format!("deserialization failed: {e}"))
    }

    /// Check if this is a data-bearing message (for bandwidth tracking).
    pub fn is_data(&self) -> bool {
        matches!(self, Message::ChunkData { .. })
    }

    /// Get the data size of this message (for progress tracking).
    pub fn data_size(&self) -> usize {
        match self {
            Message::ChunkData { data, .. } => data.len(),
            _ => 0,
        }
    }
}

/// Serde helper for Vec<u8> fields to use efficient byte serialization.
mod serde_bytes {
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde::Serialize::serialize(bytes, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use justdrop_core::types::FileEntry;

    #[test]
    fn message_encode_decode_roundtrip() {
        let manifest = TransferManifest::new(
            vec![FileEntry {
                index: 0,
                relative_path: "test.txt".into(),
                size: 1024,
                sha256: [0xAA; 32],
                mime_type: "text/plain".into(),
            }],
            "Test Device".into(),
            256 * 1024,
        );

        let msg = Message::TransferRequest(manifest);
        let encoded = msg.encode().unwrap();
        let decoded = Message::decode(&encoded[1..]).unwrap(); // Skip our manual tag byte for decode

        match decoded {
            Message::TransferRequest(m) => {
                assert_eq!(m.sender_name, "Test Device");
                assert_eq!(m.files.len(), 1);
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn chunk_data_roundtrip() {
        let msg = Message::ChunkData {
            id: ChunkId {
                file_index: 0,
                chunk_offset: 42,
            },
            data: vec![0xBB; 256],
        };

        let encoded = msg.encode().unwrap();
        let decoded = Message::decode(&encoded[1..]).unwrap();

        match decoded {
            Message::ChunkData { id, data } => {
                assert_eq!(id.chunk_offset, 42);
                assert_eq!(data.len(), 256);
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn ping_pong_roundtrip() {
        let ping = Message::Ping;
        let encoded = ping.encode().unwrap();
        let decoded = Message::decode(&encoded[1..]).unwrap();
        assert!(matches!(decoded, Message::Ping));
    }
}
