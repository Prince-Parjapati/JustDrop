//! Curve25519 key pair management for Noise protocol identity.
//!
//! Generates, stores, and loads long-term static key pairs used for
//! Noise_XX mutual authentication. Keys are persisted to the data directory.

use blake2::{Blake2s256, Digest};
use justdrop_core::error::SecurityError;
use justdrop_core::types::{Fingerprint, PublicKey};
use serde::{Deserialize, Serialize};
use snow::Keypair;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use zeroize::Zeroize;

/// Wrapper around a Noise static key pair with fingerprint caching.
pub struct IdentityKeys {
    /// The raw Noise keypair (private + public).
    keypair: Keypair,
    /// BLAKE2s hash of the public key, used as device fingerprint.
    fingerprint: Fingerprint,
    /// Path where keys are stored on disk.
    storage_path: PathBuf,
}

/// On-disk representation of the keypair.
#[derive(Serialize, Deserialize)]
struct StoredKeys {
    private_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl IdentityKeys {
    /// Load existing keys from disk, or generate new ones if none exist.
    pub fn load_or_generate(data_dir: &Path) -> Result<Self, SecurityError> {
        let storage_path = data_dir.join("identity_keys.json");

        if storage_path.exists() {
            info!(path = %storage_path.display(), "loading existing identity keys");
            Self::load_from_file(&storage_path)
        } else {
            info!("no existing keys found, generating new identity");
            let keys = Self::generate(storage_path.clone())?;
            keys.save()?;
            Ok(keys)
        }
    }

    /// Generate a fresh Curve25519 key pair.
    fn generate(storage_path: PathBuf) -> Result<Self, SecurityError> {
        let builder = snow::Builder::new(
            "Noise_XX_25519_ChaChaPoly_BLAKE2s"
                .parse()
                .map_err(|e| SecurityError::InvalidKey(format!("bad noise params: {e}")))?,
        );

        let keypair = builder.generate_keypair().map_err(|e| {
            SecurityError::InvalidKey(format!("keypair generation failed: {e}"))
        })?;

        let fingerprint = compute_fingerprint(&keypair.public);

        info!(
            fingerprint = %hex_fingerprint(&fingerprint),
            "generated new identity keypair"
        );

        Ok(Self {
            keypair,
            fingerprint,
            storage_path,
        })
    }

    /// Load keys from a JSON file.
    fn load_from_file(path: &Path) -> Result<Self, SecurityError> {
        let content = fs::read_to_string(path).map_err(|e| {
            SecurityError::KeyStorage(format!("failed to read key file: {e}"))
        })?;

        let stored: StoredKeys = serde_json::from_str(&content).map_err(|e| {
            SecurityError::KeyStorage(format!("failed to parse key file: {e}"))
        })?;

        if stored.private_key.len() != 32 || stored.public_key.len() != 32 {
            return Err(SecurityError::InvalidKey(
                "key file contains invalid key lengths".into(),
            ));
        }

        let keypair = Keypair {
            private: stored.private_key,
            public: stored.public_key,
        };

        let fingerprint = compute_fingerprint(&keypair.public);

        debug!(
            fingerprint = %hex_fingerprint(&fingerprint),
            "loaded identity keypair"
        );

        Ok(Self {
            keypair,
            fingerprint,
            storage_path: path.to_path_buf(),
        })
    }

    /// Persist keys to disk.
    fn save(&self) -> Result<(), SecurityError> {
        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SecurityError::KeyStorage(format!("failed to create key directory: {e}"))
            })?;
        }

        let stored = StoredKeys {
            private_key: self.keypair.private.clone(),
            public_key: self.keypair.public.clone(),
        };

        let content = serde_json::to_string_pretty(&stored).map_err(|e| {
            SecurityError::KeyStorage(format!("failed to serialize keys: {e}"))
        })?;

        fs::write(&self.storage_path, content).map_err(|e| {
            SecurityError::KeyStorage(format!("failed to write key file: {e}"))
        })?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.storage_path, perms).map_err(|e| {
                SecurityError::KeyStorage(format!("failed to set key file permissions: {e}"))
            })?;
        }

        info!(path = %self.storage_path.display(), "saved identity keys");
        Ok(())
    }

    /// Get the public key bytes.
    pub fn public_key(&self) -> &[u8] {
        &self.keypair.public
    }

    /// Get the public key as a fixed-size array.
    pub fn public_key_array(&self) -> PublicKey {
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&self.keypair.public);
        pk
    }

    /// Get the private key bytes.
    pub fn private_key(&self) -> &[u8] {
        &self.keypair.private
    }

    /// Get the device fingerprint (BLAKE2s of public key).
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Get the fingerprint as a human-readable hex string.
    pub fn fingerprint_hex(&self) -> String {
        hex_fingerprint(&self.fingerprint)
    }

    /// Get the raw keypair for use with snow builder.
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

impl Drop for IdentityKeys {
    fn drop(&mut self) {
        // Zeroize the private key material
        self.keypair.private.zeroize();
        debug!("zeroized private key material");
    }
}

/// Compute BLAKE2s-256 fingerprint of a public key.
pub fn compute_fingerprint(public_key: &[u8]) -> Fingerprint {
    let mut hasher = Blake2s256::new();
    hasher.update(public_key);
    let result = hasher.finalize();
    let mut fp = [0u8; 32];
    fp.copy_from_slice(&result);
    fp
}

/// Format a fingerprint as a colon-separated hex string for display.
pub fn hex_fingerprint(fp: &Fingerprint) -> String {
    fp.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|pair| pair.join(""))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn generate_and_reload_keys() {
        let tmp = env::temp_dir().join("justdrop_test_keys");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let keys1 = IdentityKeys::load_or_generate(&tmp).unwrap();
        let fp1 = *keys1.fingerprint();
        let pk1 = keys1.public_key_array();

        // Drop and reload — should get same keys
        drop(keys1);
        let keys2 = IdentityKeys::load_or_generate(&tmp).unwrap();
        assert_eq!(*keys2.fingerprint(), fp1);
        assert_eq!(keys2.public_key_array(), pk1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let pk = [42u8; 32];
        let fp1 = compute_fingerprint(&pk);
        let fp2 = compute_fingerprint(&pk);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn fingerprint_display_format() {
        let fp = [0xAB; 32];
        let display = hex_fingerprint(&fp);
        assert!(display.contains(':'));
        // Should be groups of 4 hex chars separated by colons
        assert_eq!(display, "abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab:abab");
    }
}
