pub mod types;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use self::types::{AuditCategory, AuditSeverity};
use crate::error::Result;

/// A single audit log entry with full context.
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
}

impl AuditLogger {
    pub fn new(audit_dir: PathBuf, operator: Option<String>) -> Self {
        Self {
            audit_dir,
            operator,
        }
    }

    /// Log an audit event, writing it as a JSONL line to today's log file.
    pub fn log(
        &self,
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
        };

        self.write_entry(&entry)
    }

    /// Log an audit event with additional detail text.
    pub fn log_with_details(
        &self,
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
        };

        self.write_entry(&entry)
    }

    fn write_entry(&self, entry: &AuditEntry) -> Result<()> {
        fs::create_dir_all(&self.audit_dir).map_err(|e| {
            crate::error::DriveWipeError::Audit(format!(
                "Failed to create audit directory {}: {}",
                self.audit_dir.display(),
                e
            ))
        })?;

        let date = entry.timestamp.format("%Y-%m-%d");
        let log_path = self.audit_dir.join(format!("audit-{date}.jsonl"));

        let mut line = serde_json::to_string(entry).map_err(|e| {
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
