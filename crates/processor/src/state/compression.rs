//! Compression algorithms for state backends
//!
//! This module provides transparent compression and decompression for state backends,
//! supporting multiple algorithms with configurable compression levels and thresholds.
//!
//! ## Supported Algorithms
//!
//! - **LZ4**: Fast compression with good compression ratio (default)
//! - **Snappy**: Very fast compression, moderate compression ratio
//! - **Zstd**: Best compression ratio, moderate speed
//! - **None**: No compression (passthrough)
//!
//! ## Example
//!
//! ```rust
//! use processor::state::compression::{CompressionConfig, CompressionAlgorithm, compress, decompress};
//!
//! let config = CompressionConfig::default()
//!     .with_algorithm(CompressionAlgorithm::Lz4)
//!     .with_level(4)
//!     .with_threshold(1024); // Only compress data > 1KB
//!
//! let data = b"Hello, World!".repeat(100);
//!
//! // Compress
//! let compressed = compress(&data, &config).unwrap();
//! println!("Original: {} bytes, Compressed: {} bytes", data.len(), compressed.len());
//!
//! // Decompress
//! let decompressed = decompress(&compressed, &config).unwrap();
//! assert_eq!(data, decompressed);
//! ```

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use thiserror::Error;

/// Compression algorithm selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CompressionAlgorithm {
    /// No compression (passthrough)
    None,
    /// LZ4 compression - fast with good compression ratio
    Lz4,
    /// Snappy compression - very fast with moderate compression
    Snappy,
    /// Zstd compression - best ratio with moderate speed
    Zstd,
}

impl CompressionAlgorithm {
    /// Get the algorithm name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lz4 => "lz4",
            Self::Snappy => "snappy",
            Self::Zstd => "zstd",
        }
    }

    /// Get the magic byte prefix for this algorithm
    ///
    /// This prefix is prepended to compressed data to identify the algorithm used.
    fn magic_byte(&self) -> u8 {
        match self {
            Self::None => 0x00,
            Self::Lz4 => 0x01,
            Self::Snappy => 0x02,
            Self::Zstd => 0x03,
        }
    }

    /// Parse algorithm from magic byte
    fn from_magic_byte(byte: u8) -> Result<Self, CompressionError> {
        match byte {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Lz4),
            0x02 => Ok(Self::Snappy),
            0x03 => Ok(Self::Zstd),
            _ => Err(CompressionError::InvalidMagicByte(byte)),
        }
    }
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        Self::Lz4 // Fast and good compression ratio
    }
}

impl std::fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,

    /// Compression level (algorithm-specific):
    /// - LZ4: 0 (default) to 16 (max)
    /// - Snappy: Ignored (no levels)
    /// - Zstd: 1 to 22 (default: 3)
    pub level: i32,

    /// Minimum size in bytes before compression is applied
    /// Data smaller than this threshold is stored uncompressed
    pub threshold: usize,

    /// Enable statistics tracking
    pub enable_stats: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Lz4,
            level: 4, // Good balance for LZ4
            threshold: 256, // Compress anything >= 256 bytes
            enable_stats: false,
        }
    }
}

impl CompressionConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the compression algorithm
    pub fn with_algorithm(mut self, algorithm: CompressionAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Set the compression level
    pub fn with_level(mut self, level: i32) -> Self {
        self.level = level;
        self
    }

    /// Set the compression threshold
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Enable statistics tracking
    pub fn with_stats(mut self, enable: bool) -> Self {
        self.enable_stats = enable;
        self
    }

    /// Create a configuration with no compression
    pub fn none() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
            ..Default::default()
        }
    }

    /// Create a configuration optimized for speed
    pub fn fast() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Snappy,
            level: 0,
            threshold: 512,
            enable_stats: false,
        }
    }

    /// Create a configuration balanced for speed and size
    pub fn balanced() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Lz4,
            level: 4,
            threshold: 256,
            enable_stats: false,
        }
    }

    /// Create a configuration optimized for size
    pub fn best_size() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Zstd,
            level: 15,
            threshold: 128,
            enable_stats: false,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Number of compression operations
    pub compress_count: u64,
    /// Number of decompression operations
    pub decompress_count: u64,
    /// Total bytes input to compression
    pub bytes_in: u64,
    /// Total bytes output from compression
    pub bytes_out: u64,
    /// Total bytes input to decompression
    pub decompress_bytes_in: u64,
    /// Total bytes output from decompression
    pub decompress_bytes_out: u64,
    /// Number of times threshold prevented compression
    pub threshold_skipped: u64,
}

