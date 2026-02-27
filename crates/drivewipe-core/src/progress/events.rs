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
        }
    }
}
