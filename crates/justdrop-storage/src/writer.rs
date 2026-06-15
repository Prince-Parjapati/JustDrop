//! Chunk writer for receiving files — writes chunks to disk and tracks completion.
//!
//! Supports out-of-order chunk arrival and maintains a completion bitmap
//! for resume tracking.

use justdrop_core::error::StorageError;
use justdrop_core::types::{Sha256Hash, TransferId};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{debug, trace, warn};

/// Writes received chunks to a destination file.
pub struct ChunkWriter {
    /// Transfer ID for logging.
    transfer_id: TransferId,
    /// File index within the transfer.
    file_index: u32,
    /// Path to the destination file.
    dest_path: PathBuf,
    /// Path to the temporary file (renamed on completion).
    temp_path: PathBuf,
    /// Chunk size in bytes.
    chunk_size: u32,
    /// Total expected file size.
    #[allow(dead_code)]
    file_size: u64,
    /// Total number of chunks expected.
    total_chunks: u64,
    /// Set of chunk offsets that have been written.
    completed_chunks: HashSet<u64>,
}

impl ChunkWriter {
    /// Create a new chunk writer.
    pub async fn new(
        transfer_id: TransferId,
        file_index: u32,
        dest_path: &Path,
        chunk_size: u32,
        file_size: u64,
    ) -> Result<Self, StorageError> {
        let temp_path = dest_path.with_extension("justdrop_partial");

        // Ensure parent directory exists
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| StorageError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        // Create or open the temp file, pre-allocate space
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&temp_path)
            .await
            .map_err(|e| StorageError::Io {
                path: temp_path.clone(),
                source: e,
            })?;

        // Pre-allocate the file to avoid fragmentation
        file.set_len(file_size).await.map_err(|e| StorageError::Io {
            path: temp_path.clone(),
            source: e,
        })?;

        let total_chunks = if file_size == 0 {
            1
        } else {
            file_size.div_ceil(chunk_size as u64)
        };

        debug!(
            transfer_id = %transfer_id,
            file_index = file_index,
            dest = %dest_path.display(),
            file_size = file_size,
            total_chunks = total_chunks,
            "created chunk writer"
        );

        Ok(Self {
            transfer_id,
            file_index,
            dest_path: dest_path.to_path_buf(),
            temp_path,
            chunk_size,
            file_size,
            total_chunks,
            completed_chunks: HashSet::new(),
        })
    }

    /// Write a chunk to disk, returning its SHA-256 hash for verification.
    pub async fn write_chunk(
        &mut self,
        chunk_offset: u64,
        data: &[u8],
    ) -> Result<Sha256Hash, StorageError> {
        if chunk_offset >= self.total_chunks {
            return Err(StorageError::ChunkIntegrityFailed {
                transfer_id: self.transfer_id,
                chunk_id: chunk_offset,
            });
        }

        let byte_offset = chunk_offset * self.chunk_size as u64;

        // Write to the temp file at the correct offset
        let mut file = OpenOptions::new()
            .write(true)
            .open(&self.temp_path)
            .await
            .map_err(|e| StorageError::Io {
                path: self.temp_path.clone(),
                source: e,
            })?;

        file.seek(SeekFrom::Start(byte_offset))
            .await
            .map_err(|e| StorageError::Io {
                path: self.temp_path.clone(),
                source: e,
            })?;

        file.write_all(data).await.map_err(|e| StorageError::Io {
            path: self.temp_path.clone(),
            source: e,
        })?;

        file.flush().await.map_err(|e| StorageError::Io {
            path: self.temp_path.clone(),
            source: e,
        })?;

        // Compute SHA-256
        let sha256 = {
            let data_owned = data.to_vec();
            tokio::task::spawn_blocking(move || {
                let mut hasher = Sha256::new();
                hasher.update(&data_owned);
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                hash
            })
            .await
            .map_err(|e| StorageError::Io {
                path: self.temp_path.clone(),
                source: std::io::Error::other(e),
            })?
        };

        self.completed_chunks.insert(chunk_offset);

        trace!(
            file_index = self.file_index,
            chunk_offset = chunk_offset,
            size = data.len(),
            completed = self.completed_chunks.len(),
            total = self.total_chunks,
            "wrote chunk"
        );

        Ok(sha256)
    }

    /// Check if all chunks have been received.
    pub fn is_complete(&self) -> bool {
        self.completed_chunks.len() as u64 == self.total_chunks
    }

    /// Get the number of completed chunks.
    pub fn completed_count(&self) -> u64 {
        self.completed_chunks.len() as u64
    }

    /// Get the set of completed chunk offsets (for resume).
    pub fn completed_chunks(&self) -> &HashSet<u64> {
        &self.completed_chunks
    }

    /// Get missing chunk offsets (for resume negotiation).
    pub fn missing_chunks(&self) -> Vec<u64> {
        (0..self.total_chunks)
            .filter(|offset| !self.completed_chunks.contains(offset))
            .collect()
    }

    /// Restore completion state from resume data.
    pub fn restore_completed(&mut self, chunks: HashSet<u64>) {
        self.completed_chunks = chunks;
        debug!(
            file_index = self.file_index,
            completed = self.completed_chunks.len(),
            total = self.total_chunks,
            "restored chunk completion state"
        );
    }

    /// Finalize the file: rename from temp to destination.
    pub async fn finalize(&self) -> Result<(), StorageError> {
        if !self.is_complete() {
            warn!(
                file_index = self.file_index,
                completed = self.completed_chunks.len(),
                total = self.total_chunks,
                "attempting to finalize incomplete file"
            );
        }

        // Handle filename conflicts
        let dest = unique_path(&self.dest_path).await;

        tokio::fs::rename(&self.temp_path, &dest)
            .await
            .map_err(|e| StorageError::Io {
                path: self.dest_path.clone(),
                source: e,
            })?;

        debug!(
            dest = %dest.display(),
            "finalized file"
        );

        Ok(())
    }

    /// Clean up temp file on failure.
    pub async fn cleanup(&self) {
        if self.temp_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&self.temp_path).await {
                warn!(path = %self.temp_path.display(), error = %e, "failed to cleanup temp file");
            }
        }
    }
}

