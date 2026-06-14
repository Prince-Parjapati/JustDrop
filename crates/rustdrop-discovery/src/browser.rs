//! mDNS service browser for discovering peer RustDrop devices on the local network.
//!
//! Browses for `_rustdrop._tcp.local.` services and maintains a live registry
//! of discovered peers, emitting events via a broadcast channel.

use mdns_sd::{ServiceDaemon, ServiceEvent};
use parking_lot::RwLock;
use rustdrop_core::error::DiscoveryError;
use std::net::Ipv4Addr;
use rustdrop_core::types::{DeviceInfo, Fingerprint, Platform};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Event emitted when the peer list changes.
#[derive(Debug, Clone)]
pub enum PeerEvent {
    /// A new peer was discovered.
    Discovered(DeviceInfo),
    /// A known peer went offline.
    Lost(String),
    /// A known peer updated its info.
    Updated(DeviceInfo),
}

/// Browses the local network for RustDrop peers.
pub struct ServiceBrowser {
    service_type: String,
    peers: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    event_tx: broadcast::Sender<PeerEvent>,
}

impl ServiceBrowser {
    /// Create a new browser.
    ///
    /// Returns the browser and a receiver for peer events.
    pub fn new(service_type: &str) -> (Self, broadcast::Receiver<PeerEvent>) {
        let (event_tx, event_rx) = broadcast::channel(64);
        let browser = Self {
            service_type: service_type.to_string(),
            peers: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        };
        (browser, event_rx)
    }

    /// Subscribe to peer events (additional receivers).
    pub fn subscribe(&self) -> broadcast::Receiver<PeerEvent> {
        self.event_tx.subscribe()
    }

    /// Get a snapshot of all currently known peers.
    pub fn peers(&self) -> Vec<DeviceInfo> {
        self.peers.read().values().cloned().collect()
    }

    /// Get a specific peer by ID.
    pub fn get_peer(&self, id: &str) -> Option<DeviceInfo> {
        self.peers.read().get(id).cloned()
    }

    /// Start browsing for peers. This spawns a blocking background task.
    ///
    /// # Arguments
    /// * `daemon` — shared mDNS daemon (can be from ServiceRegistrar)
    pub fn start_browsing(
        &self,
        daemon: &ServiceDaemon,
    ) -> Result<(), DiscoveryError> {
        let receiver = daemon.browse(&self.service_type).map_err(|e| {
            DiscoveryError::Browse(format!("failed to start browsing: {e}"))
        })?;

        let peers = Arc::clone(&self.peers);
        let event_tx = self.event_tx.clone();
        let service_type = self.service_type.clone();

        // Spawn a blocking task to process mDNS events (mdns-sd uses flume channels, not async)
        tokio::task::spawn_blocking(move || {
            info!(service_type = %service_type, "started mDNS browsing");
            loop {
                match receiver.recv() {
                    Ok(event) => {
                        Self::handle_event(&peers, &event_tx, event);
                    }
                    Err(_) => {
                        debug!("mDNS browse channel closed, stopping browser");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Process a single mDNS event.
    fn handle_event(
        peers: &Arc<RwLock<HashMap<String, DeviceInfo>>>,
        event_tx: &broadcast::Sender<PeerEvent>,
        event: ServiceEvent,
    ) {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                let name = info.get_fullname().to_string();
                let port = info.get_port();

                // Use get_addresses_v4() which returns HashSet<Ipv4Addr>
                let ipv4 = match info.get_addresses_v4().into_iter().next() {
                    Some(addr) => addr,
                    None => {
                        warn!(name = %name, "resolved service has no IPv4 addresses");
                        return;
                    }
                };

                let addr = SocketAddr::new(IpAddr::V4(ipv4), port);

                // Parse TXT records
                let properties = info.get_properties();
                let fingerprint = properties
                    .get_property_val_str("fingerprint")
                    .and_then(|s| parse_fingerprint(s))
                    .unwrap_or([0u8; 32]);

                let platform = properties
                    .get_property_val_str("platform")
                    .map(|s| match s {
                        "Android" => Platform::Android,
                        "macOS" => Platform::MacOS,
                        "Linux" => Platform::Linux,
                        "Windows" => Platform::Windows,
                        _ => Platform::Unknown,
                    })
                    .unwrap_or(Platform::Unknown);

                let device_info = DeviceInfo {
                    id: super::registrar::hex_encode(&fingerprint[..8]),
                    name: info.get_hostname().trim_end_matches('.').to_string(),
                    addr,
                    fingerprint,
                    platform,
                    last_seen: chrono::Utc::now(),
                };

                let mut peers_lock = peers.write();
                let event = if peers_lock.contains_key(&device_info.id) {
                    PeerEvent::Updated(device_info.clone())
                } else {
                    PeerEvent::Discovered(device_info.clone())
                };

                info!(
                    peer_id = %device_info.id,
                    peer_name = %device_info.name,
                    addr = %device_info.addr,
                    platform = %device_info.platform,
                    "peer discovered/updated"
                );

                peers_lock.insert(device_info.id.clone(), device_info);
                let _ = event_tx.send(event);
            }
            ServiceEvent::ServiceRemoved(_, fullname) => {
                let mut peers_lock = peers.write();
                // Find peer by matching fullname prefix
                let id_to_remove: Option<String> = peers_lock
                    .iter()
                    .find(|(_, info)| fullname.contains(&info.name))
                    .map(|(id, _)| id.clone());

                if let Some(id) = id_to_remove {
                    peers_lock.remove(&id);
                    info!(peer_id = %id, "peer lost");
                    let _ = event_tx.send(PeerEvent::Lost(id));
                }
            }
            ServiceEvent::SearchStarted(stype) => {
                debug!(service_type = %stype, "mDNS search started");
            }
            ServiceEvent::SearchStopped(stype) => {
                debug!(service_type = %stype, "mDNS search stopped");
            }
            _ => {}
        }
    }
}

/// Parse a 32-byte fingerprint from a hex string.
fn parse_fingerprint(hex: &str) -> Option<Fingerprint> {
    let bytes = super::registrar::hex_decode(hex)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut fp = [0u8; 32];
    fp.copy_from_slice(&bytes);
    Some(fp)
}
