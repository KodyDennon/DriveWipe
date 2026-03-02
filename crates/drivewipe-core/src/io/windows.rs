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
    CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_BEGIN,
    FlushFileBuffers, OPEN_EXISTING, ReadFile, SetFilePointerEx, WriteFile,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;
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

        // CRITICAL: On Windows 10/11, the disk must be OFFLINE before writing.
        // Windows monitors disk writes and blocks access when it detects partition
        // modifications, regardless of privileges. Setting the disk offline prevents
        // Windows from monitoring and blocking our writes.
        eprintln!("[WINDOWS] Setting disk offline...");
        write_debug("Attempting to set disk offline via IOCTL_DISK_SET_DISK_ATTRIBUTES...");

        const IOCTL_DISK_SET_DISK_ATTRIBUTES: u32 = 0x0007C0F4;

        #[repr(C)]
        #[allow(non_snake_case)]
        struct SetDiskAttributes {
            Version: u32,
            Persist: u8, // BOOLEAN
            Reserved1: [u8; 3],
            Attributes: u64,
            AttributesMask: u64,
            Reserved2: [u32; 4],
        }

        // Attribute value 0x1 = DISK_ATTRIBUTE_OFFLINE
        let mut attrs = SetDiskAttributes {
            Version: mem::size_of::<SetDiskAttributes>() as u32,
            Persist: 0, // Don't persist across reboots
            Reserved1: [0; 3],
            Attributes: 0x1, // Set OFFLINE
            AttributesMask: 0x1, // Modify OFFLINE attribute
            Reserved2: [0; 4],
        };

        // Open the disk first to set it offline
        let wide_for_offline = to_wide_null(&path_str);
        let offline_handle = unsafe {
            CreateFileW(
                PCWSTR(wide_for_offline.as_ptr()),
                0x80000000u32 | 0x40000000u32, // GENERIC_READ | GENERIC_WRITE
                Default::default(), // No sharing
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        };

        if let Ok(h) = offline_handle {
            if h != INVALID_HANDLE_VALUE {
                let mut bytes_returned: u32 = 0;
                let offline_result = unsafe {
                    DeviceIoControl(
                        h,
                        IOCTL_DISK_SET_DISK_ATTRIBUTES,
                        Some(&mut attrs as *mut _ as *mut _),
                        mem::size_of::<SetDiskAttributes>() as u32,
                        None,
                        0,
                        Some(&mut bytes_returned),
                        None,
                    )
                };

                unsafe { let _ = CloseHandle(h); }

                if offline_result.is_ok() {
                    eprintln!("[WINDOWS] Disk set offline successfully");
                    write_debug("Disk set offline successfully");
                } else {
                    let err = offline_result.unwrap_err();
                    let err_msg = format!("Failed to set disk offline: error code {}", err.code().0);
                    eprintln!("[WINDOWS WARNING] {}", err_msg);
                    write_debug(&err_msg);
                    write_debug("Continuing anyway - wipe may fail if disk has partitions");
                }
            }
        }

        // Give Windows a moment to process the offline state
        std::thread::sleep(std::time::Duration::from_millis(500));
        write_debug("Waited 500ms after setting disk offline");

        // Open the physical drive with direct, write-through access.
        log::debug!("Calling CreateFileW for {}", path.display());
        eprintln!("[WINDOWS] Calling CreateFileW...");
        write_debug("Calling CreateFileW...");

        // CRITICAL: On Windows 10/11, writing to physical drives requires specific access rights.
        // We need GENERIC_READ | GENERIC_WRITE plus additional rights:
        // - WRITE_DAC (0x00040000): Modify security descriptor
        // - READ_CONTROL (0x00020000): Read security descriptor
        // - SYNCHRONIZE (0x00100000): Synchronous I/O operations
        const GENERIC_READ: u32 = 0x80000000;
        const GENERIC_WRITE: u32 = 0x40000000;
        const WRITE_DAC: u32 = 0x00040000;
        const READ_CONTROL: u32 = 0x00020000;
        const SYNCHRONIZE: u32 = 0x00100000;

        let access_rights = GENERIC_READ | GENERIC_WRITE | WRITE_DAC | READ_CONTROL | SYNCHRONIZE;

        write_debug(&format!("Opening with access rights: 0x{:X}", access_rights));
        eprintln!("[WINDOWS] Opening with access rights: 0x{:X}", access_rights);

        // CRITICAL: Use ZERO sharing mode for exclusive access.
        // Even FILE_SHARE_READ allows other processes to keep handles open, which
        // blocks raw disk writes. We need complete exclusivity.
        let share_mode = Default::default(); // 0 = no sharing

        write_debug("Opening with ZERO sharing mode (exclusive access)");
        eprintln!("[WINDOWS] Opening with ZERO sharing mode (exclusive access)");

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                access_rights,
                share_mode,
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

        // NOTE: FSCTL_LOCK_VOLUME only works on VOLUME handles (\\.\C:), not on
        // physical drive handles (\\.\PhysicalDrive0). We don't need to lock.

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
        // Log write parameters for first write to debug alignment issues
        static FIRST_WRITE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
        if FIRST_WRITE.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let debug_log = std::env::temp_dir().join("drivewipe_debug.log");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&debug_log)
            {
                use std::io::Write;
                let _ = writeln!(f, "[WRITE_AT] First write (SYNCHRONOUS I/O):");
                let _ = writeln!(f, "  Offset: {} (0x{:X})", offset, offset);
                let _ = writeln!(f, "  Buffer size: {} bytes", buf.len());
                let _ = writeln!(f, "  Buffer address: {:p}", buf.as_ptr());
                let _ = writeln!(f, "  Offset % 512 = {}", offset % 512);
                let _ = writeln!(f, "  Buffer size % 512 = {}", buf.len() % 512);
                let _ = writeln!(f, "  Buffer address % 512 = {}", buf.as_ptr() as usize % 512);
                let _ = writeln!(f, "  Block size: {}", self.block_size);
            }
            eprintln!("[WINDOWS WRITE_AT] First write (SYNCHRONOUS): offset={}, size={}, block_size={}",
                offset, buf.len(), self.block_size);
        }

        // Use synchronous I/O with SetFilePointerEx instead of OVERLAPPED
        // SetFilePointerEx to position the file pointer
        let distance_to_move = offset as i64;
        unsafe {
            SetFilePointerEx(
                self.handle,
                distance_to_move,
                None,
                FILE_BEGIN,
            )
        }
        .map_err(|e| {
            let err_code = e.code().0;
            let err_msg = format!("SetFilePointerEx failed: error code {} (0x{:X})", err_code, err_code as u32);
            eprintln!("[WINDOWS ERROR] {}", err_msg);
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(err_code as i32))
        })?;

        // Now write at the current file position (synchronous, no OVERLAPPED)
        let mut bytes_written: u32 = 0;
        unsafe {
            WriteFile(
                self.handle,
                Some(buf),
                Some(&mut bytes_written),
                None, // No OVERLAPPED - synchronous I/O
            )
        }
        .map_err(|e| {
            let err_code = e.code().0;
            let err_msg = format!(
                "[WINDOWS WRITE ERROR] WriteFile failed:\n  \
                Offset: {} (0x{:X})\n  \
                Buffer size: {} bytes\n  \
                Buffer address: {:p}\n  \
                Error code: {} (0x{:X})\n  \
                Error: {}",
                offset, offset, buf.len(), buf.as_ptr(), err_code, err_code as u32, e
            );
            eprintln!("{}", err_msg);

            // Also write to debug log
            let debug_log = std::env::temp_dir().join("drivewipe_debug.log");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&debug_log)
            {
                use std::io::Write;
                let _ = writeln!(f, "{}", err_msg);
            }

            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(err_code as i32))
        })?;

        Ok(bytes_written as usize)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        // Use synchronous I/O with SetFilePointerEx
        let distance_to_move = offset as i64;
        unsafe {
            SetFilePointerEx(
                self.handle,
                distance_to_move,
                None,
                FILE_BEGIN,
            )
        }
        .map_err(|e| {
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32))
        })?;

        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(
                self.handle,
                Some(buf),
                Some(&mut bytes_read),
                None, // No OVERLAPPED - synchronous I/O
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

#[cfg(target_os = "windows")]
impl Drop for WindowsDeviceIo {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
