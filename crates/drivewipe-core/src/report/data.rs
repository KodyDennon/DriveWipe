use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::WipeResult;

/// Extended report data wrapping WipeResult with additional metadata
/// suitable for archival and certificate generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeReport {
    pub version: String,
    pub generated_at: DateTime<Utc>,
    pub result: WipeResult,
}

impl WipeReport {
    pub fn from_result(result: WipeResult) -> Self {
        Self {
            version: "1.0".to_string(),
            generated_at: Utc::now(),
            result,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(json)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let report: Self = serde_json::from_str(json)?;
        Ok(report)
    }
}
