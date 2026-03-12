pub mod block;
pub mod image;
pub mod ops;
pub mod partition_aware;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Clone mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneMode {
    /// Sector-by-sector block copy.
    Block,
    /// Partition-aware copy with resize support.
    Partition,
    /// Copy to/from a compressed/encrypted image file.
    Image,
}

/// Compression mode for clone images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    None,
    Gzip,
    Zstd,
}

/// Configuration for a clone operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneConfig {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mode: CloneMode,
    pub compression: CompressionMode,
    pub encrypt: bool,
    pub verify: bool,
    pub block_size: usize,
}

impl Default for CloneConfig {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            target: PathBuf::new(),
            mode: CloneMode::Block,
            compression: CompressionMode::None,
            encrypt: false,
            verify: true,
            block_size: 4 * 1024 * 1024, // 4 MiB
        }
    }
}

/// Result of a completed clone operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResult {
    pub session_id: Uuid,
    pub source: PathBuf,
    pub target: PathBuf,
    pub mode: CloneMode,
    pub bytes_copied: u64,
    pub duration_secs: f64,
    pub throughput_mbps: f64,
    pub verified: bool,
    pub verification_passed: Option<bool>,
    pub source_hash: Option<String>,
    pub target_hash: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}
