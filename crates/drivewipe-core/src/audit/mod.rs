pub mod types;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use self::types::{AuditCategory, AuditSeverity};
use crate::error::Result;

type HmacSha256 = Hmac<Sha256>;

/// A single audit log entry with full context.
///
/// Each entry includes an HMAC-SHA256 integrity tag computed over the JSON
/// content chained with the previous entry's HMAC (forming a hash chain).
/// This allows detection of log tampering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
    pub event: AuditEvent,
    pub operator: Option<String>,
    pub device_path: Option<String>,
    pub device_serial: Option<String>,
    pub session_id: Option<uuid::Uuid>,
    pub details: Option<String>,
    /// HMAC-SHA256 integrity tag: `HMAC(key, previous_hmac || json_content)`.
    /// The first entry in a log file uses an all-zero previous HMAC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity_hmac: Option<String>,
    /// Chain hash from the previous entry (hex-encoded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_hash: Option<String>,
}

/// Categorized audit events for all DriveWipe operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    // Wipe
    WipeStarted {
        method: String,
        device: String,
    },
    WipeCompleted {
        outcome: String,
        duration_secs: f64,
    },
    WipeCancelled,
    WipeResumed {
        session_id: uuid::Uuid,
    },

    // Clone
    CloneStarted {
        source: String,
        target: String,
    },
    CloneCompleted {
        duration_secs: f64,
        verified: bool,
    },

    // Partition
    PartitionCreated {
        device: String,
        partition_type: String,
    },
    PartitionDeleted {
        device: String,
        partition_index: u32,
    },
    PartitionResized {
        device: String,
        partition_index: u32,
    },

    // Forensic
    ForensicScanStarted {
        scan_type: String,
    },
    ForensicScanCompleted {
        findings: u32,
    },

    // Health
    HealthCheckPerformed {
        healthy: bool,
    },
    HealthSnapshotSaved {
        path: String,
    },

    // Config
    ConfigLoaded {
        path: String,
    },
    ConfigChanged {
        key: String,
    },

    // Keyboard Lock
    KeyboardLocked,
    KeyboardUnlocked,

    // System
    ApplicationStarted,
    ApplicationStopped,
    PrivilegeElevated,
}

/// Writes audit entries as JSONL to a directory.
pub struct AuditLogger {
    audit_dir: PathBuf,
    operator: Option<String>,
    /// HMAC key for integrity tags. Generated randomly per logger instance
    /// and stored alongside the audit log for verification.
    hmac_key: [u8; 32],
    /// The HMAC of the last written entry, forming a chain.
    last_hmac: String,
}

impl AuditLogger {
    pub fn new(audit_dir: PathBuf, operator: Option<String>) -> Self {
        use rand::RngExt;
        let mut rng = rand::rng();
        let hmac_key: [u8; 32] = rng.random();

        // Try to persist the HMAC key alongside the audit logs
        if let Err(e) = fs::create_dir_all(&audit_dir) {
            log::warn!("Failed to create audit dir for HMAC key: {e}");
        }
        let key_path = audit_dir.join(".audit_hmac_key");
        if !key_path.exists() {
            if let Err(e) = fs::write(&key_path, hex::encode(hmac_key)) {
                log::warn!("Failed to write HMAC key: {e}");
            }
        }

        Self {
            audit_dir,
            operator,
            hmac_key,
            last_hmac: "0".repeat(64), // All-zero initial chain value
        }
    }

    /// Log an audit event, writing it as a JSONL line to today's log file.
    pub fn log(
        &mut self,
        event: AuditEvent,
        device_path: Option<&str>,
        device_serial: Option<&str>,
        session_id: Option<uuid::Uuid>,
    ) -> Result<()> {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            category: event.category(),
            severity: event.severity(),
            event,
            operator: self.operator.clone(),
            device_path: device_path.map(String::from),
            device_serial: device_serial.map(String::from),
            session_id,
            details: None,
            integrity_hmac: None,
            chain_hash: None,
        };

