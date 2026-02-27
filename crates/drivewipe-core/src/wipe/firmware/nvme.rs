//! NVMe Format and Sanitize firmware commands.
//!
//! These are stubs that return [`DriveWipeError::PlatformNotSupported`] until
//! the platform-specific ioctl/admin-command layer is implemented.

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::{DriveInfo, Transport};

use super::FirmwareWipe;

// ── NVMe Format — User Data Erase (SES=1) ───────────────────────────────────

/// NVMe Format NVM command with Secure Erase Setting = User Data Erase.
///
/// Performs a low-level format that overwrites all user data on the namespace.
/// The controller determines the mechanism (pattern overwrite, reset, etc.).
pub struct NvmeFormatUserData;

impl FirmwareWipe for NvmeFormatUserData {
    fn id(&self) -> &str {
        "nvme-format-user"
    }

    fn name(&self) -> &str {
        "NVMe Format (User Data Erase)"
    }

    fn description(&self) -> &str {
        "NVMe Format NVM with SES=1 — user data erase"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Nvme
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "NVMe Format (User Data Erase) is not yet implemented on this platform".into(),
        });
        // TODO: implement NVMe Admin Command — Format NVM (SES=1)
        Err(DriveWipeError::PlatformNotSupported(
            "NVMe Format NVM (SES=1) ioctl not yet implemented".into(),
        ))
    }
}

// ── NVMe Format — Cryptographic Erase (SES=2) ───────────────────────────────

/// NVMe Format NVM command with Secure Erase Setting = Cryptographic Erase.
///
/// Rotates the internal media encryption key, rendering all previously written
/// data unrecoverable. This is the fastest and most thorough NVMe erase on
/// drives that support it.
pub struct NvmeFormatCrypto;

impl FirmwareWipe for NvmeFormatCrypto {
    fn id(&self) -> &str {
        "nvme-format-crypto"
    }

    fn name(&self) -> &str {
        "NVMe Format (Cryptographic Erase)"
    }

    fn description(&self) -> &str {
        "NVMe Format NVM with SES=2 — cryptographic erase (key rotation)"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Nvme
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "NVMe Format (Cryptographic Erase) is not yet implemented on this platform"
                .into(),
        });
        // TODO: implement NVMe Admin Command — Format NVM (SES=2)
        Err(DriveWipeError::PlatformNotSupported(
            "NVMe Format NVM (SES=2) ioctl not yet implemented".into(),
        ))
    }
}

// ── NVMe Sanitize — Block Erase ─────────────────────────────────────────────

/// NVMe Sanitize command — Block Erase action.
///
/// Instructs the controller to perform a low-level block erase of all user
/// data, including data in deallocated or unwritten logical blocks.
pub struct NvmeSanitizeBlock;

impl FirmwareWipe for NvmeSanitizeBlock {
    fn id(&self) -> &str {
        "nvme-sanitize-block"
    }

    fn name(&self) -> &str {
        "NVMe Sanitize (Block Erase)"
    }

    fn description(&self) -> &str {
        "NVMe Sanitize — block erase of all user data"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Nvme
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "NVMe Sanitize (Block Erase) is not yet implemented on this platform".into(),
        });
        // TODO: implement NVMe Admin Command — Sanitize (Block Erase)
        Err(DriveWipeError::PlatformNotSupported(
            "NVMe Sanitize (Block Erase) ioctl not yet implemented".into(),
        ))
    }
}

// ── NVMe Sanitize — Crypto Erase ────────────────────────────────────────────

/// NVMe Sanitize command — Crypto Erase action.
///
/// Rotates the media encryption key for all namespaces, rendering all data
/// cryptographically unrecoverable.
pub struct NvmeSanitizeCrypto;

impl FirmwareWipe for NvmeSanitizeCrypto {
    fn id(&self) -> &str {
        "nvme-sanitize-crypto"
    }

    fn name(&self) -> &str {
        "NVMe Sanitize (Crypto Erase)"
    }

    fn description(&self) -> &str {
        "NVMe Sanitize — cryptographic erase (key rotation)"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Nvme
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "NVMe Sanitize (Crypto Erase) is not yet implemented on this platform".into(),
        });
        // TODO: implement NVMe Admin Command — Sanitize (Crypto Erase)
        Err(DriveWipeError::PlatformNotSupported(
            "NVMe Sanitize (Crypto Erase) ioctl not yet implemented".into(),
        ))
    }
}

// ── NVMe Sanitize — Overwrite ────────────────────────────────────────────────

/// NVMe Sanitize command — Overwrite action.
///
/// Instructs the controller to perform a multi-pass overwrite of all user
/// data using a controller-defined pattern. This is the slowest NVMe sanitize
/// option but is the closest equivalent to traditional software overwrite.
pub struct NvmeSanitizeOverwrite;

impl FirmwareWipe for NvmeSanitizeOverwrite {
    fn id(&self) -> &str {
        "nvme-sanitize-overwrite"
    }

    fn name(&self) -> &str {
        "NVMe Sanitize (Overwrite)"
    }

    fn description(&self) -> &str {
        "NVMe Sanitize — controller-managed overwrite of all user data"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Nvme
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "NVMe Sanitize (Overwrite) is not yet implemented on this platform".into(),
        });
        // TODO: implement NVMe Admin Command — Sanitize (Overwrite)
        Err(DriveWipeError::PlatformNotSupported(
            "NVMe Sanitize (Overwrite) ioctl not yet implemented".into(),
        ))
    }
}