impl CompressionStats {
    /// Get the compression ratio (0.0 to 1.0, lower is better)
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_in == 0 {
            1.0
        } else {
            self.bytes_out as f64 / self.bytes_in as f64
        }
    }

    /// Get the space saved in bytes
    pub fn space_saved(&self) -> i64 {
        self.bytes_in as i64 - self.bytes_out as i64
    }

    /// Get the space saved as a percentage
    pub fn space_saved_pct(&self) -> f64 {
        if self.bytes_in == 0 {
            0.0
        } else {
            (1.0 - self.compression_ratio()) * 100.0
        }
    }
}

/// Compression errors
#[derive(Debug, Error)]
pub enum CompressionError {
    /// LZ4 compression error
    #[error("LZ4 compression error: {0}")]
    Lz4Error(String),

    /// Snappy compression error
    #[error("Snappy compression error: {0}")]
    SnappyError(String),

    /// Zstd compression error
    #[error("Zstd compression error: {0}")]
    ZstdError(String),

    /// Invalid magic byte in compressed data
    #[error("Invalid compression magic byte: {0:#x}")]
    InvalidMagicByte(u8),

    /// Data too short to contain magic byte
    #[error("Data too short (must be at least 1 byte)")]
    DataTooShort,

    /// IO error during compression/decompression
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

pub type CompressionResult<T> = Result<T, CompressionError>;

/// Compress data according to the configuration
///
/// The compressed data will have a 1-byte magic header prepended to identify
/// the compression algorithm used. This allows for transparent decompression.
///
/// # Arguments
///
/// * `data` - Input data to compress
/// * `config` - Compression configuration
///
/// # Returns
///
/// Compressed data with magic byte prefix
///
/// # Example
///
/// ```
/// # use processor::state::compression::{compress, CompressionConfig};
/// let data = b"Hello, World!".repeat(100);
/// let config = CompressionConfig::balanced();
/// let compressed = compress(&data, &config).unwrap();
/// assert!(compressed.len() < data.len());
/// ```
pub fn compress(data: &[u8], config: &CompressionConfig) -> CompressionResult<Vec<u8>> {
    // Check threshold - don't compress small data
    if data.len() < config.threshold {
        let mut result = Vec::with_capacity(1 + data.len());
        result.push(CompressionAlgorithm::None.magic_byte());
        result.extend_from_slice(data);
        return Ok(result);
    }

    match config.algorithm {
        CompressionAlgorithm::None => {
            let mut result = Vec::with_capacity(1 + data.len());
            result.push(CompressionAlgorithm::None.magic_byte());
            result.extend_from_slice(data);
            Ok(result)
        }

        CompressionAlgorithm::Lz4 => {
            let compressed = lz4::block::compress(data, Some(lz4::block::CompressionMode::HIGHCOMPRESSION(config.level)), false)
                .map_err(|e| CompressionError::Lz4Error(e.to_string()))?;

            let mut result = Vec::with_capacity(1 + compressed.len());
            result.push(CompressionAlgorithm::Lz4.magic_byte());
            result.extend_from_slice(&compressed);
            Ok(result)
        }

        CompressionAlgorithm::Snappy => {
            let mut encoder = snap::raw::Encoder::new();
            let compressed = encoder
                .compress_vec(data)
                .map_err(|e| CompressionError::SnappyError(e.to_string()))?;

            let mut result = Vec::with_capacity(1 + compressed.len());
            result.push(CompressionAlgorithm::Snappy.magic_byte());
            result.extend_from_slice(&compressed);
            Ok(result)
        }

        CompressionAlgorithm::Zstd => {
            let mut compressed = Vec::new();
            let mut encoder = zstd::Encoder::new(&mut compressed, config.level)?;
            encoder.write_all(data)?;
            encoder.finish()?;

            let mut result = Vec::with_capacity(1 + compressed.len());
            result.push(CompressionAlgorithm::Zstd.magic_byte());
            result.extend_from_slice(&compressed);
            Ok(result)
        }
    }
}

/// Decompress data that was compressed with `compress()`
///
/// The function automatically detects the compression algorithm from the magic byte
/// and decompresses accordingly.
///
/// # Arguments
///
/// * `data` - Compressed data with magic byte prefix
/// * `config` - Compression configuration (not currently used but kept for API consistency)
///
/// # Returns
///
/// Decompressed data
///
/// # Example
///
/// ```
/// # use processor::state::compression::{compress, decompress, CompressionConfig};
/// let original = b"Hello, World!".repeat(100);
/// let config = CompressionConfig::balanced();
/// let compressed = compress(&original, &config).unwrap();
/// let decompressed = decompress(&compressed, &config).unwrap();
/// assert_eq!(original.to_vec(), decompressed);
/// ```
pub fn decompress(data: &[u8], _config: &CompressionConfig) -> CompressionResult<Vec<u8>> {
    if data.is_empty() {
        return Err(CompressionError::DataTooShort);
    }

    let magic_byte = data[0];
    let compressed_data = &data[1..];

    let algorithm = CompressionAlgorithm::from_magic_byte(magic_byte)?;

    match algorithm {
        CompressionAlgorithm::None => {
            Ok(compressed_data.to_vec())
        }

        CompressionAlgorithm::Lz4 => {
            // LZ4 needs to know the decompressed size. We'll use a heuristic of 4x the compressed size
            // and let LZ4 tell us the actual size
            let decompressed = lz4::block::decompress(compressed_data, None)
                .map_err(|e| CompressionError::Lz4Error(e.to_string()))?;
            Ok(decompressed)
        }

        CompressionAlgorithm::Snappy => {
            let mut decoder = snap::raw::Decoder::new();
            let decompressed = decoder
                .decompress_vec(compressed_data)
                .map_err(|e| CompressionError::SnappyError(e.to_string()))?;
            Ok(decompressed)
        }

        CompressionAlgorithm::Zstd => {
            let mut decompressed = Vec::new();
            let mut decoder = zstd::Decoder::new(compressed_data)?;
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA_SMALL: &[u8] = b"Hello, World!";
    const TEST_DATA_MEDIUM: &[u8] = b"This is a longer test string that should definitely be compressed because it has enough repetition. This is a longer test string that should definitely be compressed because it has enough repetition.";

    fn generate_large_data(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 256) as u8).collect()
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(CompressionAlgorithm::None.to_string(), "none");
        assert_eq!(CompressionAlgorithm::Lz4.to_string(), "lz4");
        assert_eq!(CompressionAlgorithm::Snappy.to_string(), "snappy");
        assert_eq!(CompressionAlgorithm::Zstd.to_string(), "zstd");
    }

    #[test]
    fn test_magic_byte_roundtrip() {
        for algo in &[
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::Zstd,
        ] {
            let byte = algo.magic_byte();
            let parsed = CompressionAlgorithm::from_magic_byte(byte).unwrap();
            assert_eq!(*algo, parsed);
        }
    }

    #[test]
    fn test_invalid_magic_byte() {
        let result = CompressionAlgorithm::from_magic_byte(0xFF);
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_config_presets() {
        let fast = CompressionConfig::fast();
        assert_eq!(fast.algorithm, CompressionAlgorithm::Snappy);

        let balanced = CompressionConfig::balanced();
        assert_eq!(balanced.algorithm, CompressionAlgorithm::Lz4);

        let best = CompressionConfig::best_size();
        assert_eq!(best.algorithm, CompressionAlgorithm::Zstd);

        let none = CompressionConfig::none();
        assert_eq!(none.algorithm, CompressionAlgorithm::None);
    }

    #[test]
    fn test_compression_none() {
        let config = CompressionConfig::none();
        let compressed = compress(TEST_DATA_MEDIUM, &config).unwrap();
        let decompressed = decompress(&compressed, &config).unwrap();

        assert_eq!(decompressed, TEST_DATA_MEDIUM);
        // No compression should add only magic byte
        assert_eq!(compressed.len(), TEST_DATA_MEDIUM.len() + 1);
    }

    #[test]
    fn test_compression_lz4() {
        let config = CompressionConfig::default().with_algorithm(CompressionAlgorithm::Lz4);
        let compressed = compress(TEST_DATA_MEDIUM, &config).unwrap();
        let decompressed = decompress(&compressed, &config).unwrap();

        assert_eq!(decompressed, TEST_DATA_MEDIUM);
        // Should achieve some compression on repetitive data
        assert!(compressed.len() < TEST_DATA_MEDIUM.len());
    }

    #[test]
    fn test_compression_snappy() {
        let config = CompressionConfig::default().with_algorithm(CompressionAlgorithm::Snappy);
        let compressed = compress(TEST_DATA_MEDIUM, &config).unwrap();
        let decompressed = decompress(&compressed, &config).unwrap();

        assert_eq!(decompressed, TEST_DATA_MEDIUM);
        assert!(compressed.len() < TEST_DATA_MEDIUM.len());
    }

    #[test]
    fn test_compression_zstd() {
        let config = CompressionConfig::default().with_algorithm(CompressionAlgorithm::Zstd);
        let compressed = compress(TEST_DATA_MEDIUM, &config).unwrap();
        let decompressed = decompress(&compressed, &config).unwrap();

        assert_eq!(decompressed, TEST_DATA_MEDIUM);
        assert!(compressed.len() < TEST_DATA_MEDIUM.len());
    }

    #[test]
    fn test_threshold_prevents_compression() {
        let config = CompressionConfig::default()
            .with_algorithm(CompressionAlgorithm::Lz4)
            .with_threshold(1000); // Higher than test data

        let compressed = compress(TEST_DATA_SMALL, &config).unwrap();

        // Should not be compressed (only magic byte added)
        assert_eq!(compressed[0], CompressionAlgorithm::None.magic_byte());
        assert_eq!(compressed.len(), TEST_DATA_SMALL.len() + 1);

        let decompressed = decompress(&compressed, &config).unwrap();
        assert_eq!(decompressed, TEST_DATA_SMALL);
    }

    #[test]
    fn test_large_data_compression() {
        let large_data = generate_large_data(1024 * 1024); // 1MB
        let config = CompressionConfig::balanced();

        let compressed = compress(&large_data, &config).unwrap();
        let decompressed = decompress(&compressed, &config).unwrap();

        assert_eq!(decompressed, large_data);
        println!("Original: {} bytes, Compressed: {} bytes, Ratio: {:.2}%",
            large_data.len(),
            compressed.len(),
            (compressed.len() as f64 / large_data.len() as f64) * 100.0
        );
    }

    #[test]
    fn test_empty_data() {
        let config = CompressionConfig::default();
        let compressed = compress(&[], &config).unwrap();

        // Empty data below threshold, should not be compressed
        assert_eq!(compressed.len(), 1); // Just magic byte

        let decompressed = decompress(&compressed, &config).unwrap();
        assert_eq!(decompressed.len(), 0);
    }

    #[test]
    fn test_decompress_empty_fails() {
        let config = CompressionConfig::default();
        let result = decompress(&[], &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_algorithms_roundtrip() {
        let test_data = generate_large_data(10000);

        for algo in &[
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::Zstd,
        ] {
            let config = CompressionConfig::default()
                .with_algorithm(*algo)
                .with_threshold(0); // Force compression

            let compressed = compress(&test_data, &config).unwrap();
            let decompressed = decompress(&compressed, &config).unwrap();

            assert_eq!(
                decompressed, test_data,
                "Roundtrip failed for algorithm: {:?}",
                algo
            );
        }
    }

    #[test]
    fn test_compression_levels() {
        let test_data = generate_large_data(10000);

        // Test different Zstd levels
        for level in [1, 3, 10, 15] {
            let config = CompressionConfig::default()
                .with_algorithm(CompressionAlgorithm::Zstd)
                .with_level(level)
                .with_threshold(0);

            let compressed = compress(&test_data, &config).unwrap();
            let decompressed = decompress(&compressed, &config).unwrap();

            assert_eq!(decompressed, test_data);
            println!("Zstd level {}: {} bytes", level, compressed.len());
        }
    }

    #[test]
    fn test_stats() {
        let mut stats = CompressionStats::default();

        stats.compress_count = 100;
        stats.bytes_in = 10000;
        stats.bytes_out = 3000;

        assert_eq!(stats.compression_ratio(), 0.3);
        assert_eq!(stats.space_saved(), 7000);
        assert_eq!(stats.space_saved_pct(), 70.0);
    }
}
