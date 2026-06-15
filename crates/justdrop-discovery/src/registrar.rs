//! mDNS service registration for advertising this device on the local network.
//!
//! Registers a `_justdrop._tcp.local.` service with TXT records containing
//! the device name, platform, protocol version, and public key fingerprint.

use justdrop_core::error::DiscoveryError;
use justdrop_core::types::{current_platform, Fingerprint, PROTOCOL_VERSION};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use tracing::{debug, error, info};

/// Handles mDNS service registration (advertising our presence).
pub struct ServiceRegistrar {
    daemon: ServiceDaemon,
    service_type: String,
    instance_name: String,
    registered: bool,
}

impl ServiceRegistrar {
    /// Create a new registrar.
    ///
    /// # Arguments
    /// * `service_type` — e.g. `"_justdrop._tcp.local."`
    /// * `device_name` — human-readable name for this device
    pub fn new(service_type: &str, device_name: &str) -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|e| {
            DiscoveryError::DaemonCreation(format!("failed to create mDNS daemon: {e}"))
        })?;

        Ok(Self {
            daemon,
            service_type: service_type.to_string(),
            instance_name: device_name.to_string(),
            registered: false,
        })
    }

    /// Register the JustDrop service on the network.
    ///
    /// # Arguments
    /// * `port` — TCP listen port
    /// * `fingerprint` — public key fingerprint for identity verification
    pub fn register(&mut self, port: u16, fingerprint: &Fingerprint) -> Result<(), DiscoveryError> {
        let mut properties = HashMap::new();
        properties.insert("version".to_string(), PROTOCOL_VERSION.to_string());
        properties.insert("platform".to_string(), current_platform().to_string());
        properties.insert("fingerprint".to_string(), hex::encode(fingerprint));

        // Build the properties vec for ServiceInfo
        let props: Vec<(&str, &str)> = properties
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // mDNS hostname must end with ".local." and contain no spaces
        let sanitized = self
            .instance_name
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "-");
        let mdns_hostname = format!("{sanitized}.local.");

        let local_ip = get_local_ip().unwrap_or_else(|| "0.0.0.0".to_string());

        let service_info = ServiceInfo::new(
            &self.service_type,
            &self.instance_name,
            &mdns_hostname,
            &local_ip,
            port,
            &props[..],
        )
        .map_err(|e| DiscoveryError::Registration(format!("failed to build ServiceInfo: {e}")))?;

        self.daemon.register(service_info).map_err(|e| {
            DiscoveryError::Registration(format!("failed to register service: {e}"))
        })?;

        self.registered = true;
        info!(
            service_type = %self.service_type,
            instance = %self.instance_name,
            port = port,
            "registered mDNS service"
        );
        Ok(())
    }

    /// Unregister the service from the network.
    pub fn unregister(&mut self) -> Result<(), DiscoveryError> {
        if !self.registered {
            return Ok(());
        }

        let fullname = format!("{}.{}", self.instance_name, self.service_type);
        self.daemon.unregister(&fullname).map_err(|e| {
            DiscoveryError::Registration(format!("failed to unregister service: {e}"))
        })?;

        self.registered = false;
        info!("unregistered mDNS service");
        Ok(())
    }

    /// Get a reference to the underlying daemon for shared use with browser.
    pub fn daemon(&self) -> &ServiceDaemon {
        &self.daemon
    }
}

impl Drop for ServiceRegistrar {
    fn drop(&mut self) {
        if self.registered {
            if let Err(e) = self.unregister() {
                error!("failed to unregister on drop: {e}");
            }
        }
        debug!("shutting down mDNS daemon");
        if let Err(e) = self.daemon.shutdown() {
            error!("failed to shutdown mDNS daemon: {e}");
        }
    }
}

/// Hex encoding utility (avoids adding hex crate dependency).
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Option<Vec<u8>> {
        if s.len() % 2 != 0 {
            return None;
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
            .collect()
    }
}

pub use hex::{decode as hex_decode, encode as hex_encode};

/// Uses a dummy UDP connection to reliably detect the device's local LAN IP address.
fn get_local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::hex;

    #[test]
    fn hex_roundtrip() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let encoded = hex::encode(&data);
        assert_eq!(encoded, "deadbeef");
        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
