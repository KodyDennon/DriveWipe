use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::benchmark::BenchmarkResult;
use super::nvme::NvmeHealthLog;
use super::smart::SmartData;

/// A point-in-time snapshot of a drive's health data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveHealthSnapshot {
    /// When this snapshot was taken.
    pub timestamp: DateTime<Utc>,
    /// Device path.
    pub device_path: String,
    /// Device serial number.
    pub device_serial: String,
    /// Device model.
    pub device_model: String,
    /// SMART data (ATA drives).
    pub smart_data: Option<SmartData>,
    /// NVMe health log (NVMe drives).
    pub nvme_health: Option<NvmeHealthLog>,
    /// Temperature in Celsius.
    pub temperature_celsius: Option<i16>,
    /// Performance benchmark results.
    pub benchmark: Option<BenchmarkResult>,
}

impl DriveHealthSnapshot {
    /// Save snapshot to a JSON file.
    pub fn save(&self, path: &Path) -> crate::error::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            crate::error::DriveWipeError::Health(format!("Failed to serialize snapshot: {e}"))
        })?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::error::DriveWipeError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(path, json).map_err(|e| crate::error::DriveWipeError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        Ok(())
    }

    /// Load snapshot from a JSON file.
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| crate::error::DriveWipeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        let snapshot: Self = serde_json::from_str(&contents).map_err(|e| {
            crate::error::DriveWipeError::Health(format!("Failed to parse snapshot: {e}"))
        })?;

        Ok(snapshot)
    }

    /// Overall health assessment.
    pub fn is_healthy(&self) -> bool {
        if let Some(ref smart) = self.smart_data
            && !smart.healthy
        {
            return false;
        }
        if let Some(ref nvme) = self.nvme_health
            && !nvme.is_healthy()
        {
            return false;
        }
        true
    }
}
