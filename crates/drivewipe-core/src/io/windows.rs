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
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::mem;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_FLAG_NO_BUFFERING,
    FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::OVERLAPPED;
#[cfg(target_os = "windows")]
use windows::Win32::System::Ioctl::{
    DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_DISK_GET_LENGTH_INFO,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;
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

#[cfg(target_os = "windows")]
fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
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

        // Open the physical drive with direct, write-through access.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                (0x80000000 | 0x40000000).into(), // GENERIC_READ | GENERIC_WRITE
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
                None,
            )
        }
        .map_err(|e| {
            let code = e.code().0 as u32;
            if code == 5 {
                // ERROR_ACCESS_DENIED
                DriveWipeError::InsufficientPrivileges {
                    message: format!(
                        "Access denied opening {}. Run as Administrator.",
                        path.display()
                    ),
                }
            } else if code == 2 || code == 3 {
                // ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND
                DriveWipeError::DeviceNotFound(path.to_path_buf())
            } else {
                DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: std::io::Error::from_raw_os_error(code as i32),
                }
            }
        })?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        // Query capacity via IOCTL_DISK_GET_LENGTH_INFO.
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
                unsafe { let _ = CloseHandle(handle); }
                return Err(DriveWipeError::DeviceError(format!(
                    "Failed to query disk length for {}",
                    path.display()
                )));
            }
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
                // Fall back to 512 bytes if the geometry query fails.
                512u32
            } else {
                geo.Geometry.BytesPerSector
            }
        };

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
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32)))?;

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
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32)))?;

        Ok(bytes_read as usize)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        unsafe { FlushFileBuffers(self.handle) }
            .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::from_raw_os_error(e.code().0 as i32)))?;
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
