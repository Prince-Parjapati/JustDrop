//! Peer representation within the engine.
//!
//! A Peer combines discovery information with trust state and presence.

use justdrop_core::trust::TrustLevel;
use justdrop_core::types::Platform;
use justdrop_ble::advertisement::PresenceState;
use std::net::SocketAddr;

/// A discovered peer device.
#[derive(Debug, Clone)]
pub struct Peer {
    /// Truncated BLAKE3 fingerprint (8 bytes, hex-encoded).
    pub device_id: String,
    /// Human-readable device name.
    pub name: String,
    /// Full public key fingerprint (hex, 64 chars).
    pub fingerprint: Option<String>,
    /// Platform type.
    pub platform: Platform,
    /// Current presence state.
    pub presence: PresenceState,
    /// Trust level from the local database.
    pub trust: TrustLevel,
    /// Network address (available after transport negotiation).
    pub addr: Option<SocketAddr>,
    /// BLE RSSI signal strength (for sorting by proximity).
    pub rssi: Option<i16>,
    /// Battery level (0-100, None if unknown).
    pub battery: Option<u8>,
    /// When this peer was last seen.
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Discovery source.
    pub discovery: DiscoverySource,
}

/// How this peer was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    /// Discovered via BLE advertisement.
    Ble,
    /// Discovered via mDNS on shared LAN.
    Mdns,
    /// Both BLE and mDNS.
    Both,
}

impl Peer {
    /// Create a peer from a BLE advertisement.
    pub fn from_ble(
        device_id: [u8; 8],
        name_hash: u16,
        presence: PresenceState,
        rssi: i16,
    ) -> Self {
        let id_hex: String = device_id.iter().map(|b| format!("{b:02x}")).collect();
        Self {
            device_id: id_hex,
            name: format!("Device-{:04x}", name_hash),
            fingerprint: None,
            platform: Platform::Unknown,
            presence,
            trust: TrustLevel::Unknown,
            addr: None,
            rssi: Some(rssi),
            battery: None,
            last_seen: chrono::Utc::now(),
            discovery: DiscoverySource::Ble,
        }
    }

    /// Create a peer from mDNS discovery.
    pub fn from_mdns(
        name: String,
        addr: SocketAddr,
        fingerprint: String,
        platform: Platform,
    ) -> Self {
        let device_id = fingerprint.chars().take(16).collect();
        Self {
            device_id,
            name,
            fingerprint: Some(fingerprint),
            platform,
            presence: PresenceState::Available,
            trust: TrustLevel::Unknown,
            addr: Some(addr),
            rssi: None,
            battery: None,
            last_seen: chrono::Utc::now(),
            discovery: DiscoverySource::Mdns,
        }
    }

    /// Update trust level from database lookup.
    pub fn with_trust(mut self, trust: TrustLevel) -> Self {
        self.trust = trust;
        self
    }

    /// Whether this peer should be visible in the UI.
    pub fn is_visible(&self) -> bool {
        !self.trust.is_blocked() && self.presence != PresenceState::Invisible
    }

    /// Whether this peer should auto-accept transfers.
    pub fn auto_accept(&self) -> bool {
        self.trust.auto_accept()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ble_peer_creation() {
        let peer = Peer::from_ble([0xAA; 8], 0x1234, PresenceState::Available, -65);
        assert_eq!(peer.device_id, "aaaaaaaaaaaaaaaa");
        assert!(peer.is_visible());
        assert!(!peer.auto_accept());
        assert_eq!(peer.discovery, DiscoverySource::Ble);
    }

    #[test]
    fn blocked_peer_invisible() {
        let peer = Peer::from_ble([0xBB; 8], 0x5678, PresenceState::Available, -70)
            .with_trust(TrustLevel::Blocked);
        assert!(!peer.is_visible());
    }

    #[test]
    fn favorite_auto_accepts() {
        let peer = Peer::from_ble([0xCC; 8], 0x9ABC, PresenceState::Available, -50)
            .with_trust(TrustLevel::Favorite);
        assert!(peer.auto_accept());
        assert!(peer.is_visible());
    }

    #[test]
    fn invisible_peer_hidden() {
        let peer = Peer::from_ble([0xDD; 8], 0xDEF0, PresenceState::Invisible, -80);
        assert!(!peer.is_visible());
    }

    #[test]
    fn mdns_peer_has_address() {
        let addr: SocketAddr = "192.168.1.10:42420".parse().unwrap();
        let peer = Peer::from_mdns(
            "MacBook".into(),
            addr,
            "abcdef1234567890".into(),
            Platform::MacOS,
        );
        assert_eq!(peer.addr, Some(addr));
        assert_eq!(peer.platform, Platform::MacOS);
        assert_eq!(peer.discovery, DiscoverySource::Mdns);
    }
}
