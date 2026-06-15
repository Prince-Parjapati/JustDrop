//! BLE advertisement payload format.
//!
//! BLE advertisements are capped at 31 bytes (legacy) or 255 bytes (extended).
//! We use Postcard binary encoding to keep payloads minimal.
//!
//! The advertisement contains just enough information for discovery.
//! Full identity exchange happens over GATT during the handshake phase.

use serde::{Deserialize, Serialize};

/// JustDrop BLE service UUID (128-bit).
///
/// Generated once, hardcoded forever. Used by both platforms to filter
/// scan results to only JustDrop devices.
pub const SERVICE_UUID: &str = "7A5D3E2F-1B4C-4D8E-9F6A-0E3C5B7D9A1F";

/// BLE advertisement manufacturer data company ID.
/// Using 0xFFFF (reserved for testing/development).
pub const MANUFACTURER_ID: u16 = 0xFFFF;

/// Magic bytes to identify JustDrop advertisements.
pub const MAGIC: [u8; 2] = [0x4A, 0x44]; // "JD"

/// Capability flags advertised over BLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities(u8);

impl Capabilities {
    pub const FILE_TRANSFER: u8 = 0x01;
    pub const CLIPBOARD_SYNC: u8 = 0x02;
    pub const FOLDER_TRANSFER: u8 = 0x04;

    pub fn new() -> Self {
        Self(Self::FILE_TRANSFER | Self::FOLDER_TRANSFER)
    }

    pub fn has(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Available transport types the device can negotiate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransportHint {
    /// Device is on a shared LAN (prefer direct QUIC).
    SharedLan = 0,
    /// Device can create a LocalOnlyHotspot (Android).
    LocalHotspot = 1,
    /// Device can join a hotspot (macOS).
    HotspotClient = 2,
}

/// Presence state advertised over BLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PresenceState {
    Idle = 0,
    Available = 1,
    Receiving = 2,
    Busy = 3,
    Invisible = 4,
}

impl Default for PresenceState {
    fn default() -> Self {
        PresenceState::Available
    }
}

/// Compact BLE advertisement payload.
///
/// Serialized with Postcard. Target size: < 28 bytes to fit in legacy
/// BLE advertising data with manufacturer-specific data header.
///
/// Layout (approximate):
/// - protocol_version: 1 byte
/// - device_id: 8 bytes (truncated BLAKE3 fingerprint)
/// - device_name_hash: 2 bytes (for quick UI matching)
/// - capabilities: 1 byte (bitflags)
/// - transport_hint: 1 byte
/// - presence: 1 byte
/// - battery_level: 1 byte (0-100, 255 = unknown)
/// Total: ~15 bytes + postcard framing overhead
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advertisement {
    /// Protocol version for forward compatibility.
    pub protocol_version: u8,
    /// First 8 bytes of BLAKE3(public_key). Enough for collision-free discovery.
    pub device_id: [u8; 8],
    /// CRC16 of device name for quick matching without full name exchange.
    pub device_name_hash: u16,
    /// What this device can do.
    pub capabilities: Capabilities,
    /// Preferred transport mechanism.
    pub transport_hint: TransportHint,
    /// Current device state.
    pub presence: PresenceState,
    /// Battery percentage (0-100, 255 = unknown).
    pub battery_level: u8,
}

impl Advertisement {
    /// Create an advertisement from a device identity.
    pub fn from_identity(
        device_id: &[u8; 32],
        device_name: &str,
        transport_hint: TransportHint,
    ) -> Self {
        let mut id = [0u8; 8];
        id.copy_from_slice(&device_id[..8]);

        Self {
            protocol_version: 1,
            device_id: id,
            device_name_hash: name_hash(device_name),
            capabilities: Capabilities::default(),
            transport_hint,
            presence: PresenceState::Available,
            battery_level: 255, // unknown
        }
    }

    /// Serialize to bytes for BLE manufacturer data.
    pub fn to_bytes(&self) -> Result<Vec<u8>, AdvertisementError> {
        let mut buf = MAGIC.to_vec();
        let payload = postcard::to_allocvec(self)
            .map_err(|e| AdvertisementError::Serialize(e.to_string()))?;
        buf.extend_from_slice(&payload);
        Ok(buf)
    }

    /// Deserialize from BLE manufacturer data bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, AdvertisementError> {
        if data.len() < 2 || data[..2] != MAGIC {
            return Err(AdvertisementError::InvalidMagic);
        }
        postcard::from_bytes(&data[2..])
            .map_err(|e| AdvertisementError::Deserialize(e.to_string()))
    }
}

/// CRC16 of device name for compact BLE advertisement.
fn name_hash(name: &str) -> u16 {
    let hash = blake3::hash(name.as_bytes());
    let bytes = hash.as_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

#[derive(Debug, thiserror::Error)]
pub enum AdvertisementError {
    #[error("invalid magic bytes")]
    InvalidMagic,
    #[error("serialization failed: {0}")]
    Serialize(String),
    #[error("deserialization failed: {0}")]
    Deserialize(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serialization() {
        let ad = Advertisement::from_identity(
            &[0xAB; 32],
            "TestMac",
            TransportHint::SharedLan,
        );
        let bytes = ad.to_bytes().unwrap();
        let decoded = Advertisement::from_bytes(&bytes).unwrap();
        assert_eq!(ad, decoded);
    }

    #[test]
    fn payload_fits_legacy_ble() {
        let ad = Advertisement::from_identity(
            &[0x42; 32],
            "My Very Long Device Name That Should Not Matter",
            TransportHint::LocalHotspot,
        );
        let bytes = ad.to_bytes().unwrap();
        // Legacy BLE manufacturer data: 31 bytes max, minus 4 bytes header = 27 usable
        assert!(bytes.len() <= 27, "payload too large: {} bytes", bytes.len());
    }

    #[test]
    fn invalid_magic_rejected() {
        let result = Advertisement::from_bytes(&[0x00, 0x00, 0x01]);
        assert!(result.is_err());
    }

    #[test]
    fn capabilities_flags() {
        let mut caps = Capabilities::new();
        assert!(caps.has(Capabilities::FILE_TRANSFER));
        assert!(!caps.has(Capabilities::CLIPBOARD_SYNC));
        caps.set(Capabilities::CLIPBOARD_SYNC);
        assert!(caps.has(Capabilities::CLIPBOARD_SYNC));
    }

    #[test]
    fn name_hash_deterministic() {
        let h1 = name_hash("MyPhone");
        let h2 = name_hash("MyPhone");
        assert_eq!(h1, h2);

        let h3 = name_hash("OtherPhone");
        assert_ne!(h1, h3);
    }
}
