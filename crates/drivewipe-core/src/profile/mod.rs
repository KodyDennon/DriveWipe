pub mod database;
pub mod matcher;

use serde::{Deserialize, Serialize};

pub use database::ProfileDatabase;
pub use matcher::ProfileMatcher;

/// A drive profile containing manufacturer-specific optimizations and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveProfile {
    /// Manufacturer name.
    pub manufacturer: String,
    /// Human-readable profile name.
    pub name: String,
    /// Regex patterns to match against drive model strings.
    #[serde(default)]
    pub model_patterns: Vec<String>,
    /// Type of controller (e.g. "Samsung Phoenix", "Phison E12").
    pub controller_type: Option<String>,
    /// Over-provisioning ratio (e.g. 0.07 for 7%).
    #[serde(default)]
    pub over_provisioning_ratio: f64,
    /// Whether the drive supports ATA/NVMe sanitize commands.
    #[serde(default)]
    pub sanitize_support: bool,
    /// Recommended wipe method for this drive.
    pub recommended_method: Option<String>,
    /// Known quirks or issues with this drive model.
    #[serde(default)]
    pub quirks: Vec<String>,
    /// Performance characteristics.
    #[serde(default)]
    pub performance: DrivePerformance,
}

/// Performance characteristics of a drive model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrivePerformance {
    /// Typical sequential write speed in MiB/s.
    pub sequential_write_mbps: Option<f64>,
    /// Typical sequential read speed in MiB/s.
    pub sequential_read_mbps: Option<f64>,
    /// Whether the drive has an SLC cache that causes write speed cliffs.
    #[serde(default)]
    pub has_slc_cache: bool,
    /// SLC cache size in bytes, if known.
    pub slc_cache_bytes: Option<u64>,
    /// Post-cache write speed in MiB/s.
    pub post_cache_write_mbps: Option<f64>,
}
