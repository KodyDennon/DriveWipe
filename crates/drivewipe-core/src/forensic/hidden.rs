use serde::{Deserialize, Serialize};

/// Results of hidden area detection (HPA/DCO, hidden partitions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenAreaResult {
    /// Whether HPA (Host Protected Area) was detected.
    pub hpa_detected: bool,
    /// HPA size in bytes, if detected.
    pub hpa_size: Option<u64>,
    /// Whether DCO (Device Configuration Overlay) was detected.
    pub dco_detected: bool,
    /// DCO size in bytes, if detected.
    pub dco_size: Option<u64>,
    /// Hidden partitions found.
    pub hidden_partitions: Vec<HiddenPartition>,
    /// Summary message.
    pub summary: String,
}

/// A detected hidden partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenPartition {
    /// Start offset in bytes.
    pub start_offset: u64,
    /// Size in bytes.
    pub size: u64,
    /// Description or type.
    pub description: String,
}