        self.write_entry(&entry)
    }

    /// Log an audit event with additional detail text.
    pub fn log_with_details(
        &mut self,
        event: AuditEvent,
        details: &str,
        device_path: Option<&str>,
        session_id: Option<uuid::Uuid>,
    ) -> Result<()> {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            category: event.category(),
            severity: event.severity(),
            event,
            operator: self.operator.clone(),
            device_path: device_path.map(String::from),
            device_serial: None,
            session_id,
            details: Some(details.to_string()),
            integrity_hmac: None,
            chain_hash: None,
        };

        self.write_entry(&entry)
    }

    fn write_entry(&mut self, entry: &AuditEntry) -> Result<()> {
        fs::create_dir_all(&self.audit_dir).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!(
                "Failed to create audit directory {}: {}",
                self.audit_dir.display(),
                e
            ))
        })?;

        // Compute HMAC-SHA256 chain: HMAC(key, last_hmac || json_content)
        let mut signed_entry = entry.clone();
        signed_entry.chain_hash = Some(self.last_hmac.clone());

        // Serialize without HMAC first to compute the tag
        signed_entry.integrity_hmac = None;
        let content = serde_json::to_string(&signed_entry).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!("Failed to serialize audit entry: {e}"))
        })?;

        let mut mac =
            HmacSha256::new_from_slice(&self.hmac_key).expect("HMAC key should be 32 bytes");
        mac.update(self.last_hmac.as_bytes());
        mac.update(content.as_bytes());
        let hmac_hex = hex::encode(mac.finalize().into_bytes());

        // Attach the HMAC and update the chain
        signed_entry.integrity_hmac = Some(hmac_hex.clone());
        self.last_hmac = hmac_hex;

        let date = entry.timestamp.format("%Y-%m-%d");
        let log_path = self.audit_dir.join(format!("audit-{date}.jsonl"));

        let mut line = serde_json::to_string(&signed_entry).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!("Failed to serialize audit entry: {e}"))
        })?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| {
                crate::error::DriveWipeError::Audit(format!(
                    "Failed to open audit log {}: {}",
                    log_path.display(),
                    e
                ))
            })?;

        file.write_all(line.as_bytes()).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!("Failed to write audit entry: {e}"))
        })?;

        Ok(())
    }

    /// Read all audit entries from a specific date's log file.
    pub fn read_entries(audit_dir: &Path, date: &str) -> Result<Vec<AuditEntry>> {
        let log_path = audit_dir.join(format!("audit-{date}.jsonl"));
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&log_path).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!(
                "Failed to read audit log {}: {}",
                log_path.display(),
                e
            ))
        })?;

        let mut entries = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: AuditEntry = serde_json::from_str(line).map_err(|e| {
                crate::error::DriveWipeError::Audit(format!("Failed to parse audit entry: {e}"))
            })?;
            entries.push(entry);
        }

        Ok(entries)
    }
}

impl AuditEvent {
    pub fn category(&self) -> AuditCategory {
        match self {
            AuditEvent::WipeStarted { .. }
            | AuditEvent::WipeCompleted { .. }
            | AuditEvent::WipeCancelled
            | AuditEvent::WipeResumed { .. } => AuditCategory::Wipe,

            AuditEvent::CloneStarted { .. } | AuditEvent::CloneCompleted { .. } => {
                AuditCategory::Clone
            }

            AuditEvent::PartitionCreated { .. }
            | AuditEvent::PartitionDeleted { .. }
            | AuditEvent::PartitionResized { .. } => AuditCategory::Partition,

            AuditEvent::ForensicScanStarted { .. } | AuditEvent::ForensicScanCompleted { .. } => {
                AuditCategory::Forensic
            }

            AuditEvent::HealthCheckPerformed { .. } | AuditEvent::HealthSnapshotSaved { .. } => {
                AuditCategory::Health
            }

            AuditEvent::ConfigLoaded { .. } | AuditEvent::ConfigChanged { .. } => {
                AuditCategory::Config
            }

            AuditEvent::KeyboardLocked | AuditEvent::KeyboardUnlocked => {
                AuditCategory::KeyboardLock
            }

            AuditEvent::ApplicationStarted
            | AuditEvent::ApplicationStopped
            | AuditEvent::PrivilegeElevated => AuditCategory::Config,
        }
    }

    pub fn severity(&self) -> AuditSeverity {
        match self {
            AuditEvent::WipeStarted { .. }
            | AuditEvent::CloneStarted { .. }
            | AuditEvent::ForensicScanStarted { .. } => AuditSeverity::Info,

            AuditEvent::WipeCompleted { .. }
            | AuditEvent::CloneCompleted { .. }
            | AuditEvent::ForensicScanCompleted { .. }
            | AuditEvent::HealthCheckPerformed { .. } => AuditSeverity::Info,

            AuditEvent::PartitionCreated { .. }
            | AuditEvent::PartitionDeleted { .. }
            | AuditEvent::PartitionResized { .. } => AuditSeverity::Warning,

            AuditEvent::WipeCancelled => AuditSeverity::Warning,

            _ => AuditSeverity::Info,
        }
    }
}
