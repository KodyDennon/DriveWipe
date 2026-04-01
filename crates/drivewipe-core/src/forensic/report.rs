use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::ForensicResult;

/// A formal forensic report with chain-of-custody information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicReport {
    pub report_version: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub examiner: Option<String>,
    pub case_number: Option<String>,
    pub methodology: String,
    pub results: ForensicResult,
    pub conclusions: Vec<String>,
    pub hash_chain: Vec<HashChainEntry>,
}

/// An entry in the report's hash chain for integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashChainEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operation: String,
    pub hash: String,
}

impl ForensicReport {
    /// Generate a formal forensic report from analysis results.
    pub fn generate(
        results: ForensicResult,
        examiner: Option<String>,
        case_number: Option<String>,
    ) -> Self {
        let mut conclusions = Vec::new();

        // Derive conclusions from results
        if let Some(ref entropy) = results.entropy_stats {
            if entropy.zero_pct > 99.0 {
                conclusions
                    .push("Device appears to be fully wiped (>99% zero sectors)".to_string());
            } else if entropy.high_entropy_pct > 95.0 {
                conclusions.push(
                    "Device shows high entropy consistent with random overwrite or encryption"
                        .to_string(),
                );
            } else if entropy.low_entropy_pct > 50.0 {
                conclusions.push(
                    "Device contains significant low-entropy data suggesting possible data remnants"
                        .to_string(),
                );
            }
        }

        if !results.signature_hits.is_empty() {
            conclusions.push(format!(
                "{} file signatures detected — data remnants present",
                results.signature_hits.len()
            ));
        }

        if let Some(ref sampling) = results.sampling_result
            && sampling.data_remnant_pct > 1.0
        {
            conclusions.push(format!(
                "Statistical sampling found {:.1}% data remnants",
                sampling.data_remnant_pct
            ));
        }

        if let Some(ref hidden) = results.hidden_areas {
            if !hidden.hidden_partitions.is_empty() {
                conclusions.push(format!(
                    "{} hidden partition(s) detected",
                    hidden.hidden_partitions.len()
                ));
            }
            let gaps_with_data = hidden
                .unallocated_gaps
                .iter()
                .filter(|g| g.has_data)
                .count();
            if gaps_with_data > 0 {
                conclusions.push(format!(
                    "Data remnants found in {} unallocated gap(s) between partitions",
                    gaps_with_data
                ));
            }
            if hidden.hpa_detected {
                conclusions.push(
                    "Host Protected Area (HPA) detected — may contain hidden data".to_string(),
                );
            }
            if hidden.dco_detected {
                conclusions.push("Device Configuration Overlay (DCO) detected — device may be reporting reduced capacity".to_string());
            }
        }

        if conclusions.is_empty() {
            conclusions.push("No significant findings".to_string());
        }

        Self {
            report_version: "1.0".to_string(),
            generated_at: Utc::now(),
            examiner,
            case_number,
            methodology: "DriveWipe automated forensic analysis: entropy calculation, file signature scanning, statistical random sampling".to_string(),
            results,
            conclusions,
            hash_chain: Vec::new(),
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> crate::error::Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            crate::error::DriveWipeError::Forensic(format!("Failed to serialize report: {e}"))
        })
    }
}
