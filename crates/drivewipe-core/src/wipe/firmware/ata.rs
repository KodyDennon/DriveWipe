//! ATA Secure Erase firmware commands.
//!
//! Issues ATA SECURITY ERASE UNIT via platform-specific passthrough:
//! - **Linux:** SCSI ATA_16 CDB via `SG_IO` ioctl
//! - **Windows:** `IOCTL_ATA_PASS_THROUGH` with `ATA_PASS_THROUGH_EX`
//! - **macOS:** Returns `PlatformNotSupported` (no reliable ATA passthrough)

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::{AtaSecurityState, DriveInfo, Transport};

use super::FirmwareWipe;

// ── ATA constants (only used in platform-specific code) ─────────────────────

// These constants are only used inside #[cfg(target_os = "linux")] and
// #[cfg(target_os = "windows")] blocks. We suppress dead_code warnings
// rather than duplicating them inside each platform module.
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod ata_consts {
    /// ATA command: SECURITY SET PASSWORD
    pub const ATA_CMD_SEC_SET_PASS: u8 = 0xF1;
    /// ATA command: SECURITY ERASE UNIT
    pub const ATA_CMD_SEC_ERASE_UNIT: u8 = 0xF4;
    /// ATA command: SECURITY DISABLE PASSWORD
    pub const ATA_CMD_SEC_DISABLE_PASS: u8 = 0xF6;

    /// The temporary password used during ATA Secure Erase.
    pub const ATA_TEMP_PASSWORD: &[u8; 16] = b"DriveWipeTmpPwd\0";

    /// ATA password block size (always 512 bytes per ACS spec).
    pub const ATA_PASSWORD_BLOCK_SIZE: usize = 512;

    /// 12-hour timeout for the erase command (in milliseconds for SG_IO).
    pub const ATA_ERASE_TIMEOUT_MS: u32 = 12 * 60 * 60 * 1000;
}

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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        ata_secure_erase_impl(drive, session_id, progress_tx, false)
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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        ata_secure_erase_impl(drive, session_id, progress_tx, true)
    }
}

// ── Platform dispatch ────────────────────────────────────────────────────────

fn ata_secure_erase_impl(
    drive: &DriveInfo,
    session_id: Uuid,
    progress_tx: &Sender<ProgressEvent>,
    enhanced: bool,
) -> Result<()> {
    let label = if enhanced { "Enhanced " } else { "" };

    // Pre-flight checks
    if drive.ata_security == AtaSecurityState::Frozen {
        return Err(DriveWipeError::AtaSecurityFrozen);
    }
    if drive.ata_security == AtaSecurityState::Locked {
        return Err(DriveWipeError::AtaSecurityLocked);
    }

    let _ = progress_tx.send(ProgressEvent::FirmwareEraseStarted {
        session_id,
        method_name: format!("ATA {}Secure Erase", label),
    });

    #[cfg(target_os = "linux")]
    {
        linux_ata::ata_secure_erase_linux(drive, session_id, progress_tx, enhanced)
    }

    #[cfg(target_os = "windows")]
    {
        windows_ata::ata_secure_erase_windows(drive, session_id, progress_tx, enhanced)
    }

    #[cfg(target_os = "macos")]
    {
        let _ = (drive, session_id, progress_tx, enhanced);
        Err(DriveWipeError::PlatformNotSupported(
            "ATA Secure Erase is not supported on macOS (no reliable ATA passthrough)".into(),
        ))
    }
}

