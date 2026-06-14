//! Message codec for framing protocol messages over the secure transport.
//!
//! Handles serialization/deserialization and multiplexing of message types
//! through the encrypted transport layer.

use crate::messages::Message;
use rustdrop_core::error::ProtocolError;
use rustdrop_network::SecureTransport;
use tracing::{debug, trace, warn};

/// Protocol codec wrapping a `SecureTransport` for message-level I/O.
pub struct ProtocolCodec {
    transport: SecureTransport,
}

impl ProtocolCodec {
    /// Create a new protocol codec from an established secure transport.
    pub fn new(transport: SecureTransport) -> Self {
        Self { transport }
    }

    /// Send a protocol message (serialized, encrypted, and framed).
    pub async fn send(&mut self, message: &Message) -> Result<(), ProtocolError> {
        let data = bincode::serialize(message)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?;

        self.transport.send(&data).await.map_err(|e| {
            ProtocolError::Serialization(format!("transport send failed: {e}"))
        })?;

        trace!(tag = message.tag(), size = data.len(), "sent message");
        Ok(())
    }

    /// Receive the next protocol message (read, decrypted, deserialized).
    ///
    /// Returns `None` if the connection was cleanly closed.
    pub async fn recv(&mut self) -> Result<Option<Message>, ProtocolError> {
        let data = match self.transport.recv().await {
            Ok(Some(data)) => data,
            Ok(None) => return Ok(None),
            Err(e) => {
                return Err(ProtocolError::Deserialization(format!(
                    "transport recv failed: {e}"
                )));
            }
        };

        let message: Message = bincode::deserialize(&data)
            .map_err(|e| ProtocolError::Deserialization(e.to_string()))?;

        trace!(tag = message.tag(), size = data.len(), "received message");
        Ok(Some(message))
    }

    /// Send a raw handshake message (unencrypted, for Noise handshake).
    pub async fn send_handshake(&mut self, data: &[u8]) -> Result<(), ProtocolError> {
        self.transport.send_raw(data).await.map_err(|e| {
            ProtocolError::Serialization(format!("handshake send failed: {e}"))
        })
    }

    /// Receive a raw handshake message (unencrypted).
    pub async fn recv_handshake(&mut self) -> Result<Vec<u8>, ProtocolError> {
        self.transport.recv_raw().await.map_err(|e| {
            ProtocolError::Deserialization(format!("handshake recv failed: {e}"))
        })
    }

    /// Flush the transport.
    pub async fn flush(&mut self) -> Result<(), ProtocolError> {
        self.transport.flush().await.map_err(|e| {
            ProtocolError::Serialization(format!("flush failed: {e}"))
        })
    }

    /// Shutdown the transport.
    pub async fn shutdown(&mut self) -> Result<(), ProtocolError> {
        self.transport.shutdown().await.map_err(|e| {
            ProtocolError::Serialization(format!("shutdown failed: {e}"))
        })
    }

    /// Get mutable access to the underlying transport.
    pub fn transport_mut(&mut self) -> &mut SecureTransport {
        &mut self.transport
    }

    /// Consume self and return the underlying transport.
    pub fn into_transport(self) -> SecureTransport {
        self.transport
    }
}
