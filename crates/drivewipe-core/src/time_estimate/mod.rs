use std::collections::VecDeque;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Smoothed time estimator using EMA (Exponential Moving Average).
pub struct TimeEstimator {
    /// EMA smoothing factor (0.0 = ignore new, 1.0 = ignore old).
    alpha: f64,
    /// Current smoothed throughput in bytes/sec.
    smoothed_throughput: f64,
    /// Recent throughput samples for confidence intervals.
    samples: VecDeque<f64>,
    /// Maximum number of samples to keep.
    max_samples: usize,
    /// Total bytes to process across all passes.
    total_bytes_all_passes: u64,
    /// Current pass number (1-indexed).
    current_pass: u32,
    /// Total number of passes.
    total_passes: u32,
    /// Whether this includes a verification pass.
    has_verification: bool,
    /// Bytes completed so far across all passes.
    total_bytes_completed: u64,
    /// Number of throughput updates received (for calibration period).
    update_count: u64,
    /// Minimum updates before providing an estimate.
    calibration_threshold: u64,
    /// When estimation started.
    start_time: Instant,
    /// Drive profile performance hints.
    profile_write_mbps: Option<f64>,
    /// Post-SLC-cache write speed hint.
    post_cache_write_mbps: Option<f64>,
    /// SLC cache size hint.
    slc_cache_bytes: Option<u64>,
}

/// Confidence interval for a time estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEstimate {
    /// Best case remaining time in seconds.
    pub best_secs: f64,
    /// Expected remaining time in seconds.
    pub expected_secs: f64,
    /// Worst case remaining time in seconds.
    pub worst_secs: f64,
    /// Current smoothed throughput in bytes/sec.
    pub throughput_bps: f64,
    /// Whether the estimate has been calibrated.
    pub calibrated: bool,
}

/// Per-pass breakdown of estimated time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassEstimate {
    pub pass_number: u32,
    pub pass_name: String,
    pub estimated_secs: f64,
}

/// Historical performance entry for a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceHistory {
    pub device_serial: String,
    pub device_model: String,
    pub method_id: String,
    pub average_throughput_mbps: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl TimeEstimator {
    pub fn new(
        total_bytes_per_pass: u64,
        total_passes: u32,
        has_verification: bool,
    ) -> Self {
        Self {
            alpha: 0.1,
            smoothed_throughput: 0.0,
            samples: VecDeque::with_capacity(120),
            max_samples: 120,
            total_bytes_all_passes: total_bytes_per_pass * total_passes as u64,
            current_pass: 1,
            total_passes,
            has_verification,
            total_bytes_completed: 0,
            update_count: 0,
            calibration_threshold: 5,
            start_time: Instant::now(),
            profile_write_mbps: None,
            post_cache_write_mbps: None,
            slc_cache_bytes: None,
        }
    }

    /// Set drive profile performance hints for better initial estimates.
    pub fn set_profile_hints(
        &mut self,
        write_mbps: Option<f64>,
        post_cache_mbps: Option<f64>,
        cache_bytes: Option<u64>,
    ) {
        self.profile_write_mbps = write_mbps;
        self.post_cache_write_mbps = post_cache_mbps;
        self.slc_cache_bytes = cache_bytes;

        // Use profile data for initial estimate before calibration
        if let Some(mbps) = write_mbps {
            self.smoothed_throughput = mbps * 1024.0 * 1024.0;
        }
    }

    /// Update with a new throughput measurement.
    pub fn update(&mut self, throughput_bps: f64, bytes_completed_total: u64, current_pass: u32) {
        if throughput_bps <= 0.0 {
            return;
        }

        self.current_pass = current_pass;
        self.total_bytes_completed = bytes_completed_total;
        self.update_count += 1;

        // EMA update
        if self.smoothed_throughput <= 0.0 {
            self.smoothed_throughput = throughput_bps;
        } else {
            self.smoothed_throughput =
                self.alpha * throughput_bps + (1.0 - self.alpha) * self.smoothed_throughput;
        }

        // Keep sample history for confidence intervals
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(throughput_bps);
    }

    /// Get the current time estimate with confidence intervals.
    pub fn estimate(&self) -> TimeEstimate {
        let calibrated = self.update_count >= self.calibration_threshold;

        if self.smoothed_throughput <= 0.0 {
            return TimeEstimate {
                best_secs: 0.0,
                expected_secs: 0.0,
                worst_secs: 0.0,
                throughput_bps: 0.0,
                calibrated: false,
            };
        }

        let remaining_bytes = self.total_bytes_all_passes.saturating_sub(self.total_bytes_completed);
        let verification_bytes = if self.has_verification {
            self.total_bytes_all_passes / self.total_passes as u64
        } else {
            0
        };
        let total_remaining = remaining_bytes + verification_bytes;

        let expected_secs = total_remaining as f64 / self.smoothed_throughput;

        // Calculate confidence intervals from sample variance
        let (best_secs, worst_secs) = if self.samples.len() >= 10 {
            let mean: f64 = self.samples.iter().sum::<f64>() / self.samples.len() as f64;
            let variance: f64 = self.samples.iter()
                .map(|s| (s - mean).powi(2))
                .sum::<f64>() / self.samples.len() as f64;
            let stddev = variance.sqrt();

            let best_throughput = (mean + stddev).max(1.0);
            let worst_throughput = (mean - stddev).max(1.0);

            (
                total_remaining as f64 / best_throughput,
                total_remaining as f64 / worst_throughput,
            )
        } else {
            (expected_secs * 0.8, expected_secs * 1.5)
        };

        TimeEstimate {
            best_secs,
            expected_secs,
            worst_secs,
            throughput_bps: self.smoothed_throughput,
            calibrated,
        }
    }

    /// Get per-pass ETA breakdown.
    pub fn pass_estimates(&self, pass_names: &[(u32, String)]) -> Vec<PassEstimate> {
        if self.smoothed_throughput <= 0.0 {
            return Vec::new();
        }

        let bytes_per_pass = self.total_bytes_all_passes / self.total_passes as u64;

        pass_names
            .iter()
            .map(|(num, name)| PassEstimate {
                pass_number: *num,
                pass_name: name.clone(),
                estimated_secs: bytes_per_pass as f64 / self.smoothed_throughput,
            })
            .collect()
    }

    /// Load historical performance data for a device.
    pub fn load_history(history_dir: &Path, device_serial: &str) -> Vec<PerformanceHistory> {
        let path = history_dir.join(format!("{device_serial}.json"));
        if !path.exists() {
            return Vec::new();
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Save a performance entry for future reference.
    pub fn save_history(
        history_dir: &Path,
        entry: &PerformanceHistory,
    ) -> crate::error::Result<()> {
        std::fs::create_dir_all(history_dir).map_err(|e| crate::error::DriveWipeError::Io {
            path: history_dir.to_path_buf(),
            source: e,
        })?;

        let path = history_dir.join(format!("{}.json", entry.device_serial));
        let mut entries = Self::load_history(history_dir, &entry.device_serial);
        entries.push(entry.clone());

        // Keep last 50 entries
        if entries.len() > 50 {
            entries.drain(0..entries.len() - 50);
        }

        let json = serde_json::to_string_pretty(&entries).map_err(|e| {
            crate::error::DriveWipeError::Health(format!("Failed to serialize history: {e}"))
        })?;

        std::fs::write(&path, json).map_err(|e| crate::error::DriveWipeError::Io {
            path,
            source: e,
        })?;

        Ok(())
    }
}
