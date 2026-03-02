//! Windows raw device I/O using `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`.
//!
//! Opens physical drives (`\\.\PhysicalDriveN`) with direct write-through
//! semantics so that every write bypasses the filesystem cache and is committed
//! to the storage medium.

use std::path::Path;

use super::RawDeviceIo;
use crate::error::{DriveWipeError, Result};

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::mem;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING, ReadFile, WriteFile,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::OVERLAPPED;
#[cfg(target_os = "windows")]
use windows::Win32::System::Ioctl::{
    DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_DISK_GET_LENGTH_INFO,
};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

/// Raw device I/O handle for Windows physical drives.
///
/// The underlying handle is opened with `FILE_FLAG_NO_BUFFERING` and
/// `FILE_FLAG_WRITE_THROUGH` so that writes bypass the filesystem cache
/// and are committed synchronously to the device.
pub struct WindowsDeviceIo {
    #[cfg(target_os = "windows")]
    handle: HANDLE,
    #[cfg(not(target_os = "windows"))]
    handle: u64,

    /// Total device capacity in bytes, obtained via
    /// `IOCTL_DISK_GET_LENGTH_INFO`.
    capacity: u64,

    /// Logical sector size in bytes, obtained via
    /// `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX`.
    block_size: u32,
}

// SAFETY: Windows HANDLEs are process-wide resources that can be safely
// used from any thread. The raw pointer (*mut c_void) is an opaque kernel
// object handle, not a memory address.
#[cfg(target_os = "windows")]
unsafe impl Send for WindowsDeviceIo {}

#[cfg(target_os = "windows")]
fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

impl WindowsDeviceIo {
    /// Open a physical drive for direct, write-through I/O.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the physical drive (e.g. `\\.\PhysicalDrive0`).
    ///
    /// # Errors
    ///
    /// Returns [`DriveWipeError::DeviceNotFound`] if the path does not exist,
    /// or [`DriveWipeError::Io`] / [`DriveWipeError::InsufficientPrivileges`]
    /// if the device cannot be opened.
    #[cfg(target_os = "windows")]
    pub fn open(path: &Path) -> Result<Self> {
        let path_str = path.to_string_lossy();
        let wide_path = to_wide_null(&path_str);

        // Write to a debug file for troubleshooting - use temp dir
        let debug_log = std::env::temp_dir().join("drivewipe_debug.log");
        let write_debug = |msg: &str| {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&debug_log)
            {
                use std::io::Write;
                let _ = writeln!(f, "{}", msg);
            }
        };

        write_debug(&format!("\n========== NEW ATTEMPT =========="));
        write_debug(&format!("Opening: {}", path.display()));
        write_debug(&format!("Debug log location: {}", debug_log.display()));

        log::info!("Opening Windows device: {}", path.display());
        eprintln!("\n[WINDOWS DEBUG] Log file: {}", debug_log.display());
        eprintln!("[WINDOWS] Opening device: {}", path.display());

        // CRITICAL: Enable SeBackupPrivilege and SeRestorePrivilege.
        // Even when running as Administrator, these privileges are DISABLED by default.
        // Without them, CreateFileW may succeed but WriteFile will fail with ACCESS_DENIED.
        eprintln!("[WINDOWS] Enabling SeBackupPrivilege and SeRestorePrivilege...");
        write_debug("Enabling SeBackupPrivilege and SeRestorePrivilege...");
        crate::platform::privilege::enable_raw_disk_privileges()?;
        eprintln!("[WINDOWS] Privileges enabled successfully");
        write_debug("Privileges enabled successfully");

        // Dismount all volumes on this physical drive before opening.
        // Windows will block raw writes to a drive with mounted volumes.
        log::debug!("Dismounting volumes on {}", path.display());
        eprintln!("[WINDOWS] Dismounting volumes...");
        write_debug("Starting dismount...");

        dismount_volumes(&path_str);

        write_debug("Dismount complete");
        log::debug!("Dismount complete for {}", path.display());
        eprintln!("[WINDOWS] Dismount complete");

        // Give Windows a moment to release locks after dismount.
        // This is critical on Windows 10/11 where volume locks can persist briefly.
        std::thread::sleep(std::time::Duration::from_millis(500));
        write_debug("Waited 500ms after dismount for Windows to release locks");

