//! File chunker that streams fixed-size chunks from disk with per-chunk SHA-256.
//!
//! Designed for zero-copy-adjacent performance: reads directly from disk
//! into a reusable buffer, computes SHA-256 inline, and yields `ChunkData`
//! via an async stream.

use bytes::Bytes;
use justdrop_core::error::StorageError;
use justdrop_core::types::{ChunkData, ChunkId, Sha256Hash};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tracing::{debug, trace};

/// Streams chunks from a file on disk.
pub struct FileChunker {
    path: PathBuf,
    file_index: u32,
    chunk_size: u32,
    file_size: u64,
}

/// A chunk read from disk, ready for transfer.
pub struct ReadChunk {
    /// Chunk identifier.
    pub id: ChunkId,
    /// Chunk data.
    pub data: Bytes,
    /// SHA-256 hash of this chunk's data.
    pub sha256: Sha256Hash,
}

impl FileChunker {
    /// Open a file for chunked reading.
    pub async fn open(
        path: &Path,
        file_index: u32,
        chunk_size: u32,
    ) -> Result<Self, StorageError> {
        let metadata = tokio::fs::metadata(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::FileNotFound {
                    path: path.to_path_buf(),
                }
            } else {
                StorageError::Io {
                    path: path.to_path_buf(),
                    source: e,
                }
            }
        })?;

        let file_size = metadata.len();
        debug!(
            path = %path.display(),
            file_size = file_size,
            chunk_size = chunk_size,
            chunks = (file_size + chunk_size as u64 - 1) / chunk_size as u64,
            "opened file for chunking"
        );

        Ok(Self {
            path: path.to_path_buf(),
            file_index,
            chunk_size,
            file_size,
        })
    }

    /// Total number of chunks for this file.
    pub fn chunk_count(&self) -> u64 {
        if self.file_size == 0 {
            1
        } else {
            (self.file_size + self.chunk_size as u64 - 1) / self.chunk_size as u64
        }
    }

    /// File size in bytes.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Read a specific chunk by offset.
    pub async fn read_chunk(&self, chunk_offset: u64) -> Result<ReadChunk, StorageError> {
        let byte_offset = chunk_offset * self.chunk_size as u64;
        let remaining = self.file_size.saturating_sub(byte_offset);
        let read_size = (remaining as usize).min(self.chunk_size as usize);

        let mut file = File::open(&self.path).await.map_err(|e| StorageError::Io {
            path: self.path.clone(),
            source: e,
        })?;

        if byte_offset > 0 {
            file.seek(SeekFrom::Start(byte_offset))
                .await
                .map_err(|e| StorageError::Io {
                    path: self.path.clone(),
                    source: e,
                })?;
        }

        let mut buf = vec![0u8; read_size];
        let mut total_read = 0;

        while total_read < read_size {
            let n = file
                .read(&mut buf[total_read..])
                .await
                .map_err(|e| StorageError::Io {
                    path: self.path.clone(),
                    source: e,
                })?;
            if n == 0 {
                break;
            }
            total_read += n;
        }

        buf.truncate(total_read);

        // Compute SHA-256 of this chunk in a blocking task to avoid starving the runtime
        let sha256 = {
            let data = buf.clone();
            tokio::task::spawn_blocking(move || {
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                hash
            })
            .await
            .map_err(|e| StorageError::Io {
                path: self.path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?
        };

        let id = ChunkId {
            file_index: self.file_index,
            chunk_offset,
        };

        trace!(
            file_index = self.file_index,
            chunk_offset = chunk_offset,
            size = total_read,
            "read chunk"
        );

        Ok(ReadChunk {
            id,
            data: Bytes::from(buf),
            sha256,
        })
    }

    /// Stream all chunks sequentially, starting from a given offset.
    ///
    /// Returns chunks via a channel for pipelined processing.
    pub async fn stream_chunks(
        &self,
        start_chunk: u64,
        tx: tokio::sync::mpsc::Sender<Result<ReadChunk, StorageError>>,
    ) {
        let total = self.chunk_count();
        for offset in start_chunk..total {
            match self.read_chunk(offset).await {
                Ok(chunk) => {
                    if tx.send(Ok(chunk)).await.is_err() {
                        debug!("chunk receiver dropped, stopping stream");
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            }
        }
    }
}

/// Compute SHA-256 of an entire file (for manifest generation).
pub async fn hash_file(path: &Path) -> Result<Sha256Hash, StorageError> {
    let path = path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        use std::io::Read;

        let mut file = std::fs::File::open(&path).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: e,
        })?;

        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024]; // 256 KiB buffer

        loop {
            let n = file.read(&mut buf).map_err(|e| StorageError::Io {
                path: path.clone(),
                source: e,
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Ok(hash)
    })
    .await
    .map_err(|e| StorageError::Io {
        path: PathBuf::from("hash_file"),
        source: std::io::Error::new(std::io::ErrorKind::Other, e),
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn chunk_small_file() {
        let tmp = std::env::temp_dir().join("justdrop_chunker_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let file_path = tmp.join("test.bin");
        let data = vec![0xAB; 1024]; // 1 KiB
        std::fs::write(&file_path, &data).unwrap();

        let chunker = FileChunker::open(&file_path, 0, 256).await.unwrap();
        assert_eq!(chunker.chunk_count(), 4);
        assert_eq!(chunker.file_size(), 1024);

        let chunk0 = chunker.read_chunk(0).await.unwrap();
        assert_eq!(chunk0.data.len(), 256);
        assert_eq!(chunk0.id.chunk_offset, 0);

        let chunk3 = chunker.read_chunk(3).await.unwrap();
        assert_eq!(chunk3.data.len(), 256);
        assert_eq!(chunk3.id.chunk_offset, 3);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn chunk_with_remainder() {
        let tmp = std::env::temp_dir().join("justdrop_chunker_test2");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let file_path = tmp.join("test.bin");
        let data = vec![0xCD; 500]; // 500 bytes
        std::fs::write(&file_path, &data).unwrap();

        let chunker = FileChunker::open(&file_path, 0, 256).await.unwrap();
        assert_eq!(chunker.chunk_count(), 2); // 256 + 244

        let last = chunker.read_chunk(1).await.unwrap();
        assert_eq!(last.data.len(), 244);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn hash_file_matches_manual() {
        let tmp = std::env::temp_dir().join("justdrop_hash_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let file_path = tmp.join("test.bin");
        let data = b"hello justdrop";
        std::fs::write(&file_path, data).unwrap();

        let hash = hash_file(&file_path).await.unwrap();

        // Verify manually
        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected: [u8; 32] = hasher.finalize().into();

        assert_eq!(hash, expected);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn stream_chunks_through_channel() {
        let tmp = std::env::temp_dir().join("justdrop_stream_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let file_path = tmp.join("test.bin");
        std::fs::write(&file_path, vec![0u8; 768]).unwrap(); // 3 chunks of 256

        let chunker = FileChunker::open(&file_path, 0, 256).await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);

        tokio::spawn(async move {
            chunker.stream_chunks(0, tx).await;
        });

        let mut count = 0;
        while let Some(result) = rx.recv().await {
            result.unwrap();
            count += 1;
        }
        assert_eq!(count, 3);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
