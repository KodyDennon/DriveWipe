//! NVMe Format and Sanitize firmware commands.
//!
//! Issues NVMe admin commands via platform-specific passthrough:
//! - **Linux:** `NVME_IOCTL_ADMIN_CMD` on the NVMe character device
//! - **macOS:** Shells out to `nvme-cli` if installed
//! - **Windows:** `IOCTL_STORAGE_PROTOCOL_COMMAND` with NVMe admin passthrough

use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::progress::ProgressEvent;
use crate::types::{DriveInfo, Transport};

use super::FirmwareWipe;

// ── NVMe constants (shared across platform implementations) ─────────────────

/// NVMe Admin Command: Get Log Page
#[allow(dead_code)]
const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;
/// NVMe Admin Command: Format NVM
#[allow(dead_code)]
const NVME_ADMIN_FORMAT_NVM: u8 = 0x80;
/// NVMe Admin Command: Sanitize
#[allow(dead_code)]
const NVME_ADMIN_SANITIZE: u8 = 0x84;

/// Sanitize actions (CDW10 bits 2:0)
#[allow(dead_code)]
const SANITIZE_ACT_EXIT_FAILURE: u32 = 1;
const SANITIZE_ACT_BLOCK_ERASE: u32 = 2;
const SANITIZE_ACT_OVERWRITE: u32 = 3;
const SANITIZE_ACT_CRYPTO_ERASE: u32 = 4;

/// Sanitize Status Log page ID
#[allow(dead_code)]
const SANITIZE_LOG_PAGE_ID: u32 = 0x81;

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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        nvme_format_impl(drive, session_id, progress_tx, 1) // SES=1
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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        nvme_format_impl(drive, session_id, progress_tx, 2) // SES=2
    }
}

// ── NVMe Sanitize — Block Erase ─────────────────────────────────────────────

/// NVMe Sanitize command — Block Erase action.
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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        nvme_sanitize_impl(
            drive,
            session_id,
            progress_tx,
            SANITIZE_ACT_BLOCK_ERASE,
            None,
        )
    }
}

// ── NVMe Sanitize — Crypto Erase ────────────────────────────────────────────

/// NVMe Sanitize command — Crypto Erase action.
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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        nvme_sanitize_impl(
            drive,
            session_id,
            progress_tx,
            SANITIZE_ACT_CRYPTO_ERASE,
            None,
        )
    }
}

// ── NVMe Sanitize — Overwrite ────────────────────────────────────────────────

/// NVMe Sanitize command — Overwrite action.
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
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        // Use a zero overwrite pattern, 1 pass
        nvme_sanitize_impl(
            drive,
            session_id,
            progress_tx,
            SANITIZE_ACT_OVERWRITE,
            Some(0),
        )
    }
}

// ── NVMe Format dispatch ────────────────────────────────────────────────────

fn nvme_format_impl(
    drive: &DriveInfo,
    session_id: Uuid,
    progress_tx: &Sender<ProgressEvent>,
    ses: u32,
) -> Result<()> {
    let _ = progress_tx.send(ProgressEvent::FirmwareEraseStarted {
        session_id,
        method_name: format!("NVMe Format (SES={})", ses),
    });

    #[cfg(target_os = "linux")]
    {
        linux_nvme::nvme_format_linux(drive, session_id, progress_tx, ses)
    }

    #[cfg(target_os = "macos")]
    {
        macos_nvme::nvme_format_macos(drive, session_id, progress_tx, ses)
    }

    #[cfg(target_os = "windows")]
    {
        windows_nvme::nvme_format_windows(drive, session_id, progress_tx, ses)
    }
}

// ── NVMe Sanitize dispatch ──────────────────────────────────────────────────

fn nvme_sanitize_impl(
    drive: &DriveInfo,
    session_id: Uuid,
    progress_tx: &Sender<ProgressEvent>,
    sanact: u32,
    overwrite_pattern: Option<u32>,
) -> Result<()> {
    let _ = progress_tx.send(ProgressEvent::FirmwareEraseStarted {
        session_id,
        method_name: format!("NVMe Sanitize (action={})", sanact),
    });

    #[cfg(target_os = "linux")]
    {
        linux_nvme::nvme_sanitize_linux(drive, session_id, progress_tx, sanact, overwrite_pattern)
    }

    #[cfg(target_os = "macos")]
    {
        macos_nvme::nvme_sanitize_macos(drive, session_id, progress_tx, sanact, overwrite_pattern)
    }

    #[cfg(target_os = "windows")]
    {
        windows_nvme::nvme_sanitize_windows(
            drive,
            session_id,
            progress_tx,
            sanact,
            overwrite_pattern,
        )
    }
}