        // Open the physical drive with direct, write-through access.
        log::debug!("Calling CreateFileW for {}", path.display());
        eprintln!("[WINDOWS] Calling CreateFileW...");
        write_debug("Calling CreateFileW...");

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                (0x80000000u32 | 0x40000000u32).into(), // GENERIC_READ | GENERIC_WRITE
                FILE_SHARE_READ, // Don't share write access for exclusive disk access
                None,
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
                None,
            )
        }
        .map_err(|e| {
            let code = e.code().0 as u32;
            let err_msg = format!("CreateFileW FAILED: error code {}", code);
            eprintln!("[WINDOWS ERROR] {}", err_msg);
            write_debug(&err_msg);
            write_debug(&format!("Error details: {}", e));
            log::error!("CreateFileW failed for {}: error code {}", path.display(), code);
            if code == 5 {
                // ERROR_ACCESS_DENIED
                write_debug("ERROR_ACCESS_DENIED (code 5)");
                DriveWipeError::InsufficientPrivileges {
                    message: format!(
                        "Access denied opening {} (error 5). Ensure you are running as Administrator.",
                        path.display()
                    ),
                }
            } else if code == 2 || code == 3 {
                // ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND
                write_debug("ERROR_FILE_NOT_FOUND or ERROR_PATH_NOT_FOUND");
                DriveWipeError::DeviceNotFound(path.to_path_buf())
            } else {
                write_debug(&format!("Other Windows error: {}", code));
                DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: std::io::Error::from_raw_os_error(code as i32),
                }
            }
        })?;

        eprintln!("[WINDOWS] CreateFileW SUCCESS");
        write_debug("CreateFileW SUCCESS");
        log::debug!("CreateFileW succeeded for {}", path.display());

        if handle == INVALID_HANDLE_VALUE {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        write_debug("Device handle opened successfully");

        // Query capacity via IOCTL_DISK_GET_LENGTH_INFO.
        log::debug!("Querying capacity for {}", path.display());
        eprintln!("[WINDOWS] Querying capacity...");
        write_debug("Querying capacity via IOCTL_DISK_GET_LENGTH_INFO...");

        let capacity = {
            let mut length_info: i64 = 0;
            let mut bytes_returned: u32 = 0;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_DISK_GET_LENGTH_INFO,
                    None,
                    0,
                    Some(&mut length_info as *mut _ as *mut _),
                    mem::size_of::<i64>() as u32,
                    Some(&mut bytes_returned),
                    None,
                )
            };
            if ok.is_err() {
                let err_code = ok.unwrap_err().code().0;
                let err_msg = format!("IOCTL_DISK_GET_LENGTH_INFO FAILED: error code {}", err_code);
                eprintln!("[WINDOWS ERROR] {}", err_msg);
                write_debug(&err_msg);
                log::error!("IOCTL_DISK_GET_LENGTH_INFO failed for {}: error {}", path.display(), err_code);
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(DriveWipeError::DeviceError(format!(
                    "Failed to query disk length for {} (error code: {})",
                    path.display(),
                    err_code
                )));
            }
            eprintln!("[WINDOWS] Capacity: {} bytes", length_info);
            write_debug(&format!("Capacity SUCCESS: {} bytes", length_info));
            log::debug!("Capacity for {}: {} bytes", path.display(), length_info);
            length_info as u64
        };

        // Query block size via IOCTL_DISK_GET_DRIVE_GEOMETRY_EX.
        write_debug("Querying block size via IOCTL_DISK_GET_DRIVE_GEOMETRY_EX...");
        let block_size = {
            let mut geo: DISK_GEOMETRY_EX = unsafe { mem::zeroed() };
            let mut bytes_returned: u32 = 0;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
                    None,
                    0,
                    Some(&mut geo as *mut _ as *mut _),
                    mem::size_of::<DISK_GEOMETRY_EX>() as u32,
                    Some(&mut bytes_returned),
                    None,
                )
            };
            if ok.is_err() {
                write_debug("Block size query failed, using default 512");
                // Fall back to 512 bytes if the geometry query fails.
                512u32
            } else {
                write_debug(&format!("Block size: {}", geo.Geometry.BytesPerSector));
                geo.Geometry.BytesPerSector
            }
        };

        write_debug(&format!("========== DEVICE OPENED SUCCESSFULLY =========="));
        write_debug(&format!("Handle: valid, Capacity: {} bytes, Block size: {} bytes", capacity, block_size));
        eprintln!("[WINDOWS] Device opened successfully!");
        eprintln!("[WINDOWS] Capacity: {} bytes, Block size: {}", capacity, block_size);

        Ok(Self {
            handle,
            capacity,
            block_size,
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub fn open(_path: &Path) -> Result<Self> {
        Err(DriveWipeError::PlatformNotSupported(
            "Windows device I/O is only available on Windows".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
impl RawDeviceIo for WindowsDeviceIo {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let mut overlapped: OVERLAPPED = unsafe { mem::zeroed() };
        overlapped.Anonymous.Anonymous.Offset = offset as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;

        let mut bytes_written: u32 = 0;
        unsafe {
            WriteFile(
                self.handle,
                Some(buf),
                Some(&mut bytes_written),
                Some(&mut overlapped),
            )
        }
        .map_err(|e| {
            let err_code = e.code().0;
            eprintln!("[WINDOWS WRITE ERROR] WriteFile failed at offset {}: error code {} ({})",
                offset, err_code, e);
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(err_code as i32))
        })?;

        Ok(bytes_written as usize)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let mut overlapped: OVERLAPPED = unsafe { mem::zeroed() };
        overlapped.Anonymous.Anonymous.Offset = offset as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;

        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(
                self.handle,
                Some(buf),
                Some(&mut bytes_read),
                Some(&mut overlapped),
            )
        }
        .map_err(|e| {
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32))
        })?;

        Ok(bytes_read as usize)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        unsafe { FlushFileBuffers(self.handle) }.map_err(|e| {
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32))
        })?;
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
impl RawDeviceIo for WindowsDeviceIo {
    fn write_at(&mut self, _offset: u64, _buf: &[u8]) -> Result<usize> {
        Err(DriveWipeError::PlatformNotSupported(
            "Windows write_at is only available on Windows".to_string(),
        ))
    }

