//! macOS raw device I/O using `F_NOCACHE`.
//!
//! Opens raw disk devices (`/dev/rdiskN`) with cache-bypass semantics so that
//! every write is committed directly to the storage medium without lingering
//! in the unified buffer cache.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use super::RawDeviceIo;
use crate::error::{DriveWipeError, Result};

/// Raw device I/O handle for macOS block devices.
///
/// The underlying file is opened in read-write mode and then configured with
/// `fcntl(F_NOCACHE, 1)` to disable the unified buffer cache.  This is the
/// macOS equivalent of Linux's `O_DIRECT`.
///
/// For best results, use the raw disk device (`/dev/rdiskN`) rather than the
/// block device (`/dev/diskN`).  The raw device avoids an extra layer of
/// buffering in the block device driver.
pub struct MacosDeviceIo {
    file: File,
    capacity: u64,
    block_size: u32,
}

impl MacosDeviceIo {
    /// Open a raw disk device for unbuffered I/O.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the raw device (e.g. `/dev/rdisk2`).
    ///
    /// # Errors
    ///
    /// Returns [`DriveWipeError::DeviceNotFound`] if the path does not exist,
    /// [`DriveWipeError::Io`] if the device cannot be opened, or
    /// [`DriveWipeError::Ioctl`] if `F_NOCACHE` cannot be set.
    pub fn open(path: &Path) -> Result<Self> {
        // Validate the path is a disk device (prevents arbitrary file overwrite).
        let path_str = path.to_string_lossy();
        if !path_str.starts_with("/dev/rdisk") && !path_str.starts_with("/dev/disk") {
            return Err(DriveWipeError::DeviceError(format!(
                "{} is not a disk device (expected /dev/rdiskN or /dev/diskN)",
                path.display()
            )));
        }

        // Open directly — handle NotFound in the error rather than a separate
        // exists() check (eliminates TOCTOU race).
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    DriveWipeError::DeviceNotFound(path.to_path_buf())
                }
                _ => DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: e,
                },
            })?;

        // Disable the unified buffer cache for this file descriptor.
        // This is the macOS equivalent of Linux's O_DIRECT.
        let fd = file.as_raw_fd();
        let ret = unsafe { libc::fcntl(fd, libc::F_NOCACHE, 1) };
        if ret == -1 {
            return Err(DriveWipeError::Ioctl {
                operation: "F_NOCACHE".to_string(),
                source: std::io::Error::last_os_error(),
            });
        }

        // Determine capacity by seeking to the end of the device.
        let capacity = file
            .seek(SeekFrom::End(0))
            .map_err(|e| DriveWipeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Seek back to the beginning.
        file.seek(SeekFrom::Start(0))
            .map_err(|e| DriveWipeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Query the device's logical block size via ioctl(DKIOCGETBLOCKSIZE).
        // DKIOCGETBLOCKSIZE is defined as _IOR('d', 24, u32) = 0x40046418.
        // Falls back to 512 bytes if the ioctl fails.
        const DKIOCGETBLOCKSIZE: libc::c_ulong = 0x40046418;
        let mut block_size: u32 = 512;
        let ret = unsafe { libc::ioctl(fd, DKIOCGETBLOCKSIZE, &mut block_size) };
        if ret == -1 {
            log::warn!(
                "DKIOCGETBLOCKSIZE ioctl failed on {}: {}, using default 512-byte sectors",
                path.display(),
                std::io::Error::last_os_error()
            );
            block_size = 512;
        }

        Ok(Self {
            file,
            capacity,
            block_size,
        })
    }
}

impl RawDeviceIo for MacosDeviceIo {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        self.file
            .write_at(buf, offset)
            .map_err(|e| DriveWipeError::IoGeneric(e))
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.file
            .read_at(buf, offset)
            .map_err(|e| DriveWipeError::IoGeneric(e))
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        self.file
            .sync_all()
            .map_err(|e| DriveWipeError::IoGeneric(e))
    }
}
