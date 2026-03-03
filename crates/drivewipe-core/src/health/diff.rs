use serde::{Deserialize, Serialize};

use super::snapshot::DriveHealthSnapshot;

/// Result of comparing two health snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdict {
    /// No degradation detected.
    Pass,
    /// Minor degradation detected but within tolerance.
    Warning,
    /// Significant degradation detected.
    Fail,
}

/// Detailed comparison between two health snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthComparison {
    pub verdict: HealthVerdict,
    pub temperature_change: Option<i16>,
    pub reallocated_sectors_change: Option<i64>,
    pub pending_sectors_change: Option<i64>,
    pub media_errors_change: Option<i128>,
    pub messages: Vec<String>,
}

/// Compares two health snapshots to detect degradation.
pub struct HealthDiff;

impl HealthDiff {
    pub fn compare(before: &DriveHealthSnapshot, after: &DriveHealthSnapshot) -> HealthComparison {
        let mut verdict = HealthVerdict::Pass;
        let mut messages = Vec::new();

        let temperature_change = match (before.temperature_celsius, after.temperature_celsius) {
            (Some(b), Some(a)) => {
                let change = a - b;
                if change > 15 {
                    messages.push(format!("Temperature increased by {change}°C"));
                    verdict = HealthVerdict::Warning;
                }
                Some(change)
            }
            _ => None,
        };

        let reallocated_sectors_change = match (&before.smart_data, &after.smart_data) {
            (Some(b), Some(a)) => match (b.reallocated_sectors, a.reallocated_sectors) {
                (Some(bv), Some(av)) => {
                    let change = av as i64 - bv as i64;
                    if change > 0 {
                        messages.push(format!("Reallocated sectors increased by {change}"));
                        verdict = HealthVerdict::Fail;
                    }
                    Some(change)
                }
                _ => None,
            },
            _ => None,
        };

        let pending_sectors_change = match (&before.smart_data, &after.smart_data) {
            (Some(b), Some(a)) => match (b.pending_sectors, a.pending_sectors) {
                (Some(bv), Some(av)) => {
                    let change = av as i64 - bv as i64;
                    if change > 0 {
                        messages.push(format!("Pending sectors increased by {change}"));
                        if verdict != HealthVerdict::Fail {
                            verdict = HealthVerdict::Warning;
                        }
                    }
                    Some(change)
                }
                _ => None,
            },
            _ => None,
        };

        let media_errors_change = match (&before.nvme_health, &after.nvme_health) {
            (Some(b), Some(a)) => {
                let change = a.media_errors as i128 - b.media_errors as i128;
                if change > 0 {
                    messages.push(format!("Media errors increased by {change}"));
                    verdict = HealthVerdict::Fail;
                }
                Some(change)
            }
            _ => None,
        };

        if !after.is_healthy() && before.is_healthy() {
            messages.push("Drive health status changed from healthy to unhealthy".to_string());
            verdict = HealthVerdict::Fail;
        }

        if messages.is_empty() {
            messages.push("No degradation detected".to_string());
        }

        HealthComparison {
            verdict,
            temperature_change,
            reallocated_sectors_change,
            pending_sectors_change,
            media_errors_change,
            messages,
        }
    }
}
