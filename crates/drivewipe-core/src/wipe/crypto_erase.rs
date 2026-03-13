//! TCG Opal crypto erase — destroys the encryption key on a self-encrypting
//! drive (SED), rendering all data unrecoverable.
//!
//! - **Linux:** Uses the kernel's `sed-opal` driver ioctls
//! - **macOS:** Returns `PlatformNotSupported` (no kernel SED support)
//! - **Windows:** Returns `PlatformNotSupported` (deferred; would use
//!   `IOCTL_SCSI_MINIPORT`)

use async_trait::async_trait;
use crossbeam_channel::Sender;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

use super::firmware::FirmwareWipe;

/// TCG Opal crypto erase — destroys the encryption key on a self-encrypting drive (SED).
pub struct TcgOpalCryptoErase;

#[async_trait]
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

    async fn execute(
        &self,
        drive: &DriveInfo,
        session_id: uuid::Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let drive = drive.clone();
        let progress_tx = progress_tx.clone();
        tokio::task::spawn_blocking(move || {
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseStarted {
                session_id,
                method_name: "TCG Opal Crypto Erase".to_string(),
            });

            #[cfg(target_os = "linux")]
            {
                linux_opal::tcg_opal_erase_linux(&drive, session_id, &progress_tx)
            }

            #[cfg(target_os = "macos")]
            {
                let _ = (drive, session_id, progress_tx);
                Err(DriveWipeError::PlatformNotSupported(
                    "TCG Opal crypto erase is not supported on macOS (no kernel SED driver)".into(),
                ))
            }

            #[cfg(target_os = "windows")]
            {
                let _ = (drive, session_id, progress_tx);
                Err(DriveWipeError::PlatformNotSupported(
                    "TCG Opal crypto erase on Windows is not yet implemented \
                     (requires IOCTL_SCSI_MINIPORT with TCG Storage commands)"
                        .into(),
                ))
            }
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?
    }
}

// ── Linux implementation ─────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_opal {
    use super::*;
    use std::os::unix::io::AsRawFd;

    // sed-opal ioctl numbers (from linux/sed-opal.h)
    // These are _IOW('p', N, struct opal_lock_unlock) etc.
    // The exact values depend on struct sizes; we use the standard Linux values.

    // Ioctl command numbers for the sed-opal driver.
    // _IOW('p', 220, struct opal_key)
    #[allow(dead_code)]
    const IOC_OPAL_SAVE: u64 = 0x4050_70DC;
    // _IOW('p', 225, struct opal_key)
    const IOC_OPAL_TAKE_OWNERSHIP: u64 = 0x4050_70E1;
    // _IOW('p', 226, struct opal_lr_act)
    const IOC_OPAL_ACTIVATE_LSP: u64 = 0x4058_70E2;
    // _IOW('p', 229, struct opal_key)
    const IOC_OPAL_REVERT_TPR: u64 = 0x4050_70E5;

    /// Maximum key length for the opal_key structure.
    const OPAL_KEY_MAX: usize = 256;

    /// The MSID (Manufacturing Secure ID) is the default SID password on
    /// factory-fresh drives. It's typically all zeros or a drive-specific
    /// value. We try the common default first.
    const DEFAULT_MSID: &[u8] = b"";

    /// opal_key structure matching the kernel's definition.
    #[repr(C)]
    struct OpalKey {
        lr: u8,
        who: u8,
        __lra: [u8; 6],
        key_len: u32,
        __align: [u8; 4],
        key: [u8; OPAL_KEY_MAX],
    }

    impl OpalKey {
        fn new(password: &[u8]) -> Self {
            let mut key = Self {
                lr: 0,
                who: 0, // OPAL_ADMIN1
                __lra: [0; 6],
                key_len: password.len() as u32,
                __align: [0; 4],
                key: [0; OPAL_KEY_MAX],
            };
            let len = password.len().min(OPAL_KEY_MAX);
            key.key[..len].copy_from_slice(&password[..len]);
            key
        }
    }

    /// opal_lr_act structure for activating the Locking SP.
    #[repr(C)]
    struct OpalLrAct {
        key: OpalKey,
        sum: u32,
        num_lrs: u32,
        lr: [u8; 9],
        _padding: [u8; 3],
    }

    pub fn tcg_opal_erase_linux(
        drive: &DriveInfo,
        session_id: uuid::Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&drive.path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DriveWipeError::InsufficientPrivileges {
                        message: format!("Cannot open {} — run as root", drive.path.display()),
                    }
                } else {
                    DriveWipeError::Io {
                        path: drive.path.clone(),
                        source: e,
                    }
                }
            })?;

        let fd = file.as_raw_fd();

        // Step 1: Take ownership with MSID (default password)
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 10.0,
        });

        let msid_key = OpalKey::new(DEFAULT_MSID);
        let ret = unsafe { libc::ioctl(fd, IOC_OPAL_TAKE_OWNERSHIP as _, &msid_key as *const _) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            // EPERM typically means the drive is already owned with a
            // non-default password.
            if err.raw_os_error() == Some(libc::EPERM) {
                return Err(DriveWipeError::FirmwareError {
                    reason: "Drive is already owned with a non-default SID password. \
                             The current SID password is required to perform a TCG Opal \
                             crypto erase."
                        .into(),
                });
            }
            return Err(DriveWipeError::Ioctl {
                operation: "IOC_OPAL_TAKE_OWNERSHIP".into(),
                source: err,
            });
        }

        // Step 2: Activate the Locking SP
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 30.0,
        });

        let mut lr_act: OpalLrAct = unsafe { std::mem::zeroed() };
        lr_act.key = OpalKey::new(DEFAULT_MSID);
        lr_act.num_lrs = 1;
        lr_act.lr[0] = 0; // Locking range 0 (global)

        let ret = unsafe { libc::ioctl(fd, IOC_OPAL_ACTIVATE_LSP as _, &lr_act as *const _) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            // EALREADY means the Locking SP is already active — that's OK.
            if err.raw_os_error() != Some(libc::EALREADY) {
                return Err(DriveWipeError::Ioctl {
                    operation: "IOC_OPAL_ACTIVATE_LSP".into(),
                    source: err,
                });
            }
        }

        // Step 3: Revert the TPer (destroys encryption key = factory reset)
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 60.0,
        });

        let revert_key = OpalKey::new(DEFAULT_MSID);
        let ret = unsafe { libc::ioctl(fd, IOC_OPAL_REVERT_TPR as _, &revert_key as *const _) };
        if ret < 0 {
            return Err(DriveWipeError::Ioctl {
                operation: "IOC_OPAL_REVERT_TPR".into(),
                source: std::io::Error::last_os_error(),
            });
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }
}
