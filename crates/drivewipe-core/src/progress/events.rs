use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressEvent {
    SessionStarted {
        session_id: Uuid,
        device_path: String,
        device_serial: String,
        method_id: String,
        method_name: String,
        total_bytes: u64,
        total_passes: u32,
    },
    PassStarted {
        session_id: Uuid,
        pass_number: u32,
        pass_name: String,
    },
    BlockWritten {
        session_id: Uuid,
        pass_number: u32,
        bytes_written: u64,
        total_bytes: u64,
        throughput_bps: f64,
    },
    PassCompleted {
        session_id: Uuid,
        pass_number: u32,
        duration_secs: f64,
        throughput_mbps: f64,
    },
    VerificationStarted {
        session_id: Uuid,
    },
    VerificationProgress {
        session_id: Uuid,
        bytes_verified: u64,
        total_bytes: u64,
    },
    VerificationCompleted {
        session_id: Uuid,
        passed: bool,
        duration_secs: f64,
    },
    FirmwareEraseStarted {
        session_id: Uuid,
        method_name: String,
    },
    FirmwareEraseProgress {
        session_id: Uuid,
        percent: f32,
    },
    FirmwareEraseCompleted {
        session_id: Uuid,
        duration_secs: f64,
    },
    Warning {
        session_id: Uuid,
        message: String,
    },
    Error {
        session_id: Uuid,
        message: String,
    },
    Interrupted {
        session_id: Uuid,
        reason: String,
        bytes_written: u64,
    },
    Completed {
        session_id: Uuid,
        outcome: crate::types::WipeOutcome,
        duration_secs: f64,
    },

    // ── Health events ────────────────────────────────────────────────
    HealthCheckStarted {
        session_id: Uuid,
        device_path: String,
    },
    HealthCheckCompleted {
        session_id: Uuid,
        healthy: bool,
        message: String,
    },
    HealthSnapshotSaved {
        session_id: Uuid,
        path: String,
    },

    // ── Clone events ─────────────────────────────────────────────────
    CloneStarted {
        session_id: Uuid,
        source: String,
        target: String,
        total_bytes: u64,
    },
    CloneProgress {
        session_id: Uuid,
        bytes_copied: u64,
        total_bytes: u64,
        throughput_bps: f64,
    },
    CloneCompleted {
        session_id: Uuid,
        duration_secs: f64,
        verified: bool,
    },

    // ── Partition events ─────────────────────────────────────────────
    PartitionOperationStarted {
        session_id: Uuid,
        operation: String,
        device_path: String,
    },
    PartitionOperationCompleted {
        session_id: Uuid,
        operation: String,
        success: bool,
    },

    // ── Forensic events ──────────────────────────────────────────────
    ForensicScanStarted {
        session_id: Uuid,
        device_path: String,
        scan_type: String,
    },
    ForensicScanProgress {
        session_id: Uuid,
        bytes_scanned: u64,
        total_bytes: u64,
        findings: u32,
    },
    ForensicScanCompleted {
        session_id: Uuid,
        duration_secs: f64,
        total_findings: u32,
    },
}

impl ProgressEvent {
    pub fn session_id(&self) -> Uuid {
        match self {
            ProgressEvent::SessionStarted { session_id, .. } => *session_id,
            ProgressEvent::PassStarted { session_id, .. } => *session_id,
            ProgressEvent::BlockWritten { session_id, .. } => *session_id,
            ProgressEvent::PassCompleted { session_id, .. } => *session_id,
            ProgressEvent::VerificationStarted { session_id } => *session_id,
            ProgressEvent::VerificationProgress { session_id, .. } => *session_id,
            ProgressEvent::VerificationCompleted { session_id, .. } => *session_id,
            ProgressEvent::FirmwareEraseStarted { session_id, .. } => *session_id,
            ProgressEvent::FirmwareEraseProgress { session_id, .. } => *session_id,
            ProgressEvent::FirmwareEraseCompleted { session_id, .. } => *session_id,
            ProgressEvent::Warning { session_id, .. } => *session_id,
            ProgressEvent::Error { session_id, .. } => *session_id,
            ProgressEvent::Interrupted { session_id, .. } => *session_id,
            ProgressEvent::Completed { session_id, .. } => *session_id,
            ProgressEvent::HealthCheckStarted { session_id, .. } => *session_id,
            ProgressEvent::HealthCheckCompleted { session_id, .. } => *session_id,
            ProgressEvent::HealthSnapshotSaved { session_id, .. } => *session_id,
            ProgressEvent::CloneStarted { session_id, .. } => *session_id,
            ProgressEvent::CloneProgress { session_id, .. } => *session_id,
            ProgressEvent::CloneCompleted { session_id, .. } => *session_id,
            ProgressEvent::PartitionOperationStarted { session_id, .. } => *session_id,
            ProgressEvent::PartitionOperationCompleted { session_id, .. } => *session_id,
            ProgressEvent::ForensicScanStarted { session_id, .. } => *session_id,
            ProgressEvent::ForensicScanProgress { session_id, .. } => *session_id,
            ProgressEvent::ForensicScanCompleted { session_id, .. } => *session_id,
        }
    }
}
