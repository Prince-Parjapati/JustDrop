//! Device identity model.
//!
//! Each JustDrop installation owns a permanent cryptographic identity
//! consisting of an Ed25519 signing keypair, a stable UUID, and a
//! human-readable device name. The public key is the trust anchor.

use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use uuid::Uuid;

/// Persistent device identity.
///
/// Survives app restarts. The Ed25519 public key is used as the canonical
/// device identifier across the protocol. The UUID is a secondary
/// correlation key for backward compatibility.
pub struct DeviceIdentity {
    /// Stable installation UUID (v4, generated once).
    pub uuid: Uuid,
    /// Human-readable device name.
    pub name: String,
    /// Ed25519 signing keypair (private + public).
    keypair: Ed25519KeyPair,
    /// PKCS#8 DER bytes of the keypair (for persistence).
    pkcs8_bytes: Vec<u8>,
    /// BLAKE3 hash of the public key, truncated for display.
    fingerprint: [u8; 32],
    /// Filesystem path where identity is stored.
    storage_path: PathBuf,
}

/// Serializable on-disk representation.
#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    uuid: Uuid,
    name: String,
    /// Ed25519 PKCS#8 v2 DER-encoded keypair.
    pkcs8_der: Vec<u8>,
}

impl DeviceIdentity {
    /// Load from disk or generate a new identity.
    pub fn load_or_generate(data_dir: &Path, device_name: &str) -> Result<Self, IdentityError> {
        let storage_path = data_dir.join("device_identity.json");

        if storage_path.exists() {
            info!(path = %storage_path.display(), "loading device identity");
            Self::load(&storage_path)
        } else {
            info!("generating new device identity");
            let identity = Self::generate(storage_path, device_name)?;
            identity.save()?;
            Ok(identity)
        }
    }

    /// Generate a fresh Ed25519 identity.
    fn generate(storage_path: PathBuf, device_name: &str) -> Result<Self, IdentityError> {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|_| IdentityError::KeyGeneration)?;

        let keypair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
            .map_err(|_| IdentityError::KeyGeneration)?;

        let fingerprint = blake3::hash(keypair.public_key().as_ref());
        let mut fp = [0u8; 32];
        fp.copy_from_slice(fingerprint.as_bytes());

        let identity = Self {
            uuid: Uuid::new_v4(),
            name: device_name.to_string(),
            keypair,
            pkcs8_bytes: pkcs8_bytes.as_ref().to_vec(),
            fingerprint: fp,
            storage_path,
        };

        info!(
            uuid = %identity.uuid,
            fingerprint = %identity.fingerprint_hex(),
            "new device identity created"
        );
        Ok(identity)
    }

    /// Load identity from JSON file.
    fn load(path: &Path) -> Result<Self, IdentityError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| IdentityError::Storage(format!("read: {e}")))?;

        let stored: StoredIdentity = serde_json::from_str(&content)
            .map_err(|e| IdentityError::Storage(format!("parse: {e}")))?;

        let keypair = Ed25519KeyPair::from_pkcs8(&stored.pkcs8_der)
            .map_err(|_| IdentityError::InvalidKey)?;

        let fingerprint = blake3::hash(keypair.public_key().as_ref());
        let mut fp = [0u8; 32];
        fp.copy_from_slice(fingerprint.as_bytes());

        debug!(uuid = %stored.uuid, "loaded device identity");

        Ok(Self {
            uuid: stored.uuid,
            name: stored.name,
            keypair,
            pkcs8_bytes: stored.pkcs8_der,
            fingerprint: fp,
            storage_path: path.to_path_buf(),
        })
    }

    /// Persist identity to disk with restrictive permissions.
    fn save(&self) -> Result<(), IdentityError> {
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| IdentityError::Storage(format!("mkdir: {e}")))?;
        }

        let stored = StoredIdentity {
            uuid: self.uuid,
            name: self.name.clone(),
            pkcs8_der: self.pkcs8_bytes.clone(),
        };

        let json = serde_json::to_string_pretty(&stored)
            .map_err(|e| IdentityError::Storage(format!("serialize: {e}")))?;

        std::fs::write(&self.storage_path, &json)
            .map_err(|e| IdentityError::Storage(format!("write: {e}")))?;

        // Restrict permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &self.storage_path,
                std::fs::Permissions::from_mode(0o600),
            )
            .map_err(|e| IdentityError::Storage(format!("chmod: {e}")))?;
        }

        info!(path = %self.storage_path.display(), "saved device identity");
        Ok(())
    }

    /// Ed25519 public key bytes (32 bytes).
    pub fn public_key(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }

    /// BLAKE3 fingerprint of the public key (32 bytes).
    pub fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    /// Fingerprint as colon-separated hex for display.
    pub fn fingerprint_hex(&self) -> String {
        self.fingerprint
            .iter()
            .take(8) // 8 bytes = 16 hex chars, enough for display
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Fingerprint as plain hex string (full 32 bytes) for DB keys.
    pub fn fingerprint_full_hex(&self) -> String {
        self.fingerprint
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Sign arbitrary data with this device's Ed25519 key.
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.keypair.sign(data).as_ref().to_vec()
    }

    /// Reference to the raw keypair for TLS certificate generation.
    pub fn pkcs8_der(&self) -> &[u8] {
        &self.pkcs8_bytes
    }
}

impl fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("uuid", &self.uuid)
            .field("name", &self.name)
            .field("fingerprint", &self.fingerprint_hex())
            .finish_non_exhaustive()
    }
}

/// Identity-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("key generation failed")]
    KeyGeneration,
    #[error("invalid key material")]
    InvalidKey,
    #[error("identity storage error: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generate_and_reload() {
        let tmp = TempDir::new().unwrap();
        let id1 = DeviceIdentity::load_or_generate(tmp.path(), "TestMac").unwrap();
        let fp1 = *id1.fingerprint();
        let uuid1 = id1.uuid;

        drop(id1);

        let id2 = DeviceIdentity::load_or_generate(tmp.path(), "TestMac").unwrap();
        assert_eq!(*id2.fingerprint(), fp1);
        assert_eq!(id2.uuid, uuid1);
    }

    #[test]
    fn fingerprint_deterministic() {
        let tmp = TempDir::new().unwrap();
        let id = DeviceIdentity::load_or_generate(tmp.path(), "Test").unwrap();
        let fp1 = id.fingerprint_full_hex();
        let fp2 = id.fingerprint_full_hex();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn sign_produces_valid_signature() {
        let tmp = TempDir::new().unwrap();
        let id = DeviceIdentity::load_or_generate(tmp.path(), "Test").unwrap();
        let data = b"hello justdrop";
        let sig = id.sign(data);
        assert_eq!(sig.len(), 64); // Ed25519 signature is 64 bytes

        // Verify with ring
        let pk = ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519,
            id.public_key(),
        );
        pk.verify(data, &sig).unwrap();
    }

    #[test]
    fn debug_does_not_leak_private_key() {
        let tmp = TempDir::new().unwrap();
        let id = DeviceIdentity::load_or_generate(tmp.path(), "Test").unwrap();
        let dbg = format!("{id:?}");
        assert!(dbg.contains("fingerprint"));
        assert!(!dbg.contains("pkcs8"));
        assert!(!dbg.contains("keypair"));
    }
}
