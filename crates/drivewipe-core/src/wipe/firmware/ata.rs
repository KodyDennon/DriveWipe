//! ATA Secure Erase firmware commands.
//!
//! These are stubs that return [`DriveWipeError::PlatformNotSupported`] until
//! the platform-specific ioctl layer is implemented.

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::{AtaSecurityState, DriveInfo, Transport};

use super::FirmwareWipe;

// ── ATA Secure Erase (Normal) ────────────────────────────────────────────────

/// ATA SECURITY ERASE UNIT -- normal mode.
///
/// Issues the standard ATA Secure Erase command which overwrites all user-
/// accessible sectors. Requires the drive's ATA security feature set to be
/// in the Disabled or Enabled state (not Frozen or NotSupported).
pub struct AtaSecureErase;

impl FirmwareWipe for AtaSecureErase {
    fn id(&self) -> &str {
        "ata-erase"
    }

    fn name(&self) -> &str {
        "ATA Secure Erase"
    }

    fn description(&self) -> &str {
        "ATA SECURITY ERASE UNIT (normal) — drive-controller overwrite of all sectors"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Sata
            && !matches!(
                drive.ata_security,
                AtaSecurityState::Frozen | AtaSecurityState::NotSupported
            )
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "ATA Secure Erase is not yet implemented on this platform".into(),
        });
        // TODO: implement platform-specific ATA SECURITY ERASE UNIT ioctl
        Err(DriveWipeError::PlatformNotSupported(
            "ATA Secure Erase ioctl not yet implemented".into(),
        ))
    }
}

// ── ATA Enhanced Secure Erase ────────────────────────────────────────────────

/// ATA SECURITY ERASE UNIT -- enhanced mode.
///
/// The enhanced variant may additionally erase reallocated sectors and vendor-
/// specific areas. On self-encrypting drives (SEDs) it typically performs a
/// cryptographic erase by rotating the internal media encryption key.
pub struct AtaEnhancedSecureErase;

impl FirmwareWipe for AtaEnhancedSecureErase {
    fn id(&self) -> &str {
        "ata-erase-enhanced"
    }

    fn name(&self) -> &str {
        "ATA Enhanced Secure Erase"
    }

    fn description(&self) -> &str {
        "ATA SECURITY ERASE UNIT (enhanced) — includes reallocated sectors and vendor areas"
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.transport == Transport::Sata
            && !matches!(
                drive.ata_security,
                AtaSecurityState::Frozen | AtaSecurityState::NotSupported
            )
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let _ = progress_tx.send(ProgressEvent::Warning {
            session_id,
            message: "ATA Enhanced Secure Erase is not yet implemented on this platform".into(),
        });
        // TODO: implement platform-specific ATA SECURITY ERASE UNIT (enhanced) ioctl
        Err(DriveWipeError::PlatformNotSupported(
            "ATA Enhanced Secure Erase ioctl not yet implemented".into(),
        ))
    }
}
