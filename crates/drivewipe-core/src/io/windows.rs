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
    CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FlushFileBuffers, OPEN_EXISTING,
    ReadFile, WriteFile,
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

    /// Total device capacity in bytes, obtained via `IOCTL_DISK_GET_LENGTH_INFO`.
    capacity: u64,

    /// Logical sector size in bytes, obtained via `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX`.
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
    pub fn open(path: &Path, writable: bool) -> Result<Self> {
        let path_str = path.to_string_lossy();
        let wide_path = to_wide_null(&path_str);

        log::info!("Opening Windows device: {}", path.display());

        // CRITICAL: Enable SeBackupPrivilege and SeRestorePrivilege.
        // Even when running as Administrator, these privileges are DISABLED by default.
        // Without them, CreateFileW may succeed but WriteFile will fail with ACCESS_DENIED.
        log::debug!("[WINDOWS] Enabling raw disk privileges...");
        crate::platform::privilege::enable_raw_disk_privileges()?;
        log::debug!("[WINDOWS] Privileges enabled");

        // CRITICAL: On Windows 10/11, the disk must be OFFLINE before writing.
        // Windows monitors disk writes and blocks access when it detects partition
        // modifications, regardless of privileges. Setting the disk offline prevents
        // Windows from monitoring and blocking our writes.
        // Only set offline when opening for write — read-only operations don't need it.
        if writable {
            log::debug!("[WINDOWS] Setting disk offline...");

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
                Attributes: 0x1,     // Set OFFLINE
                AttributesMask: 0x1, // Modify OFFLINE attribute
                Reserved2: [0; 4],
            };

            // Open the disk first to set it offline
            let wide_for_offline = to_wide_null(&path_str);
            let offline_handle = unsafe {
                CreateFileW(
                    PCWSTR(wide_for_offline.as_ptr()),
                    0x80000000u32 | 0x40000000u32, // GENERIC_READ | GENERIC_WRITE
                    Default::default(),            // No sharing
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

                    unsafe {
                        let _ = CloseHandle(h);
                    }

                    if let Err(err) = offline_result {
                        log::warn!(
                            "[WINDOWS] Failed to set disk offline (code {}): wipe may fail if disk has active partitions",
                            err.code().0
                        );
                    } else {
                        log::debug!("[WINDOWS] Disk set offline");
                    }
                }
            }

            // Give Windows a moment to process the offline state
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Open the physical drive with direct, write-through access.
        log::debug!("[WINDOWS] Opening device...");

        const GENERIC_READ: u32 = 0x80000000;
        const GENERIC_WRITE: u32 = 0x40000000;
        const WRITE_DAC: u32 = 0x00040000;
        const READ_CONTROL: u32 = 0x00020000;
        const SYNCHRONIZE: u32 = 0x00100000;

        let access_rights = if writable {
            GENERIC_READ | GENERIC_WRITE | WRITE_DAC | READ_CONTROL | SYNCHRONIZE
        } else {
            GENERIC_READ | READ_CONTROL | SYNCHRONIZE
        };
        let share_mode = Default::default(); // 0 = no sharing (exclusive access)

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
            log::error!("CreateFileW failed for {}: error code {}", path.display(), code);
            if code == 5 {
                DriveWipeError::InsufficientPrivileges {
                    message: format!(
                        "Access denied opening {} (error 5). Ensure you are running as Administrator.",
                        path.display()
                    ),
                }
            } else if code == 2 || code == 3 {
                DriveWipeError::DeviceNotFound(path.to_path_buf())
            } else {
                DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: std::io::Error::from_raw_os_error(code as i32),
                }
            }
        })?;

        log::debug!("[WINDOWS] Device opened: {}", path.display());

        if handle == INVALID_HANDLE_VALUE {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        // Query capacity via IOCTL_DISK_GET_LENGTH_INFO.
        log::debug!("[WINDOWS] Querying capacity...");
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
            if let Err(e) = ok {
                let err_code = e.code().0;
                log::error!(
                    "IOCTL_DISK_GET_LENGTH_INFO failed for {}: error {}",
                    path.display(),
                    err_code
                );
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(DriveWipeError::DeviceError(format!(
                    "Failed to query disk length for {} (error code: {})",
                    path.display(),
                    err_code
                )));
            }
            log::debug!("[WINDOWS] Capacity: {} bytes", length_info);
            length_info as u64
        };

        // Query block size via IOCTL_DISK_GET_DRIVE_GEOMETRY_EX.
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
                log::warn!("[WINDOWS] Block size query failed, defaulting to 512");
                512u32
            } else {
                log::debug!("[WINDOWS] Block size: {}", geo.Geometry.BytesPerSector);
                geo.Geometry.BytesPerSector
            }
        };

        log::info!(
            "[WINDOWS] Device ready: {} — capacity {} bytes, block size {} bytes",
            path.display(),
            capacity,
            block_size
        );

        Ok(Self {
            handle,
            capacity,
            block_size,
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub fn open(_path: &Path, _writable: bool) -> Result<Self> {
        Err(DriveWipeError::PlatformNotSupported(
            "Windows device I/O is only available on Windows".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
impl RawDeviceIo for WindowsDeviceIo {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        use windows::Win32::System::IO::OVERLAPPED;

        // Use OVERLAPPED to specify the offset directly, making the
        // operation atomic (single syscall) instead of seek+write.
        let mut overlapped = OVERLAPPED::default();
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
            log::error!(
                "[WINDOWS] WriteFile failed: offset={}, size={}, error={}",
                offset,
                buf.len(),
                err_code
            );
            DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(err_code as i32))
        })?;

        Ok(bytes_written as usize)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        use windows::Win32::System::IO::OVERLAPPED;

        // Use OVERLAPPED to specify the offset directly (atomic positioned read).
        let mut overlapped = OVERLAPPED::default();
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

#[cfg(target_os = "windows")]
impl Drop for WindowsDeviceIo {
    fn drop(&mut self) {
        // Bring the disk back online before closing the handle.
        // Without this, the disk remains in offline state after DriveWipe exits,
        // requiring manual intervention via diskpart or Disk Management.
        const IOCTL_DISK_SET_DISK_ATTRIBUTES: u32 = 0x0007C0F4;

        #[repr(C)]
        #[allow(non_snake_case)]
        struct SetDiskAttributes {
            Version: u32,
            Persist: u8,
            Reserved1: [u8; 3],
            Attributes: u64,
            AttributesMask: u64,
            Reserved2: [u32; 4],
        }

        // Clear the OFFLINE attribute (set Attributes to 0, mask the OFFLINE bit).
        let attrs = SetDiskAttributes {
            Version: std::mem::size_of::<SetDiskAttributes>() as u32,
            Persist: 0,
            Reserved1: [0; 3],
            Attributes: 0,       // Clear OFFLINE
            AttributesMask: 0x1, // Modify OFFLINE attribute
            Reserved2: [0; 4],
        };

        let mut bytes_returned: u32 = 0;
        unsafe {
            let _ = DeviceIoControl(
                self.handle,
                IOCTL_DISK_SET_DISK_ATTRIBUTES,
                Some(&attrs as *const _ as *const _),
                std::mem::size_of::<SetDiskAttributes>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            );
            let _ = CloseHandle(self.handle);
        }
    }
}