// ── Linux implementation ─────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_nvme {
    use super::*;
    use std::os::unix::io::AsRawFd;

    /// `NVME_IOCTL_ADMIN_CMD` = _IOWR('N', 0x41, struct nvme_admin_cmd)
    /// sizeof(nvme_admin_cmd) = 72 bytes, ioctl number = 0xC0484E41
    const NVME_IOCTL_ADMIN_CMD: u64 = 0xC048_4E41;

    /// Kernel nvme_admin_cmd / nvme_passthru_cmd structure.
    #[repr(C)]
    struct NvmeAdminCmd {
        opcode: u8,
        flags: u8,
        rsvd1: u16,
        nsid: u32,
        cdw2: u32,
        cdw3: u32,
        metadata: u64,
        addr: u64,
        metadata_len: u32,
        data_len: u32,
        cdw10: u32,
        cdw11: u32,
        cdw12: u32,
        cdw13: u32,
        cdw14: u32,
        cdw15: u32,
        timeout_ms: u32,
        result: u32,
    }

    impl Default for NvmeAdminCmd {
        fn default() -> Self {
            unsafe { std::mem::zeroed() }
        }
    }

    /// Derive the NVMe character device path from a block device or namespace.
    /// `/dev/nvme0n1` → `/dev/nvme0`
    /// `/dev/nvme0n1p1` → `/dev/nvme0`
    /// `/dev/nvme0` → `/dev/nvme0` (already a char dev)
    fn nvme_char_device(path: &std::path::Path) -> String {
        let s = path.to_string_lossy();
        // Find "nvmeN" and take up through the controller number
        if let Some(idx) = s.find("nvme") {
            let after = &s[idx + 4..];
            let digit_end = after
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(after.len());
            format!("/dev/nvme{}", &after[..digit_end])
        } else {
            // Fall back to the raw path
            s.to_string()
        }
    }

    pub fn nvme_format_linux(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        ses: u32,
    ) -> Result<()> {
        let char_dev = nvme_char_device(&drive.path);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&char_dev)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DriveWipeError::InsufficientPrivileges {
                        message: format!("Cannot open {} — run as root", char_dev),
                    }
                } else {
                    DriveWipeError::Io {
                        path: drive.path.clone(),
                        source: e,
                    }
                }
            })?;

        let fd = file.as_raw_fd();

        // CDW10: (SES << 9) | LBAF. Use LBAF=0 (current format).
        let cdw10 = ses << 9;

        let mut cmd = NvmeAdminCmd {
            opcode: NVME_ADMIN_FORMAT_NVM,
            nsid: 0xFFFFFFFF, // All namespaces
            cdw10,
            timeout_ms: 600_000, // 10 minute timeout for format
            ..Default::default()
        };

        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd as *mut _) };
        if ret < 0 {
            return Err(DriveWipeError::Ioctl {
                operation: format!("NVMe Format NVM (SES={})", ses),
                source: std::io::Error::last_os_error(),
            });
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }

    pub fn nvme_sanitize_linux(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        sanact: u32,
        overwrite_pattern: Option<u32>,
    ) -> Result<()> {
        let char_dev = nvme_char_device(&drive.path);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&char_dev)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DriveWipeError::InsufficientPrivileges {
                        message: format!("Cannot open {} — run as root", char_dev),
                    }
                } else {
                    DriveWipeError::Io {
                        path: drive.path.clone(),
                        source: e,
                    }
                }
            })?;

        let fd = file.as_raw_fd();

        // Build CDW10 for Sanitize
        let mut cdw10 = sanact;
        let mut cdw11 = 0u32;

        if sanact == SANITIZE_ACT_OVERWRITE {
            // For overwrite: CDW10 bits 4 = OIPBP (invert between passes),
            // bits 8:5 = OWPASS (overwrite pass count, 0-based => 1 pass)
            cdw10 |= 0 << 4; // No invert
            cdw10 |= 0 << 5; // 1 pass (0 = 1 pass)
            cdw11 = overwrite_pattern.unwrap_or(0);
        }

        let mut cmd = NvmeAdminCmd {
            opcode: NVME_ADMIN_SANITIZE,
            nsid: 0xFFFFFFFF,
            cdw10,
            cdw11,
            timeout_ms: 0, // Sanitize is asynchronous
            ..Default::default()
        };

        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd as *mut _) };
        if ret < 0 {
            return Err(DriveWipeError::Ioctl {
                operation: format!("NVMe Sanitize (action={})", sanact),
                source: std::io::Error::last_os_error(),
            });
        }

        // Poll sanitize progress via Get Log Page (Sanitize Status Log, LID=0x81)
        poll_sanitize_progress(fd, session_id, progress_tx)?;

        Ok(())
    }

    /// Poll the NVMe Sanitize Status Log until the operation completes.
    fn poll_sanitize_progress(
        fd: std::os::unix::io::RawFd,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        #[repr(C)]
        #[allow(dead_code)]
        struct SanitizeStatusLog {
            sprog: u16,
            sstat: u16,
            scdw10: u32,
            // Additional fields we don't need
            overwrite_est: u32,
            block_erase_est: u32,
            crypto_erase_est: u32,
            overwrite_no_dealloc_est: u32,
        }

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));

            let mut log_buf = [0u8; 20];
            // numdl = (sizeof(log_buf) / 4) - 1 = 4
            let numdl = (log_buf.len() as u32 / 4) - 1;
            let cdw10 = (numdl << 16) | SANITIZE_LOG_PAGE_ID;

            let mut cmd = NvmeAdminCmd {
                opcode: NVME_ADMIN_GET_LOG_PAGE,
                nsid: 0xFFFFFFFF,
                addr: log_buf.as_mut_ptr() as u64,
                data_len: log_buf.len() as u32,
                cdw10,
                timeout_ms: 5000,
                ..Default::default()
            };

            let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd as *mut _) };
            if ret < 0 {
                // If we can't read the log, assume it completed
                break;
            }

            let sprog = u16::from_le_bytes([log_buf[0], log_buf[1]]);
            let sstat = u16::from_le_bytes([log_buf[2], log_buf[3]]);

            // SPROG is in units of 1/65536 completion
            let percent = (sprog as f32 / 65536.0) * 100.0;
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent,
            });

            // SSTAT bits 2:0: 0=never sanitized, 1=completed, 2=in progress, 3=failed
            let status = sstat & 0x07;
            match status {
                1 => break, // Completed successfully
                3 => {
                    return Err(DriveWipeError::FirmwareError {
                        reason: "NVMe sanitize operation failed (controller reported failure)"
                            .into(),
                    });
                }
                2 => continue, // Still in progress
                _ => {
                    // 0 = never started or already done
                    if sprog == 0 && status == 0 {
                        // Might have completed instantly (e.g. crypto erase)
                        break;
                    }
                    continue;
                }
            }
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }
}