// ── Linux implementation ─────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_ata {
    use super::ata_consts::*;
    use super::*;
    use std::os::unix::io::RawFd;

    /// SG_IO ioctl number
    const SG_IO: u32 = 0x2285;

    /// SCSI ATA_16 opcode (SAT)
    const ATA_16: u8 = 0x85;

    /// ATA protocol values for ATA_16 CDB byte 1
    #[allow(dead_code)]
    const ATA_PROTO_NON_DATA: u8 = 3 << 1;
    const ATA_PROTO_PIO_DATA_OUT: u8 = 5 << 1;

    /// Direction constants for sg_io_hdr
    const SG_DXFER_NONE: i32 = -1;
    const SG_DXFER_TO_DEV: i32 = 1;

    /// SG_IO header structure (simplified for our needs).
    #[repr(C)]
    struct SgIoHdr {
        interface_id: i32,
        dxfer_direction: i32,
        cmd_len: u8,
        mx_sb_len: u8,
        iovec_count: u16,
        dxfer_len: u32,
        dxferp: *mut u8,
        cmdp: *const u8,
        sbp: *mut u8,
        timeout: u32,
        flags: u32,
        pack_id: i32,
        usr_ptr: *mut u8,
        status: u8,
        masked_status: u8,
        msg_status: u8,
        sb_len_wr: u8,
        host_status: u16,
        driver_status: u16,
        resid: i32,
        duration: u32,
        info: u32,
    }

    impl Default for SgIoHdr {
        fn default() -> Self {
            unsafe { std::mem::zeroed() }
        }
    }

    nix::ioctl_readwrite!(sg_io_ioctl, 0x22, 0x85, SgIoHdr);

    fn build_password_block(enhanced: bool) -> [u8; ATA_PASSWORD_BLOCK_SIZE] {
        let mut block = [0u8; ATA_PASSWORD_BLOCK_SIZE];
        // Byte 0: control word — bit 0 = identifier (0=user, 1=master),
        // bit 1 = enhanced erase mode
        if enhanced {
            block[0] = 0x02; // Enhanced bit set
        }
        // Bytes 2..34: password (up to 32 bytes, null-padded)
        let pwd_len = ATA_TEMP_PASSWORD.len().min(32);
        block[2..2 + pwd_len].copy_from_slice(&ATA_TEMP_PASSWORD[..pwd_len]);
        block
    }

    fn sg_io_ata16(
        fd: RawFd,
        command: u8,
        protocol: u8,
        data: Option<&mut [u8]>,
        timeout_ms: u32,
    ) -> Result<()> {
        let mut sense_buf = [0u8; 32];

        // Build ATA_16 CDB (16 bytes)
        let mut cdb = [0u8; 16];
        cdb[0] = ATA_16;
        cdb[1] = protocol; // Protocol
        // cdb[2]: t_length, t_dir, byte_block, ck_cond
        if data.is_some() {
            cdb[2] = 0x06; // t_length=SECTOR_COUNT, t_dir=TO_DEV, byte_block=BLOCKS
        }
        cdb[14] = command; // ATA command register

        let (direction, dxfer_len, dxferp) = match data {
            Some(buf) => {
                // For SECURITY SET PASSWORD: also set sector count = 1
                cdb[6] = 1; // sector count
                (SG_DXFER_TO_DEV, buf.len() as u32, buf.as_mut_ptr())
            }
            None => (SG_DXFER_NONE, 0u32, std::ptr::null_mut()),
        };

        let mut hdr = SgIoHdr {
            interface_id: b'S' as i32,
            dxfer_direction: direction,
            cmd_len: 16,
            mx_sb_len: sense_buf.len() as u8,
            dxfer_len,
            dxferp,
            cmdp: cdb.as_ptr(),
            sbp: sense_buf.as_mut_ptr(),
            timeout: timeout_ms,
            ..Default::default()
        };

        let fd_raw = fd;
        let ret = unsafe { libc::ioctl(fd_raw, SG_IO as _, &mut hdr as *mut _) };
        if ret < 0 {
            return Err(DriveWipeError::Ioctl {
                operation: format!("SG_IO ATA command {:#04x}", command),
                source: std::io::Error::last_os_error(),
            });
        }

        // Check for errors
        if hdr.status != 0 || hdr.host_status != 0 || hdr.driver_status != 0 {
            return Err(DriveWipeError::FirmwareError {
                reason: format!(
                    "ATA command {:#04x} failed: status={}, host_status={}, driver_status={}",
                    command, hdr.status, hdr.host_status, hdr.driver_status
                ),
            });
        }

        Ok(())
    }

    pub fn ata_secure_erase_linux(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        enhanced: bool,
    ) -> Result<()> {
        use std::os::unix::io::AsRawFd;

        let path = &drive.path;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DriveWipeError::InsufficientPrivileges {
                        message: format!("Cannot open {} — run as root", path.display()),
                    }
                } else {
                    DriveWipeError::Io {
                        path: path.to_path_buf(),
                        source: e,
                    }
                }
            })?;
        let fd = file.as_raw_fd();

        // Step 1: SECURITY SET PASSWORD
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 0.0,
        });

        let mut pwd_block = build_password_block(false);
        sg_io_ata16(
            fd,
            ATA_CMD_SEC_SET_PASS,
            ATA_PROTO_PIO_DATA_OUT,
            Some(&mut pwd_block),
            30_000, // 30 seconds
        )?;

        // Step 2: SECURITY ERASE UNIT
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 5.0,
        });

        let mut erase_block = build_password_block(enhanced);
        sg_io_ata16(
            fd,
            ATA_CMD_SEC_ERASE_UNIT,
            ATA_PROTO_PIO_DATA_OUT,
            Some(&mut erase_block),
            ATA_ERASE_TIMEOUT_MS,
        )?;

        // Step 3: SECURITY DISABLE PASSWORD (belt-and-suspenders)
        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 99.0,
        });

        let mut disable_block = build_password_block(false);
        // If this fails it's not fatal — a successful erase should have
        // already cleared all passwords.
        let _ = sg_io_ata16(
            fd,
            ATA_CMD_SEC_DISABLE_PASS,
            ATA_PROTO_PIO_DATA_OUT,
            Some(&mut disable_block),
            30_000,
        );

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }
}

