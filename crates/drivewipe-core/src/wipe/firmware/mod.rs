//! Firmware-level drive erase commands.
//!
//! This module defines the [`FirmwareWipe`] trait for operations that delegate
//! erasure to the drive's own controller (ATA Secure Erase, NVMe Format,
//! NVMe Sanitize). Platform-specific ioctl implementations live in the
//! sub-modules.

pub mod ata;
pub mod nvme;

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::Result;
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

/// A firmware-level erase command executed by the drive controller.
///
/// Unlike software overwrite methods, firmware wipes cannot be observed at the
/// byte level from the host -- the drive's controller is responsible for the
/// actual data destruction.
pub trait FirmwareWipe: Send + Sync {
    /// Machine-readable identifier (e.g. `"ata-erase"`).
    fn id(&self) -> &str;

    /// Human-readable name shown in the UI and reports.
    fn name(&self) -> &str;

    /// Longer description of what this firmware command does.
    fn description(&self) -> &str;

    /// Returns `true` if this erase command is likely to succeed on the given
    /// drive, based on transport type and security state.
    fn is_supported(&self, drive: &DriveInfo) -> bool;

    /// Issue the firmware erase command.
    ///
    /// Progress updates are sent through `progress_tx`. The implementation
    /// should block until the drive signals completion or an error occurs.
    fn execute(
        &self,
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()>;
}
