use crossbeam_channel::Sender;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

use super::firmware::FirmwareWipe;

/// TCG Opal crypto erase — destroys the encryption key on a self-encrypting drive (SED).
pub struct TcgOpalCryptoErase;

impl FirmwareWipe for TcgOpalCryptoErase {
    fn id(&self) -> &str {
        "tcg-opal"
    }

    fn name(&self) -> &str {
        "TCG Opal Crypto Erase"
    }

    fn description(&self) -> &str {
        "Destroys the encryption key on a self-encrypting drive (SED), rendering all data \
         unrecoverable. Near-instant operation. Requires TCG Opal support."
    }

    fn is_supported(&self, drive: &DriveInfo) -> bool {
        drive.is_sed
    }

    fn execute(
        &self,
        _drive: &DriveInfo,
        _session_id: uuid::Uuid,
        _progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        // TODO: Implement TCG Opal crypto erase
        // On Linux: use the sed-opal kernel driver via ioctl
        // On Windows: use TCG Storage commands via IOCTL_SCSI_MINIPORT
        // On macOS: very limited support, may need external tools
        //
        // Steps:
        // 1. Verify drive supports TCG Opal (already checked in is_supported)
        // 2. Take ownership of the Locking SP
        // 3. Issue a RevertSP command to destroy the encryption key
        // 4. The drive generates a new random key, making old data unreadable
        Err(DriveWipeError::PlatformNotSupported(
            "TCG Opal crypto erase not yet implemented — requires platform-specific SED commands"
                .to_string(),
        ))
    }
}
