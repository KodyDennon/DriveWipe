pub mod entropy;
pub mod export;
pub mod hidden;
pub mod report;
pub mod sampling;
pub mod signatures;

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::io::RawDeviceIo;
use crate::progress::ProgressEvent;
use crate::session::CancellationToken;

/// Configuration for a forensic scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicConfig {
    /// Whether to compute per-sector entropy.
    pub entropy_analysis: bool,
    /// Whether to scan for file signatures.
    pub signature_scan: bool,
    /// Whether to do statistical sampling.
    pub statistical_sampling: bool,
    /// Whether to check for hidden areas (HPA/DCO).
    pub hidden_area_check: bool,
    /// Sample size for statistical sampling (0.0-1.0).
    pub sample_ratio: f64,
    /// Block size for scanning.
    pub block_size: usize,
}

impl Default for ForensicConfig {
    fn default() -> Self {
        Self {
            entropy_analysis: true,
            signature_scan: true,
            statistical_sampling: true,
            hidden_area_check: true,
            sample_ratio: 0.01, // 1% sampling
            block_size: 4096,
        }
    }
}

/// Results from a forensic analysis session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicResult {
    pub session_id: Uuid,
    pub device_path: String,
    pub device_serial: String,
    pub entropy_stats: Option<entropy::EntropyStats>,
    pub signature_hits: Vec<signatures::FileSignatureHit>,
    pub sampling_result: Option<sampling::SamplingResult>,
    pub hidden_areas: Option<hidden::HiddenAreaResult>,
    pub duration_secs: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Orchestrator for forensic analysis operations.
pub struct ForensicSession {
    pub config: ForensicConfig,
}

impl ForensicSession {
    pub fn new(config: ForensicConfig) -> Self {
        Self { config }
    }

    /// Run a full forensic scan according to the configuration.
    pub fn execute(
        &self,
        device: &mut dyn RawDeviceIo,
        device_path: &str,
        device_serial: &str,
        progress_tx: &Sender<ProgressEvent>,
        cancel_token: &CancellationToken,
    ) -> Result<ForensicResult> {
        let session_id = Uuid::new_v4();
        let start = std::time::Instant::now();

        let _ = progress_tx.send(ProgressEvent::ForensicScanStarted {
            session_id,
            device_path: device_path.to_string(),
            scan_type: "comprehensive".to_string(),
        });

        let mut signature_hits = Vec::new();
        let mut entropy_stats = None;
        let mut sampling_result = None;
        let mut total_findings: u32 = 0;

        // Entropy analysis
        if self.config.entropy_analysis && !cancel_token.is_cancelled() {
            log::info!("Running entropy analysis...");
            match entropy::analyze_entropy(device, self.config.block_size) {
                Ok(stats) => {
                    entropy_stats = Some(stats);
                }
                Err(e) => log::warn!("Entropy analysis failed: {}", e),
            }
        }

        // Signature scan
        if self.config.signature_scan && !cancel_token.is_cancelled() {
            log::info!("Running signature scan...");
            match signatures::scan_signatures(device, self.config.block_size) {
                Ok(hits) => {
                    total_findings += hits.len() as u32;
                    signature_hits = hits;
                }
                Err(e) => log::warn!("Signature scan failed: {}", e),
            }
        }

        // Statistical sampling
        if self.config.statistical_sampling && !cancel_token.is_cancelled() {
            log::info!("Running statistical sampling...");
            match sampling::statistical_sample(device, self.config.sample_ratio) {
                Ok(result) => {
                    sampling_result = Some(result);
                }
                Err(e) => log::warn!("Statistical sampling failed: {}", e),
            }
        }

        let duration = start.elapsed().as_secs_f64();

        let _ = progress_tx.send(ProgressEvent::ForensicScanCompleted {
            session_id,
            duration_secs: duration,
            total_findings,
        });

        Ok(ForensicResult {
            session_id,
            device_path: device_path.to_string(),
            device_serial: device_serial.to_string(),
            entropy_stats,
            signature_hits,
            sampling_result,
            hidden_areas: None,
            duration_secs: duration,
            timestamp: chrono::Utc::now(),
        })
    }
}