/// Generate a unique file path by appending (1), (2), etc. if the file exists.
async fn unique_path(path: &Path) -> PathBuf {
    if !tokio::fs::try_exists(path).await.unwrap_or(false) {
        return path.to_path_buf();
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(Path::new("."));

    for i in 1..1000 {
        let new_name = if ext.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{ext}")
        };
        let new_path = parent.join(new_name);
        if !tokio::fs::try_exists(&new_path).await.unwrap_or(false) {
            return new_path;
        }
    }

    // Fallback: use UUID
    let uuid = uuid::Uuid::new_v4();
    if ext.is_empty() {
        parent.join(format!("{stem}_{uuid}"))
    } else {
        parent.join(format!("{stem}_{uuid}.{ext}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_and_finalize() {
        let tmp = std::env::temp_dir().join("justdrop_writer_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let dest = tmp.join("output.bin");
        let transfer_id = uuid::Uuid::new_v4();

        let mut writer = ChunkWriter::new(transfer_id, 0, &dest, 256, 512).await.unwrap();

        assert!(!writer.is_complete());
        assert_eq!(writer.completed_count(), 0);

        let _hash0 = writer.write_chunk(0, &[0xAA; 256]).await.unwrap();
        assert_eq!(writer.completed_count(), 1);

        let _hash1 = writer.write_chunk(1, &[0xBB; 256]).await.unwrap();
        assert!(writer.is_complete());

        writer.finalize().await.unwrap();
        assert!(dest.exists());

        let content = std::fs::read(&dest).unwrap();
        assert_eq!(content.len(), 512);
        assert_eq!(&content[..256], &[0xAA; 256]);
        assert_eq!(&content[256..], &[0xBB; 256]);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn missing_chunks_tracking() {
        let tmp = std::env::temp_dir().join("justdrop_writer_missing_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let dest = tmp.join("output.bin");
        let transfer_id = uuid::Uuid::new_v4();

        let mut writer = ChunkWriter::new(transfer_id, 0, &dest, 256, 768).await.unwrap();

        // Write chunks 0 and 2 (skip 1)
        writer.write_chunk(0, &[0xAA; 256]).await.unwrap();
        writer.write_chunk(2, &[0xCC; 256]).await.unwrap();

        let missing = writer.missing_chunks();
        assert_eq!(missing, vec![1]);

        writer.cleanup().await;
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
