//! Hotspot credential exchange format.
//!
//! After BLE handshake negotiates a LocalOnlyHotspot transport,
//! the hotspot-creating device sends credentials over BLE GATT
//! so the other device can auto-join.

use serde::{Deserialize, Serialize};

/// GATT characteristic UUID for hotspot credential delivery.
pub const HOTSPOT_CREDENTIAL_UUID: &str = "7A5D3E2F-1B4C-4D8E-9F6A-0E3C5B7D9A32";

/// Hotspot credentials sent over BLE after hotspot creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotspotCredentials {
    /// Wi-Fi SSID.
    pub ssid: String,
    /// WPA2/WPA3 passphrase.
    pub passphrase: String,
    /// BSSID (MAC address) for faster association.
    pub bssid: Option<String>,
    /// IP address the QUIC server is listening on within the hotspot.
    pub server_ip: String,
    /// QUIC port.
    pub port: u16,
}

impl HotspotCredentials {
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let creds = HotspotCredentials {
            ssid: "DIRECT-JD-a1b2".to_string(),
            passphrase: "randompassword123".to_string(),
            bssid: Some("AA:BB:CC:DD:EE:FF".to_string()),
            server_ip: "192.168.49.1".to_string(),
            port: 42420,
        };
        let bytes = creds.to_bytes().unwrap();
        let decoded = HotspotCredentials::from_bytes(&bytes).unwrap();
        assert_eq!(creds, decoded);
    }

    #[test]
    fn size_fits_gatt() {
        let creds = HotspotCredentials {
            ssid: "DIRECT-JD-longname123456".to_string(),
            passphrase: "a_very_long_secure_passphrase_here".to_string(),
            bssid: Some("AA:BB:CC:DD:EE:FF".to_string()),
            server_ip: "192.168.49.1".to_string(),
            port: 42420,
        };
        let bytes = creds.to_bytes().unwrap();
        assert!(bytes.len() < 512, "too large: {} bytes", bytes.len());
    }
}
