//! UniFFI bindings for JustDrop.
//!
//! This crate generates Swift and Kotlin bindings via UniFFI.
//! All types and the engine interface are defined in `justdrop.udl`.

uniffi::include_scaffolding!("justdrop");

use justdrop_core::config::Config;
use justdrop_core::trust::TrustLevel as CoreTrustLevel;
use justdrop_engine::engine::{Engine, EngineConfig, EngineError as CoreEngineError};
use justdrop_engine::events::{EngineEvent, EngineEventHandler};
use justdrop_engine::peer::Peer as EnginePeer;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

// Legacy modules retained for backward compat during migration
pub mod android;
pub mod legacy_c_abi;
pub mod macos;

// ── UniFFI Type Mappings ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TrustLevel {
    Unknown,
    Trusted,
    Favorite,
    Blocked,
}

impl From<CoreTrustLevel> for TrustLevel {
    fn from(t: CoreTrustLevel) -> Self {
        match t {
            CoreTrustLevel::Unknown => TrustLevel::Unknown,
            CoreTrustLevel::Trusted => TrustLevel::Trusted,
            CoreTrustLevel::Favorite => TrustLevel::Favorite,
            CoreTrustLevel::Blocked => TrustLevel::Blocked,
        }
    }
}

impl From<TrustLevel> for CoreTrustLevel {
    fn from(t: TrustLevel) -> Self {
        match t {
            TrustLevel::Unknown => CoreTrustLevel::Unknown,
            TrustLevel::Trusted => CoreTrustLevel::Trusted,
            TrustLevel::Favorite => CoreTrustLevel::Favorite,
            TrustLevel::Blocked => CoreTrustLevel::Blocked,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PresenceState {
    Idle,
    Available,
    Receiving,
    Busy,
    Invisible,
}

impl From<justdrop_ble::advertisement::PresenceState> for PresenceState {
    fn from(p: justdrop_ble::advertisement::PresenceState) -> Self {
        match p {
            justdrop_ble::advertisement::PresenceState::Idle => PresenceState::Idle,
            justdrop_ble::advertisement::PresenceState::Available => PresenceState::Available,
            justdrop_ble::advertisement::PresenceState::Receiving => PresenceState::Receiving,
            justdrop_ble::advertisement::PresenceState::Busy => PresenceState::Busy,
            justdrop_ble::advertisement::PresenceState::Invisible => PresenceState::Invisible,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlatformType {
    MacOS,
    Android,
    Windows,
    Linux,
    Unknown,
}

impl From<justdrop_core::types::Platform> for PlatformType {
    fn from(p: justdrop_core::types::Platform) -> Self {
        match p {
            justdrop_core::types::Platform::MacOS => PlatformType::MacOS,
            justdrop_core::types::Platform::Android => PlatformType::Android,
            justdrop_core::types::Platform::Windows => PlatformType::Windows,
            justdrop_core::types::Platform::Linux => PlatformType::Linux,
            justdrop_core::types::Platform::Unknown => PlatformType::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone)]
pub enum TransferStatus {
    Negotiating,
    Transferring,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

// ── Data Transfer Objects ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub device_id: String,
    pub name: String,
    pub fingerprint: Option<String>,
    pub platform: PlatformType,
    pub presence: PresenceState,
    pub trust: TrustLevel,
    pub address: Option<String>,
    pub rssi: Option<i16>,
    pub battery: Option<u8>,
}

impl From<EnginePeer> for PeerInfo {
    fn from(p: EnginePeer) -> Self {
        Self {
            device_id: p.device_id,
            name: p.name,
            fingerprint: p.fingerprint,
            platform: p.platform.into(),
            presence: p.presence.into(),
            trust: p.trust.into(),
            address: p.addr.map(|a| a.to_string()),
            rssi: p.rssi,
            battery: p.battery,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferInfo {
    pub transfer_id: String,
    pub peer_name: String,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub percent: u8,
    pub eta_secs: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct IncomingRequest {
    pub transfer_id: String,
    pub peer: PeerInfo,
    pub files: Vec<FileInfo>,
    pub total_size: u64,
}

// ── Error ───────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum JustDropError {
    #[error("initialization failed")]
    InitFailed,
    #[error("engine not running")]
    NotRunning,
    #[error("peer not found")]
    PeerNotFound,
    #[error("transfer failed")]
    TransferFailed,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("database error")]
    DatabaseError,
}

impl From<CoreEngineError> for JustDropError {
    fn from(e: CoreEngineError) -> Self {
        match e {
            CoreEngineError::Init(_) => JustDropError::InitFailed,
            CoreEngineError::Database(_) => JustDropError::DatabaseError,
            CoreEngineError::PeerNotFound(_) => JustDropError::PeerNotFound,
            CoreEngineError::SessionNotFound(_) => JustDropError::TransferFailed,
            CoreEngineError::Transfer(_) => JustDropError::TransferFailed,
        }
    }
}

// ── Callback Interface ──────────────────────────────────────────────

pub trait JustDropEventHandler: Send + Sync {
    fn on_peer_discovered(&self, peer: PeerInfo);
    fn on_peer_lost(&self, device_id: String);
    fn on_peer_updated(&self, peer: PeerInfo);
    fn on_incoming_request(&self, request: IncomingRequest);
    fn on_transfer_accepted(&self, transfer_id: String);
    fn on_transfer_rejected(&self, transfer_id: String, reason: String);
    fn on_transfer_progress(&self, info: TransferInfo);
    fn on_transfer_completed(&self, transfer_id: String, saved_paths: Vec<String>);
    fn on_transfer_failed(&self, transfer_id: String, error: String);
    fn on_transfer_cancelled(&self, transfer_id: String, reason: String);
    fn on_join_hotspot(&self, ssid: String, passphrase: String);
    fn on_create_hotspot(&self);
    fn on_engine_started(&self);
    fn on_engine_stopped(&self);
    fn on_error(&self, message: String);
}

/// Bridge from Engine events to UniFFI callback.
struct EventBridge {
    handler: Box<dyn JustDropEventHandler>,
}

impl EngineEventHandler for EventBridge {
    fn on_event(&self, event: EngineEvent) {
        match event {
            EngineEvent::PeerDiscovered(p) => self.handler.on_peer_discovered(p.into()),
            EngineEvent::PeerLost { device_id } => self.handler.on_peer_lost(device_id),
            EngineEvent::PeerUpdated(p) => self.handler.on_peer_updated(p.into()),
            EngineEvent::IncomingTransferRequest {
                transfer_id,
                peer,
                manifest,
                previews: _,
            } => {
                let files: Vec<FileInfo> = manifest
                    .files
                    .iter()
                    .map(|f| FileInfo {
                        name: f.relative_path.clone(),
                        size: f.size,
                        mime_type: f.mime_type.clone(),
                    })
                    .collect();
                self.handler.on_incoming_request(IncomingRequest {
                    transfer_id: transfer_id.to_string(),
                    peer: peer.into(),
                    total_size: manifest.total_size,
                    files,
                });
            }
            EngineEvent::TransferAccepted { transfer_id } => {
                self.handler.on_transfer_accepted(transfer_id.to_string());
            }
            EngineEvent::TransferRejected {
                transfer_id,
                reason,
            } => {
                self.handler
                    .on_transfer_rejected(transfer_id.to_string(), reason);
            }
            EngineEvent::TransferProgress(p) => {
                self.handler.on_transfer_progress(TransferInfo {
                    transfer_id: p.transfer_id.to_string(),
                    peer_name: String::new(),
                    direction: TransferDirection::Send,
                    status: TransferStatus::Transferring,
                    bytes_transferred: p.bytes_transferred,
                    total_bytes: p.total_bytes,
                    speed_bps: p.speed_bps,
                    percent: p.percent(),
                    eta_secs: p.eta_secs,
                });
            }
            EngineEvent::TransferCompleted {
                transfer_id,
                saved_paths,
            } => {
                let paths: Vec<String> = saved_paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                self.handler
                    .on_transfer_completed(transfer_id.to_string(), paths);
            }
            EngineEvent::TransferFailed { transfer_id, error } => {
                self.handler
                    .on_transfer_failed(transfer_id.to_string(), error);
            }
            EngineEvent::TransferCancelled {
                transfer_id,
                reason,
            } => {
                self.handler
                    .on_transfer_cancelled(transfer_id.to_string(), reason);
            }
            EngineEvent::JoinHotspot { ssid, passphrase } => {
                self.handler.on_join_hotspot(ssid, passphrase);
            }
            EngineEvent::CreateHotspot => {
                self.handler.on_create_hotspot();
            }
            EngineEvent::Started => {
                self.handler.on_engine_started();
            }
            EngineEvent::Stopped => {
                self.handler.on_engine_stopped();
            }
            EngineEvent::Error { message } => {
                self.handler.on_error(message);
            }
        }
    }
}

// ── UniFFI Engine Wrapper ───────────────────────────────────────────

pub struct JustDropEngine {
    inner: Engine,
}

impl JustDropEngine {
    pub fn new(
        data_dir: String,
        config_path: Option<String>,
        handler: Box<dyn JustDropEventHandler>,
    ) -> Result<Self, JustDropError> {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("justdrop=info")
            .try_init();

        let config = match config_path {
            Some(path) => {
                Config::load(std::path::Path::new(&path)).map_err(|_| JustDropError::InitFailed)?
            }
            None => Config::default(),
        };

        let bridge = EventBridge { handler };
        let engine_config = EngineConfig {
            config,
            data_dir: PathBuf::from(&data_dir),
        };

        let inner = Engine::new(engine_config, Arc::new(bridge))?;
        Ok(Self { inner })
    }

    pub fn device_name(&self) -> String {
        self.inner.device_name()
    }

    pub fn device_fingerprint(&self) -> String {
        self.inner.device_fingerprint()
    }

    pub fn device_uuid(&self) -> String {
        self.inner.device_uuid()
    }

    pub fn start_discovery(&self) -> Result<(), JustDropError> {
        // TODO: Start BLE scanning via platform callback + mDNS browsing
        Ok(())
    }

    pub fn stop_discovery(&self) {
        // TODO: Stop BLE scanning + mDNS browsing
    }

    pub fn visible_peers(&self) -> Vec<PeerInfo> {
        self.inner
            .visible_peers()
            .into_iter()
            .map(|p| p.into())
            .collect()
    }

    pub fn peer_count(&self) -> u32 {
        self.inner.peer_count() as u32
    }

    pub fn set_peer_trust(
        &self,
        fingerprint: String,
        level: TrustLevel,
    ) -> Result<(), JustDropError> {
        self.inner.set_peer_trust(&fingerprint, level.into())?;
        Ok(())
    }

    pub fn send_files(
        &self,
        device_id: String,
        file_paths: Vec<String>,
    ) -> Result<String, JustDropError> {
        let paths: Vec<PathBuf> = file_paths.into_iter().map(PathBuf::from).collect();
        let id = self.inner.send_files(&device_id, paths)?;
        Ok(id.to_string())
    }

    pub fn accept_transfer(&self, transfer_id: String) -> Result<(), JustDropError> {
        let id = Uuid::parse_str(&transfer_id).map_err(|_| JustDropError::InvalidArgument)?;
        self.inner.accept_transfer(id)?;
        Ok(())
    }

    pub fn reject_transfer(
        &self,
        transfer_id: String,
        reason: String,
    ) -> Result<(), JustDropError> {
        let id = Uuid::parse_str(&transfer_id).map_err(|_| JustDropError::InvalidArgument)?;
        self.inner.reject_transfer(id, &reason)?;
        Ok(())
    }

    pub fn cancel_transfer(&self, transfer_id: String) -> Result<(), JustDropError> {
        let id = Uuid::parse_str(&transfer_id).map_err(|_| JustDropError::InvalidArgument)?;
        self.inner.cancel_transfer(id)?;
        Ok(())
    }

    pub fn active_transfer_count(&self) -> u32 {
        self.inner.active_transfer_count() as u32
    }

    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    pub fn stop(&self) {
        self.inner.stop();
    }
}