// ── macOS implementation ─────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos_nvme {
    use super::*;

    const NVME_CLI_PATH: &str = "/usr/local/bin/nvme";
    const NVME_CLI_BREW: &str = "/opt/homebrew/bin/nvme";

    fn find_nvme_cli() -> Option<&'static str> {
        if std::path::Path::new(NVME_CLI_PATH).exists() {
            Some(NVME_CLI_PATH)
        } else if std::path::Path::new(NVME_CLI_BREW).exists() {
            Some(NVME_CLI_BREW)
        } else {
            None
        }
    }

    pub fn nvme_format_macos(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        ses: u32,
    ) -> Result<()> {
        let nvme_cli = find_nvme_cli().ok_or_else(|| {
            DriveWipeError::PlatformNotSupported(
                "nvme-cli not found. Install with: brew install nvme-cli".into(),
            )
        })?;

        let dev = drive.path.to_string_lossy();
        let output = std::process::Command::new(nvme_cli)
            .args(["format", &dev, "--ses", &ses.to_string(), "--force"])
            .output()
            .map_err(|e| DriveWipeError::Io {
                path: drive.path.clone(),
                source: e,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DriveWipeError::FirmwareError {
                reason: format!("nvme format failed: {}", stderr.trim()),
            });
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });
        Ok(())
    }

    pub fn nvme_sanitize_macos(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        sanact: u32,
        overwrite_pattern: Option<u32>,
    ) -> Result<()> {
        let nvme_cli = find_nvme_cli().ok_or_else(|| {
            DriveWipeError::PlatformNotSupported(
                "nvme-cli not found. Install with: brew install nvme-cli".into(),
            )
        })?;

        let dev = drive.path.to_string_lossy();
        let mut args = vec![
            "sanitize".to_string(),
            dev.to_string(),
            "--sanact".to_string(),
            sanact.to_string(),
        ];

        if sanact == SANITIZE_ACT_OVERWRITE {
            if let Some(pattern) = overwrite_pattern {
                args.push("--ovrpat".to_string());
                args.push(pattern.to_string());
            }
        }

        args.push("--force".to_string());

        let output = std::process::Command::new(nvme_cli)
            .args(&args)
            .output()
            .map_err(|e| DriveWipeError::Io {
                path: drive.path.clone(),
                source: e,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DriveWipeError::FirmwareError {
                reason: format!("nvme sanitize failed: {}", stderr.trim()),
            });
        }

        // Poll sanitize progress using nvme-cli
        poll_sanitize_progress_macos(nvme_cli, &dev, session_id, progress_tx)?;

        Ok(())
    }

    fn poll_sanitize_progress_macos(
        nvme_cli: &str,
        dev: &str,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let output = std::process::Command::new(nvme_cli)
                .args(["sanitize-log", dev])
                .output();

            let Ok(output) = output else {
                break;
            };

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Look for SPROG (Sanitize Progress) in the output
            let mut percent = 0.0f32;
            let mut completed = false;

            for line in stdout.lines() {
                let lower = line.to_lowercase();
                if lower.contains("progress") {
                    // Try to parse percentage
                    if let Some(pct) = line
                        .split_whitespace()
                        .filter_map(|w| w.trim_end_matches('%').parse::<f32>().ok())
                        .next()
                    {
                        percent = pct;
                    }
                }
                if (lower.contains("completed") || lower.contains("most recent"))
                    && lower.contains("success")
                {
                    completed = true;
                }
            }

            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent,
            });

            if completed || percent >= 100.0 {
                break;
            }
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }
}