    fn read_at(&mut self, _offset: u64, _buf: &mut [u8]) -> Result<usize> {
        Err(DriveWipeError::PlatformNotSupported(
            "Windows read_at is only available on Windows".to_string(),
        ))
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        Err(DriveWipeError::PlatformNotSupported(
            "Windows sync is only available on Windows".to_string(),
        ))
    }
}

/// Dismount all volumes on a physical drive before opening for raw I/O.
///
/// On Windows, mounted volumes prevent raw writes to the underlying physical
/// drive.  This function iterates volumes A:-Z:, checks if they reside on
/// the target physical drive, and dismounts them via
/// `FSCTL_LOCK_VOLUME` + `FSCTL_DISMOUNT_VOLUME`.
#[cfg(target_os = "windows")]
fn dismount_volumes(drive_path: &str) {
    use crate::drive::info::extract_windows_drive_number;

    log::info!("Dismounting volumes for {}", drive_path);

    // Extract the target drive number (e.g. 2 from \\.\PhysicalDrive2).
    let Some(target_num) = extract_windows_drive_number(drive_path) else {
        log::warn!("Could not extract drive number from {}", drive_path);
        return;
    };

    log::debug!("Target drive number: {}", target_num);

    // IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS
    const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x00560000;
    // FSCTL_LOCK_VOLUME / FSCTL_DISMOUNT_VOLUME
    const FSCTL_LOCK_VOLUME: u32 = 0x00090018;
    const FSCTL_DISMOUNT_VOLUME: u32 = 0x00090020;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct DiskExtent {
        DiskNumber: u32,
        _StartingOffset: i64,
        _ExtentLength: i64,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct VolumeDiskExtents {
        NumberOfDiskExtents: u32,
        Extents: [DiskExtent; 1],
    }

    for letter in b'A'..=b'Z' {
        let vol_path = format!("\\\\.\\{}:", letter as char);
        let wide = to_wide_null(&vol_path);

        let handle = unsafe {
            match CreateFileW(
                PCWSTR(wide.as_ptr()),
                0, // Query only — no read/write needed for the check
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            ) {
                Ok(h) if h != INVALID_HANDLE_VALUE => h,
                _ => continue,
            }
        };

        // Check which physical drive backs this volume.
        let mut extents: VolumeDiskExtents = unsafe { mem::zeroed() };
        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                None,
                0,
                Some(&mut extents as *mut _ as *mut _),
                mem::size_of::<VolumeDiskExtents>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if ok.is_err() || extents.NumberOfDiskExtents == 0 {
            unsafe { let _ = CloseHandle(handle); }
            continue;
        }

        if extents.Extents[0].DiskNumber != target_num {
            unsafe { let _ = CloseHandle(handle); }
            continue;
        }

        // This volume is on our target drive — lock and dismount it.
        log::info!("Dismounting volume {}:", letter as char);

        // Re-open with write access for lock/dismount.
        unsafe { let _ = CloseHandle(handle); }
        let wide = to_wide_null(&vol_path);
        let handle = unsafe {
            match CreateFileW(
                PCWSTR(wide.as_ptr()),
                (0x80000000u32 | 0x40000000u32).into(),
                FILE_SHARE_READ, // Exclusive write access for lock/dismount
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            ) {
                Ok(h) if h != INVALID_HANDLE_VALUE => h,
                _ => {
                    log::warn!("Failed to re-open volume {}: for dismount", letter as char);
                    continue;
                }
            }
        };

        // Lock the volume.
        let lock_ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_LOCK_VOLUME,
                None, 0, None, 0, None, None,
            )
        };
        if lock_ok.is_err() {
            log::warn!("Failed to lock volume {}:", letter as char);
        }

        // Dismount the volume.
        let dismount_ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_DISMOUNT_VOLUME,
                None, 0, None, 0, None, None,
            )
        };
        if dismount_ok.is_err() {
            log::warn!("Failed to dismount volume {}:", letter as char);
            unsafe { let _ = CloseHandle(handle); }
        } else {
            log::info!("Successfully dismounted volume {}:", letter as char);
            // IMPORTANT: Close the volume handle after dismount.
            // Keeping it open can cause ERROR_ACCESS_DENIED when writing to the
            // physical drive, especially on Windows 11. The volume will stay
            // dismounted for a short time which is enough for our operations.
            unsafe { let _ = CloseHandle(handle); }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsDeviceIo {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
