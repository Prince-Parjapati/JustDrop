//! BLE GATT handshake protocol.
//!
//! After BLE discovery, devices perform a GATT-based handshake to exchange
//! full cryptographic identities and negotiate transport. This happens
//! over a custom GATT service with two characteristics:
//!
//! 1. **Handshake Write** — initiator writes `HandshakeRequest`
//! 2. **Handshake Notify** — responder sends `HandshakeResponse`
//!
//! All messages are Postcard-encoded.

use serde::{Deserialize, Serialize};

/// GATT service UUID for the JustDrop handshake.
pub const HANDSHAKE_SERVICE_UUID: &str = "7A5D3E2F-1B4C-4D8E-9F6A-0E3C5B7D9A2F";

/// GATT characteristic UUID: initiator writes request here.
pub const HANDSHAKE_WRITE_UUID: &str = "7A5D3E2F-1B4C-4D8E-9F6A-0E3C5B7D9A30";

/// GATT characteristic UUID: responder notifies response here.
pub const HANDSHAKE_NOTIFY_UUID: &str = "7A5D3E2F-1B4C-4D8E-9F6A-0E3C5B7D9A31";

/// Handshake request sent by the initiating device.
///
/// Contains the initiator's full public key and an ephemeral
/// X25519 public key for the session key exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    /// Protocol version.
    pub version: u8,
    /// Initiator's Ed25519 public key (32 bytes).
    pub public_key: [u8; 32],
    /// Ephemeral X25519 public key for DH key exchange (32 bytes).
    pub ephemeral_key: [u8; 32],
    /// Nonce for replay protection (16 bytes).
    pub nonce: [u8; 16],
    /// Ed25519 signature of (version || ephemeral_key || nonce).
    pub signature: Vec<u8>,
    /// Human-readable device name.
    pub device_name: String,
    /// Requested transport type.
    pub transport_request: TransportRequest,
}

/// Handshake response from the responder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    /// Protocol version.
    pub version: u8,
    /// Responder's Ed25519 public key (32 bytes).
    pub public_key: [u8; 32],
    /// Responder's ephemeral X25519 public key (32 bytes).
    pub ephemeral_key: [u8; 32],
    /// Nonce echoed back + responder's own nonce.
    pub nonce: [u8; 16],
    /// Ed25519 signature of (version || ephemeral_key || nonce).
    pub signature: Vec<u8>,
    /// Human-readable device name.
    pub device_name: String,
    /// Accepted transport with connection details.
    pub transport_accept: TransportAccept,
}

/// Transport negotiation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportRequest {
    /// Prefer connecting over existing shared LAN.
    SharedLan {
        /// Initiator's LAN IP address (if known).
        ip: Option<String>,
        /// QUIC port to connect to.
        port: u16,
    },
    /// Request the responder to create a LocalOnlyHotspot.
    RequestHotspot,
    /// Initiator will create a hotspot.
    OfferHotspot,
}

/// Transport negotiation acceptance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportAccept {
    /// Both devices are on the same LAN.
    SharedLan {
        /// Responder's LAN IP.
        ip: String,
        /// QUIC port.
        port: u16,
    },
    /// Responder created a hotspot. Initiator should join.
    HotspotCreated {
        /// SSID of the temporary hotspot.
        ssid: String,
        /// WPA2 passphrase.
        passphrase: String,
        /// QUIC port on the hotspot interface.
        port: u16,
    },
    /// Rejected — incompatible transports.
    Rejected {
        reason: String,
    },
}

impl HandshakeRequest {
    /// Serialize for GATT write.
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    /// Deserialize from GATT write data.
    pub fn from_bytes(data: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(data)
    }

    /// Construct the signed payload: version || ephemeral_key || nonce
    pub fn signed_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 32 + 16);
        buf.push(self.version);
        buf.extend_from_slice(&self.ephemeral_key);
        buf.extend_from_slice(&self.nonce);
        buf
    }
}

impl HandshakeResponse {
    /// Serialize for GATT notification.
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    /// Deserialize from GATT notification data.
    pub fn from_bytes(data: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(data)
    }

    /// Construct the signed payload.
    pub fn signed_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 32 + 16);
        buf.push(self.version);
        buf.extend_from_slice(&self.ephemeral_key);
        buf.extend_from_slice(&self.nonce);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_request() -> HandshakeRequest {
        HandshakeRequest {
            version: 1,
            public_key: [0xAA; 32],
            ephemeral_key: [0xBB; 32],
            nonce: [0xCC; 16],
            signature: vec![0xDD; 64],
            device_name: "TestMac".to_string(),
            transport_request: TransportRequest::SharedLan {
                ip: Some("192.168.1.10".to_string()),
                port: 42420,
            },
        }
    }

    fn dummy_response() -> HandshakeResponse {
        HandshakeResponse {
            version: 1,
            public_key: [0x11; 32],
            ephemeral_key: [0x22; 32],
            nonce: [0x33; 16],
            signature: vec![0x44; 64],
            device_name: "AndroidPhone".to_string(),
            transport_accept: TransportAccept::HotspotCreated {
                ssid: "DIRECT-JD-abc".to_string(),
                passphrase: "s3cur3pa55".to_string(),
                port: 42420,
            },
        }
    }

    #[test]
    fn request_roundtrip() {
        let req = dummy_request();
        let bytes = req.to_bytes().unwrap();
        let decoded = HandshakeRequest::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.public_key, [0xAA; 32]);
        assert_eq!(decoded.device_name, "TestMac");
    }

    #[test]
    fn response_roundtrip() {
        let resp = dummy_response();
        let bytes = resp.to_bytes().unwrap();
        let decoded = HandshakeResponse::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.device_name, "AndroidPhone");
        match decoded.transport_accept {
            TransportAccept::HotspotCreated { ssid, .. } => {
                assert_eq!(ssid, "DIRECT-JD-abc");
            }
            _ => panic!("expected HotspotCreated"),
        }
    }

    #[test]
    fn signed_payload_deterministic() {
        let req = dummy_request();
        let p1 = req.signed_payload();
        let p2 = req.signed_payload();
        assert_eq!(p1, p2);
        assert_eq!(p1.len(), 1 + 32 + 16);
    }

    #[test]
    fn gatt_payload_size_reasonable() {
        // GATT MTU is typically 512 bytes, but can be as low as 23.
        // With negotiated MTU, our handshake must fit in 512 bytes.
        let req = dummy_request();
        let bytes = req.to_bytes().unwrap();
        assert!(bytes.len() < 512, "request too large: {} bytes", bytes.len());

        let resp = dummy_response();
        let bytes = resp.to_bytes().unwrap();
        assert!(bytes.len() < 512, "response too large: {} bytes", bytes.len());
    }
}
