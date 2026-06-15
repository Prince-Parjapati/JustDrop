//! # JustDrop Storage
//!
//! File I/O layer providing chunked reading, chunk writing with completion tracking,
//! transfer resume state persistence, BLAKE3 integrity verification, and zstd compression.

pub mod chunker;
pub mod compress;
pub mod hash;
pub mod integrity;
pub mod resume;
pub mod writer;

pub use chunker::{hash_file, FileChunker, ReadChunk};
pub use compress::{compress, decompress, should_compress};
pub use hash::{hash_bytes, hash_hex, IncrementalHasher};
pub use integrity::{verify_chunk, verify_file};
pub use resume::{ResumeManager, TransferResumeState};
pub use writer::ChunkWriter;

