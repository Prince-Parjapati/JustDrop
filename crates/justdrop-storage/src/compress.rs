//! Zstd compression for file chunks.
//!
//! Compression is negotiated per-transfer via the manifest.
//! Chunks are compressed individually so they can be decompressed
//! independently for resume support.

use std::io::Read;

/// Default zstd compression level. Level 3 provides a good
/// balance of speed and ratio for real-time file transfer.
const DEFAULT_LEVEL: i32 = 3;

/// Compress a chunk of data using zstd.
pub fn compress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    zstd::encode_all(data, DEFAULT_LEVEL).map_err(|e| CompressionError::Compress(e.to_string()))
}

/// Decompress a zstd-compressed chunk.
pub fn decompress(data: &[u8], max_size: usize) -> Result<Vec<u8>, CompressionError> {
    let decoder =
        zstd::Decoder::new(data).map_err(|e| CompressionError::Decompress(e.to_string()))?;

    let mut buf = Vec::new();
    let mut limited = decoder.take(max_size as u64 + 1);
    limited
        .read_to_end(&mut buf)
        .map_err(|e| CompressionError::Decompress(e.to_string()))?;

    if buf.len() > max_size {
        return Err(CompressionError::TooLarge {
            size: buf.len(),
            max: max_size,
        });
    }

    Ok(buf)
}

/// Check if compression would be beneficial for this data.
/// Returns false for already-compressed formats (jpeg, png, zip, etc).
pub fn should_compress(mime_type: &str) -> bool {
    !matches!(
        mime_type,
        "image/jpeg"
            | "image/png"
            | "image/webp"
            | "image/gif"
            | "video/mp4"
            | "video/webm"
            | "audio/mp3"
            | "audio/aac"
            | "audio/ogg"
            | "application/zip"
            | "application/gzip"
            | "application/x-7z-compressed"
            | "application/x-rar-compressed"
            | "application/zstd"
    )
}

/// Compression ratio as a percentage (0-100).
/// 80 means the compressed size is 80% of the original.
pub fn ratio(original: usize, compressed: usize) -> f64 {
    if original == 0 {
        return 100.0;
    }
    (compressed as f64 / original as f64) * 100.0
}

#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("compression failed: {0}")]
    Compress(String),
    #[error("decompression failed: {0}")]
    Decompress(String),
    #[error("decompressed data too large: {size} bytes (max {max})")]
    TooLarge { size: usize, max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let data = b"hello world, this is a test of zstd compression in JustDrop";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed, 1024).unwrap();
        assert_eq!(&decompressed, data);
    }

    #[test]
    fn compresses_text_well() {
        let data = "a]".repeat(10_000);
        let compressed = compress(data.as_bytes()).unwrap();
        assert!(compressed.len() < data.len() / 2);
    }

    #[test]
    fn max_size_enforced() {
        let data = vec![0u8; 1024];
        let compressed = compress(&data).unwrap();
        let result = decompress(&compressed, 512);
        assert!(result.is_err());
    }

    #[test]
    fn empty_data() {
        let compressed = compress(b"").unwrap();
        let decompressed = decompress(&compressed, 1024).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn should_compress_logic() {
        assert!(should_compress("text/plain"));
        assert!(should_compress("application/pdf"));
        assert!(should_compress("application/octet-stream"));
        assert!(!should_compress("image/jpeg"));
        assert!(!should_compress("video/mp4"));
        assert!(!should_compress("application/zip"));
    }

    #[test]
    fn ratio_calculation() {
        assert!((ratio(100, 50) - 50.0).abs() < f64::EPSILON);
        assert!((ratio(100, 100) - 100.0).abs() < f64::EPSILON);
        assert!((ratio(0, 0) - 100.0).abs() < f64::EPSILON);
    }
}
