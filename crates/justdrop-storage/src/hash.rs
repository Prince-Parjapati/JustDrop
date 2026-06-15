//! BLAKE3 hashing utilities.
//!
//! Replaces SHA-256 for all new code paths. BLAKE3 is ~4x faster
//! than SHA-256 and supports incremental/streaming hashing natively.

use std::path::Path;
use tokio::io::AsyncReadExt;

/// Hash a byte slice with BLAKE3. Returns 32-byte hash.
pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

/// Hash a file using BLAKE3 with streaming reads.
/// Does not load the entire file into memory.
pub async fn hash_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024]; // 256 KiB read buffer

    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Incremental hasher for chunk-by-chunk hashing during transfer.
pub struct IncrementalHasher {
    inner: blake3::Hasher,
}

impl IncrementalHasher {
    pub fn new() -> Self {
        Self {
            inner: blake3::Hasher::new(),
        }
    }

    /// Feed data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    /// Finalize and return the 32-byte hash.
    pub fn finalize(self) -> [u8; 32] {
        *self.inner.finalize().as_bytes()
    }

    /// Finalize without consuming (for intermediate checks).
    pub fn finalize_peek(&self) -> [u8; 32] {
        *self.inner.finalize().as_bytes()
    }
}

impl Default for IncrementalHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a 32-byte hash as hex string.
pub fn hash_hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let h1 = hash_bytes(b"hello justdrop");
        let h2 = hash_bytes(b"hello justdrop");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_different_inputs() {
        let h1 = hash_bytes(b"hello");
        let h2 = hash_bytes(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn incremental_matches_oneshot() {
        let data = b"the quick brown fox jumps over the lazy dog";
        let oneshot = hash_bytes(data);

        let mut inc = IncrementalHasher::new();
        inc.update(&data[..10]);
        inc.update(&data[10..]);
        let incremental = inc.finalize();

        assert_eq!(oneshot, incremental);
    }

    #[test]
    fn hex_format() {
        let hash = hash_bytes(b"test");
        let hex = hash_hex(&hash);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn hash_file_works() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"file content for hashing").unwrap();

        let file_hash = hash_file(tmp.path()).await.unwrap();
        let expected = hash_bytes(b"file content for hashing");
        assert_eq!(file_hash, expected);
    }
}