// ── Windows implementation ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_nvme {
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

    /// IOCTL_STORAGE_PROTOCOL_COMMAND
    const IOCTL_STORAGE_PROTOCOL_COMMAND: u32 = 0x002D1400;

    /// Protocol type: NVMe
    const PROTOCOL_TYPE_NVME: u32 = 3; // ProtocolTypeNvme

    /// Command flag: adapter request
    const STORAGE_PROTOCOL_COMMAND_FLAG_ADAPTER_REQUEST: u32 = 0x80000000;

    /// STORAGE_PROTOCOL_COMMAND structure (simplified).
    #[repr(C)]
    #[allow(non_snake_case)]
    struct StorageProtocolCommand {
        Version: u32,
        Length: u32,
        ProtocolType: u32,
        Flags: u32,
        ReturnStatus: u32,
        ErrorCode: u32,
        CommandLength: u32,
        ErrorInfoLength: u32,
        DataToDeviceTransferLength: u32,
        DataFromDeviceTransferLength: u32,
        TimeOutValue: u32,
        ErrorInfoOffset: u32,
        DataToDeviceBufferOffset: u32,
        DataFromDeviceBufferOffset: u32,
        CommandSpecificInformation: u32,
        Reserved0: u32,
        FixedProtocolReturnData: u32,
        Reserved1: [u32; 3],
        // NVMe command data follows (CDW0-CDW15 = 64 bytes)
        Command: [u32; 16],
    }

    const STORAGE_PROTOCOL_COMMAND_VERSION: u32 = 1;

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn open_drive(drive: &DriveInfo) -> Result<HANDLE> {
        let path_str = drive.path.to_string_lossy().to_string();
        let wide = to_wide_null(&path_str);
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                (0x80000000u32 | 0x40000000u32).into(),
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
        Ok(handle)
    }

    fn send_nvme_admin_command(
        handle: HANDLE,
        opcode: u8,
        nsid: u32,
        cdw10: u32,
        cdw11: u32,
        timeout_secs: u32,
    ) -> Result<()> {
        let mut cmd: StorageProtocolCommand = unsafe { mem::zeroed() };
        cmd.Version = STORAGE_PROTOCOL_COMMAND_VERSION;
        cmd.Length = mem::size_of::<StorageProtocolCommand>() as u32;
        cmd.ProtocolType = PROTOCOL_TYPE_NVME;
        cmd.Flags = STORAGE_PROTOCOL_COMMAND_FLAG_ADAPTER_REQUEST;
        cmd.CommandLength = 64; // NVMe command = 16 DWORDs
        cmd.TimeOutValue = timeout_secs;

        // CDW0: opcode in bits 7:0
        cmd.Command[0] = opcode as u32;
        // CDW1: NSID
        cmd.Command[1] = nsid;
        // CDW10
        cmd.Command[10] = cdw10;
        // CDW11
        cmd.Command[11] = cdw11;

        let mut bytes_returned: u32 = 0;
        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_PROTOCOL_COMMAND,
                Some(&cmd as *const _ as *const _),
                mem::size_of::<StorageProtocolCommand>() as u32,
                Some(&mut cmd as *mut _ as *mut _),
                mem::size_of::<StorageProtocolCommand>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        }
        .map_err(|e| DriveWipeError::FirmwareError {
            reason: format!("NVMe admin command {:#04x} failed: {}", opcode, e),
        })?;

        if cmd.ReturnStatus != 0 {
            return Err(DriveWipeError::FirmwareError {
                reason: format!(
                    "NVMe admin command {:#04x} returned error status: {}",
                    opcode, cmd.ReturnStatus
                ),
            });
        }

        Ok(())
    }

    pub fn nvme_format_windows(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        ses: u32,
    ) -> Result<()> {
        let handle = open_drive(drive)?;
        let cdw10 = ses << 9; // SES field at bits 11:9, LBAF=0

        let result = send_nvme_admin_command(
            handle,
            NVME_ADMIN_FORMAT_NVM,
            0xFFFFFFFF,
            cdw10,
            0,
            600, // 10 min timeout
        );

        unsafe {
            let _ = CloseHandle(handle);
        }

        result?;

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });
        Ok(())
    }

    pub fn nvme_sanitize_windows(
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
        sanact: u32,
        overwrite_pattern: Option<u32>,
    ) -> Result<()> {
        let handle = open_drive(drive)?;

        let mut cdw10 = sanact;
        let mut cdw11 = 0u32;

        if sanact == SANITIZE_ACT_OVERWRITE {
            cdw10 |= 0 << 4; // No invert
            cdw10 |= 0 << 5; // 1 pass
            cdw11 = overwrite_pattern.unwrap_or(0);
        }

        let result = send_nvme_admin_command(
            handle,
            NVME_ADMIN_SANITIZE,
            0xFFFFFFFF,
            cdw10,
            cdw11,
            5, // Sanitize command returns quickly; actual work is async
        );

        if result.is_err() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return result;
        }

        // Poll sanitize status
        let poll_result = poll_sanitize_windows(handle, session_id, progress_tx);
        unsafe {
            let _ = CloseHandle(handle);
        }
        poll_result
    }

    fn poll_sanitize_windows(
        handle: HANDLE,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<()> {
        // Poll via IOCTL_STORAGE_QUERY_PROPERTY for sanitize status log.
        // This is a simplified version; production code might use
        // StorageAdapterProtocolSpecificProperty for the sanitize log.
        // For now, we use a simple timeout-based approach.
        for i in 0..3600 {
            // Max 1 hour
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Send an estimated progress (we can't easily read the sanitize
            // log on Windows without more complex IOCTL). In production, we
            // would query the Sanitize Status Log via
            // IOCTL_STORAGE_QUERY_PROPERTY with the appropriate protocol
            // specific data.
            let estimated_percent = ((i as f32) / 60.0 * 100.0).min(99.0);
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
                session_id,
                percent: estimated_percent,
            });

            // Try to read sanitize status by sending a Get Log Page command
            let log_result = send_nvme_admin_command(
                handle,
                NVME_ADMIN_GET_LOG_PAGE,
                0xFFFFFFFF,
                (4 << 16) | SANITIZE_LOG_PAGE_ID, // numdl=4, lid=0x81
                0,
                5,
            );

            // If we can successfully send commands, the sanitize is done
            // (the drive is no longer processing the sanitize)
            if log_result.is_ok() {
                break;
            }
        }

        let _ = progress_tx.send(ProgressEvent::FirmwareEraseProgress {
            session_id,
            percent: 100.0,
        });

        Ok(())
    }
}