// ── Windows implementation ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_ata {
    use super::ata_consts::*;
    use super::*;
    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::core::PCWSTR;

    /// IOCTL_ATA_PASS_THROUGH
    const IOCTL_ATA_PASS_THROUGH: u32 = 0x0004D02C;

    /// ATA_PASS_THROUGH_EX flags
    const ATA_FLAGS_DATA_OUT: u16 = 0x02;
    const ATA_FLAGS_DRDY_REQUIRED: u16 = 0x01;

    /// ATA_PASS_THROUGH_EX structure (48 bytes).
    #[repr(C)]
    #[allow(non_snake_case)]
    struct AtaPassThroughEx {
        Length: u16,
        AtaFlags: u16,
        PathId: u8,
        TargetId: u8,
        Lun: u8,
        ReservedAsUchar: u8,
        DataTransferLength: u32,
        TimeOutValue: u32,
        ReservedAsUlong: u32,
        DataBufferOffset: usize,
        PreviousTaskFile: [u8; 8],
        CurrentTaskFile: [u8; 8],
    }

    /// Combined buffer: ATA_PASS_THROUGH_EX header + 512-byte data payload.
    #[repr(C)]
    struct AtaPassThroughExWithData {
        header: AtaPassThroughEx,
        data: [u8; ATA_PASSWORD_BLOCK_SIZE],
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn build_password_block_win(enhanced: bool) -> [u8; ATA_PASSWORD_BLOCK_SIZE] {
        let mut block = [0u8; ATA_PASSWORD_BLOCK_SIZE];
        if enhanced {
            block[0] = 0x02;
        }
        let pwd_len = ATA_TEMP_PASSWORD.len().min(32);
        block[2..2 + pwd_len].copy_from_slice(&ATA_TEMP_PASSWORD[..pwd_len]);
        block
    }

    fn ata_passthrough(
        handle: HANDLE,
        command: u8,
        data: Option<&[u8; ATA_PASSWORD_BLOCK_SIZE]>,
        timeout_secs: u32,
    ) -> Result<()> {
        let has_data = data.is_some();
        let mut buf: AtaPassThroughExWithData = unsafe { mem::zeroed() };

        buf.header.Length = mem::size_of::<AtaPassThroughEx>() as u16;
        buf.header.AtaFlags =
            ATA_FLAGS_DRDY_REQUIRED | if has_data { ATA_FLAGS_DATA_OUT } else { 0 };
        buf.header.TimeOutValue = timeout_secs;

        if has_data {
            buf.header.DataTransferLength = ATA_PASSWORD_BLOCK_SIZE as u32;
            buf.header.DataBufferOffset = mem::offset_of!(AtaPassThroughExWithData, data);
            if let Some(d) = data {
                buf.data = *d;
            }
        }

        // CurrentTaskFile: [Features, SectorCount, SectorNumber, CylLow, CylHigh, DevHead, Command, Reserved]
        buf.header.CurrentTaskFile[0] = 0; // Features
        buf.header.CurrentTaskFile[1] = if has_data { 1 } else { 0 }; // Sector count
        buf.header.CurrentTaskFile[6] = command;

        let buf_size = if has_data {
            mem::size_of::<AtaPassThroughExWithData>() as u32
        } else {
            mem::size_of::<AtaPassThroughEx>() as u32
        };

        let mut bytes_returned: u32 = 0;
        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_ATA_PASS_THROUGH,
                Some(&buf as *const _ as *const _),
                buf_size,
                Some(&mut buf as *mut _ as *mut _),
                buf_size,
                Some(&mut bytes_returned),
                None,
            )
        }
        .map_err(|e| DriveWipeError::FirmwareError {
            reason: format!("ATA passthrough command {:#04x} failed: {}", command, e),
        })?;

        // Check the returned task file for errors (bit 0 of Status = error)
        let status = buf.header.CurrentTaskFile[6];
        if status & 0x01 != 0 {
            return Err(DriveWipeError::FirmwareError {
                reason: format!(
                    "ATA command {:#04x} returned error status: {:#04x}",
                    command, status
                ),
            });
        }

        Ok(())
    }

    pub fn ata_secure_erase_windows(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        enhanced: bool,
    ) -> Result<()> {
        let path_str = drive.path.to_string_lossy().to_string();
        let wide = to_wide_null(&path_str);

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                (0x80000000u32 | 0x40000000u32).into(), // GENERIC_READ | GENERIC_WRITE
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        }
        .map_err(|e| DriveWipeError::FirmwareError {
            reason: format!("Failed to open {}: {}", path_str, e),
        })?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(DriveWipeError::DeviceNotFound(drive.path.clone()));
        }

        let result = (|| -> Result<()> {
            // Step 1: SECURITY SET PASSWORD
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent: 0.0,
            });

            let pwd_block = build_password_block_win(false);
            ata_passthrough(handle, ATA_CMD_SEC_SET_PASS, Some(&pwd_block), 30)?;

            // Step 2: SECURITY ERASE UNIT
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent: 5.0,
            });

            let erase_block = build_password_block_win(enhanced);
            let timeout_secs = ATA_ERASE_TIMEOUT_MS / 1000;
            ata_passthrough(
                handle,
                ATA_CMD_SEC_ERASE_UNIT,
                Some(&erase_block),
                timeout_secs,
            )?;

            // Step 3: SECURITY DISABLE PASSWORD
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent: 99.0,
            });

            let disable_block = build_password_block_win(false);
            let _ = ata_passthrough(handle, ATA_CMD_SEC_DISABLE_PASS, Some(&disable_block), 30);

            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent: 100.0,
            });

            Ok(())
        })();

        unsafe {
            let _ = CloseHandle(handle);
        }
        result
    }
}
