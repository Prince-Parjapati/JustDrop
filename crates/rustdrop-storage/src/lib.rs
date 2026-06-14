//! # RustDrop Storage
//!
//! File I/O layer providing chunked reading, chunk writing with completion tracking,
//! transfer resume state persistence, and SHA-256 integrity verification.

pub mod chunker;
pub mod integrity;
pub mod resume;
pub mod writer;

pub use chunker::{hash_file, FileChunker, ReadChunk};
pub use integrity::{verify_chunk, verify_file};
pub use resume::{ResumeManager, TransferResumeState};
pub use writer::ChunkWriter;
