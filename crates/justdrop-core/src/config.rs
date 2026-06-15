//! Configuration loading and management for JustDrop.
//!
//! Supports loading from TOML files with sensible defaults for all values.

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub transfer: TransferConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Human-readable device name. Auto-detected if empty.
    #[serde(default)]
    pub device_name: String,
    /// Download directory. Uses platform default if empty.
    #[serde(default)]
    pub download_dir: String,
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// TCP listen port.
    #[serde(default = "default_port")]
    pub listen_port: u16,
    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// TCP send buffer size in bytes.
    #[serde(default = "default_buffer_size")]
    pub send_buffer_size: usize,
    /// TCP receive buffer size in bytes.
    #[serde(default = "default_buffer_size")]
    pub recv_buffer_size: usize,
    /// Enable TCP_NODELAY.
    #[serde(default = "default_true")]
    pub tcp_nodelay: bool,
}

/// Transfer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferConfig {
    /// Base chunk size in bytes.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    /// Large file threshold in bytes for auto-scaling chunk size.
    #[serde(default = "default_large_file_threshold")]
    pub large_file_threshold: u64,
    /// Maximum concurrent transfers.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_transfers: usize,
    /// Pipeline depth (chunks buffered ahead).
    #[serde(default = "default_pipeline_depth")]
    pub pipeline_depth: usize,
}

/// Security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Encrypt all chunk data (recommended).
    #[serde(default = "default_true")]
    pub encrypt_data: bool,
    /// Auto-accept from known (previously paired) devices.
    #[serde(default)]
    pub auto_accept_known: bool,
    /// Auto-accept all transfers (insecure).
    #[serde(default)]
    pub auto_accept_all: bool,
}

/// Discovery configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// mDNS service type.
    #[serde(default = "default_service_type")]
    pub service_type: String,
    /// Announce interval in seconds.
    #[serde(default = "default_announce_interval")]
    pub announce_interval_secs: u64,
    /// Peer timeout in seconds.
    #[serde(default = "default_peer_timeout")]
    pub peer_timeout_secs: u64,
}

// Default value functions

fn default_log_level() -> String {
    "info".into()
}

fn default_port() -> u16 {
    42420
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_buffer_size() -> usize {
    2 * 1024 * 1024 // 2 MiB
}

fn default_true() -> bool {
    true
}

fn default_chunk_size() -> u32 {
    256 * 1024 // 256 KiB
}

fn default_large_file_threshold() -> u64 {
    1024 * 1024 * 1024 // 1 GiB
}

fn default_max_concurrent() -> usize {
    4
}

fn default_pipeline_depth() -> usize {
    8
}

fn default_service_type() -> String {
    "_justdrop._tcp.local.".into()
}

fn default_announce_interval() -> u64 {
    60
}

fn default_peer_timeout() -> u64 {
    180
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            network: NetworkConfig::default(),
            transfer: TransferConfig::default(),
            security: SecurityConfig::default(),
            discovery: DiscoveryConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            download_dir: String::new(),
            log_level: default_log_level(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: default_port(),
            connect_timeout_secs: default_connect_timeout(),
            send_buffer_size: default_buffer_size(),
            recv_buffer_size: default_buffer_size(),
            tcp_nodelay: true,
        }
    }
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            large_file_threshold: default_large_file_threshold(),
            max_concurrent_transfers: default_max_concurrent(),
            pipeline_depth: default_pipeline_depth(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            encrypt_data: true,
            auto_accept_known: false,
            auto_accept_all: false,
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_type: default_service_type(),
            announce_interval_secs: default_announce_interval(),
            peer_timeout_secs: default_peer_timeout(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file, falling back to defaults for missing fields.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            info!("Config file not found at {}, using defaults", path.display());
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path).map_err(|_| ConfigError::NotFound {
            path: path.to_path_buf(),
        })?;

        let config: Config = toml::from_str(&content).map_err(|e| ConfigError::Parse { source: e })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.network.listen_port == 0 {
            return Err(ConfigError::InvalidValue {
                field: "network.listen_port".into(),
                value: "0".into(),
                reason: "port must be > 0".into(),
            });
        }
        if self.transfer.chunk_size < 4096 {
            return Err(ConfigError::InvalidValue {
                field: "transfer.chunk_size".into(),
                value: self.transfer.chunk_size.to_string(),
                reason: "chunk size must be >= 4096 bytes".into(),
            });
        }
        if self.transfer.pipeline_depth == 0 {
            return Err(ConfigError::InvalidValue {
                field: "transfer.pipeline_depth".into(),
                value: "0".into(),
                reason: "pipeline depth must be > 0".into(),
            });
        }
        Ok(())
    }

    /// Resolve the download directory, using ~/JustDrop as the default.
    /// All received files are saved into this folder for easy access.
    pub fn download_dir(&self) -> PathBuf {
        if self.general.download_dir.is_empty() {
            dirs::home_dir()
                .map(|h| h.join("JustDrop"))
                .unwrap_or_else(|| PathBuf::from("JustDrop"))
        } else {
            PathBuf::from(&self.general.download_dir)
        }
    }

    /// Resolve the device name, using hostname if not configured.
    pub fn device_name(&self) -> String {
        if self.general.device_name.is_empty() {
            hostname()
        } else {
            self.general.device_name.clone()
        }
    }

    /// Get the path to the config directory.
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("justdrop"))
            .unwrap_or_else(|| PathBuf::from(".justdrop"))
    }

    /// Get the path to the data directory (for keys, resume state, etc.).
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .map(|d| d.join("justdrop"))
            .unwrap_or_else(|| PathBuf::from(".justdrop"))
    }
}

/// Get the system hostname.
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "JustDrop Device".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_config_is_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn load_from_toml_string() {
        let toml_str = r#"
[general]
device_name = "Test Device"
log_level = "debug"

[network]
listen_port = 9999

[transfer]
chunk_size = 524288
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.general.device_name, "Test Device");
        assert_eq!(config.network.listen_port, 9999);
        assert_eq!(config.transfer.chunk_size, 524288);
        // Defaults for unspecified fields
        assert!(config.security.encrypt_data);
        assert_eq!(config.transfer.pipeline_depth, 8);
    }

    #[test]
    fn validation_rejects_zero_port() {
        let mut config = Config::default();
        config.network.listen_port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_tiny_chunk_size() {
        let mut config = Config::default();
        config.transfer.chunk_size = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let config = Config::load(Path::new("/nonexistent/config.toml")).unwrap();
        assert_eq!(config.network.listen_port, 42420);
    }
}
