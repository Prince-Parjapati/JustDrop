//! Transfer resume state persistence.
//!
//! Saves and loads the state of interrupted transfers so they can be
//! resumed from the last successfully received chunk.

use justdrop_core::error::StorageError;
use justdrop_core::types::TransferId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Persisted resume state for a single file within a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResumeState {
    /// File index within the transfer.
    pub file_index: u32,
    /// Destination path.
    pub dest_path: String,
    /// Temporary file path.
    pub temp_path: String,
    /// File size.
    pub file_size: u64,
    /// Chunk size used.
    pub chunk_size: u32,
    /// Set of completed chunk offsets.
    pub completed_chunks: HashSet<u64>,
}

/// Persisted resume state for an entire transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResumeState {
    /// Transfer ID.
    pub transfer_id: TransferId,
    /// Timestamp of last activity.
    pub last_updated: chrono::DateTime<chrono::Utc>,
    /// Per-file resume states.
    pub files: Vec<FileResumeState>,
    /// Total bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total bytes expected.
    pub total_bytes: u64,
}

/// Manages resume state files on disk.
pub struct ResumeManager {
    /// Directory where resume state files are stored.
    state_dir: PathBuf,
}

impl ResumeManager {
    /// Create a new resume manager.
    pub fn new(state_dir: &Path) -> Self {
        Self {
            state_dir: state_dir.to_path_buf(),
        }
    }

    /// Ensure the state directory exists.
    pub async fn init(&self) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.state_dir)
            .await
            .map_err(|e| StorageError::Io {
                path: self.state_dir.clone(),
                source: e,
            })?;
        Ok(())
    }

    /// Save resume state for a transfer.
    pub async fn save(&self, state: &TransferResumeState) -> Result<(), StorageError> {
        let path = self.state_path(state.transfer_id);

        let content = serde_json::to_string_pretty(state).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| StorageError::Io {
                path: path.clone(),
                source: e,
            })?;

        debug!(
            transfer_id = %state.transfer_id,
            path = %path.display(),
            "saved resume state"
        );
        Ok(())
    }

    /// Load resume state for a transfer, if it exists.
    pub async fn load(&self, transfer_id: TransferId) -> Result<Option<TransferResumeState>, StorageError> {
        let path = self.state_path(transfer_id);

        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| StorageError::Io {
                path: path.clone(),
                source: e,
            })?;

        let state: TransferResumeState =
            serde_json::from_str(&content).map_err(|_| StorageError::ResumeStateCorrupted {
                transfer_id,
            })?;

        info!(
            transfer_id = %transfer_id,
            bytes_transferred = state.bytes_transferred,
            total_bytes = state.total_bytes,
            "loaded resume state"
        );

        Ok(Some(state))
    }

    /// Remove resume state after successful transfer completion.
    pub async fn remove(&self, transfer_id: TransferId) -> Result<(), StorageError> {
        let path = self.state_path(transfer_id);

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| StorageError::Io {
                    path: path.clone(),
                    source: e,
                })?;
            debug!(transfer_id = %transfer_id, "removed resume state");
        }

        Ok(())
    }

    /// List all pending resume states.
    pub async fn list_pending(&self) -> Result<Vec<TransferResumeState>, StorageError> {
        let mut states = Vec::new();

        let mut dir = tokio::fs::read_dir(&self.state_dir)
            .await
            .map_err(|e| StorageError::Io {
                path: self.state_dir.clone(),
                source: e,
            })?;

        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        match serde_json::from_str::<TransferResumeState>(&content) {
                            Ok(state) => states.push(state),
                            Err(e) => {
                                warn!(path = %path.display(), error = %e, "corrupt resume state, skipping");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "failed to read resume state");
                    }
                }
            }
        }

        Ok(states)
    }

    /// Compute a compressed bitmap of received chunks for the resume response.
    pub fn encode_received_bitmap(completed: &HashSet<u64>, total_chunks: u64) -> Vec<u8> {
        let byte_count = ((total_chunks + 7) / 8) as usize;
        let mut bitmap = vec![0u8; byte_count];
        for &chunk in completed {
            let byte_idx = (chunk / 8) as usize;
            let bit_idx = (chunk % 8) as u8;
            if byte_idx < bitmap.len() {
                bitmap[byte_idx] |= 1 << bit_idx;
            }
        }
        bitmap
    }

    /// Decode a received chunk bitmap back into a set of chunk offsets.
    pub fn decode_received_bitmap(bitmap: &[u8], total_chunks: u64) -> HashSet<u64> {
        let mut completed = HashSet::new();
        for chunk in 0..total_chunks {
            let byte_idx = (chunk / 8) as usize;
            let bit_idx = (chunk % 8) as u8;
            if byte_idx < bitmap.len() && (bitmap[byte_idx] & (1 << bit_idx)) != 0 {
                completed.insert(chunk);
            }
        }
        completed
    }

    /// Get the file path for a transfer's resume state.
    fn state_path(&self, transfer_id: TransferId) -> PathBuf {
        self.state_dir.join(format!("{transfer_id}.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_and_load_resume_state() {
        let tmp = std::env::temp_dir().join("justdrop_resume_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let manager = ResumeManager::new(&tmp);
        manager.init().await.unwrap();

        let transfer_id = uuid::Uuid::new_v4();
        let state = TransferResumeState {
            transfer_id,
            last_updated: chrono::Utc::now(),
            files: vec![FileResumeState {
                file_index: 0,
                dest_path: "/tmp/test.bin".into(),
                temp_path: "/tmp/test.bin.justdrop_partial".into(),
                file_size: 1024,
                chunk_size: 256,
                completed_chunks: [0, 1, 3].into_iter().collect(),
            }],
            bytes_transferred: 768,
            total_bytes: 1024,
        };

        manager.save(&state).await.unwrap();

        let loaded = manager.load(transfer_id).await.unwrap().unwrap();
        assert_eq!(loaded.transfer_id, transfer_id);
        assert_eq!(loaded.bytes_transferred, 768);
        assert_eq!(loaded.files[0].completed_chunks.len(), 3);

        manager.remove(transfer_id).await.unwrap();
        assert!(manager.load(transfer_id).await.unwrap().is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn bitmap_roundtrip() {
        let completed: HashSet<u64> = [0, 2, 5, 7, 10].into_iter().collect();
        let total_chunks = 16;

        let bitmap = ResumeManager::encode_received_bitmap(&completed, total_chunks);
        let decoded = ResumeManager::decode_received_bitmap(&bitmap, total_chunks);

        assert_eq!(completed, decoded);
    }

    #[test]
    fn empty_bitmap() {
        let completed: HashSet<u64> = HashSet::new();
        let bitmap = ResumeManager::encode_received_bitmap(&completed, 8);
        assert_eq!(bitmap, vec![0]);
        let decoded = ResumeManager::decode_received_bitmap(&bitmap, 8);
        assert!(decoded.is_empty());
    }
}
