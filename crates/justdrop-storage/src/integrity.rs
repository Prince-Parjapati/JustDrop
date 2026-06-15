//! Full-file integrity verification using SHA-256.

use justdrop_core::error::StorageError;
use justdrop_core::types::Sha256Hash;
use sha2::{Digest, Sha256};
use std::path::Path;
use tracing::info;

/// Verify the SHA-256 hash of a completed file against the expected hash.
pub async fn verify_file(path: &Path, expected: &Sha256Hash) -> Result<bool, StorageError> {
    let actual = crate::chunker::hash_file(path).await?;
    let matches = &actual == expected;

    if matches {
        info!(path = %path.display(), "file integrity verified");
    } else {
        let expected_hex: String = expected.iter().map(|b| format!("{b:02x}")).collect();
        let actual_hex: String = actual.iter().map(|b| format!("{b:02x}")).collect();
        info!(
            path = %path.display(),
            expected = %expected_hex,
            actual = %actual_hex,
            "file integrity FAILED"
        );
    }

    Ok(matches)
}

/// Verify a chunk's hash matches the expected value.
pub fn verify_chunk(data: &[u8], expected: &Sha256Hash) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let actual: [u8; 32] = result.into();
    actual == *expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verify_file_success() {
        let tmp = std::env::temp_dir().join("justdrop_integrity_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("test.bin");
        let data = b"integrity test data";
        std::fs::write(&path, data).unwrap();

        let hash = crate::chunker::hash_file(&path).await.unwrap();
        assert!(verify_file(&path, &hash).await.unwrap());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn verify_file_failure() {
        let tmp = std::env::temp_dir().join("justdrop_integrity_fail_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("test.bin");
        std::fs::write(&path, b"data").unwrap();

        let wrong_hash = [0xFF; 32];
        assert!(!verify_file(&path, &wrong_hash).await.unwrap());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn verify_chunk_matches() {
        let data = b"chunk data";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash: [u8; 32] = hasher.finalize().into();
        assert!(verify_chunk(data, &hash));
    }

    #[test]
    fn verify_chunk_mismatch() {
        let data = b"chunk data";
        let wrong = [0u8; 32];
        assert!(!verify_chunk(data, &wrong));
    }
}
