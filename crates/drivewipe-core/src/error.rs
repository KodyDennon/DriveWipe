use std::path::PathBuf;

use thiserror::Error;

/// Top-level error type for all DriveWipe operations.
#[derive(Debug, Error)]
pub enum DriveWipeError {
    // ── I/O ──────────────────────────────────────────────────────────
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    IoGeneric(#[from] std::io::Error),

    // ── Device ───────────────────────────────────────────────────────
    #[error("Device not found: {0}")]
    DeviceNotFound(PathBuf),

    #[error("Cannot wipe boot drive: {0}")]
    BootDriveRefused(PathBuf),

    #[error("Insufficient privileges: {message}")]
    InsufficientPrivileges { message: String },

    #[error("Device error: {0}")]
    DeviceError(String),

    #[error("Device busy: {0}")]
    DeviceBusy(PathBuf),

    #[error("Device is read-only: {0}")]
    DeviceReadOnly(PathBuf),

    // ── Wipe ─────────────────────────────────────────────────────────
    #[error("Unknown wipe method: {0}")]
    UnknownMethod(String),

    #[error("Wipe cancelled by user")]
    Cancelled,

    #[error("Wipe interrupted: {reason}")]
    Interrupted { reason: String },

    #[error(
        "Verification failed at offset {offset:#x}: expected {expected:#04x}, got {actual:#04x}"
    )]
    VerificationFailed {
        offset: u64,
        expected: u8,
        actual: u8,
    },

    #[error("Verification failed: {message}")]
    VerificationError { message: String },

    // ── Firmware ──────────────────────────────────────────────────────
    #[error("Firmware erase not supported: {reason}")]
    FirmwareNotSupported { reason: String },

    #[error("Firmware erase failed: {reason}")]
    FirmwareError { reason: String },

    #[error("ATA security is frozen — suspend/resume the system to unfreeze")]
    AtaSecurityFrozen,

    #[error("ATA security is locked with an unknown password")]
    AtaSecurityLocked,

    // ── Config ───────────────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to parse config file {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    // ── Resume ───────────────────────────────────────────────────────
    #[error("No resumable session found for device {serial}")]
    NoResumableSession { serial: String },

    #[error("Session state corrupted: {path}")]
    SessionCorrupted { path: PathBuf },

    // ── Report ───────────────────────────────────────────────────────
    #[error("Report generation failed: {0}")]
    ReportError(String),

    // ── Platform ─────────────────────────────────────────────────────
    #[error("Platform not supported for this operation: {0}")]
    PlatformNotSupported(String),

    #[error("ioctl failed: {operation}: {source}")]
    Ioctl {
        operation: String,
        source: std::io::Error,
    },

    // ── Serialization ────────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DriveWipeError>;
