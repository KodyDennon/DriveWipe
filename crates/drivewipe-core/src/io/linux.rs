//! Linux raw device I/O using `O_DIRECT`.
//!
//! Opens block devices with direct I/O so that every write bypasses the kernel
//! page cache. Manual sync via `sync()` ensures data is committed to storage.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::Command;

use super::RawDeviceIo;
use crate::error::{DriveWipeError, Result};

/// Raw device I/O handle for Linux block devices.
///
/// The underlying file descriptor is opened with `O_RDWR | O_DIRECT`
/// so that:
///
/// - `O_DIRECT` bypasses the kernel page cache, ensuring data goes straight to
///   the device's write-back buffer.
/// - Manual `sync()` after each pass flushes the device's internal write cache,
///   guaranteeing durability without the performance penalty of syncing every write.
///
/// Callers must ensure that I/O buffers are aligned to the device's logical
/// block size (typically 512 bytes) when using `O_DIRECT`.
pub struct LinuxDeviceIo {
    file: File,
    capacity: u64,
    block_size: u32,
}

impl LinuxDeviceIo {
    /// Open a block device for direct, synchronous I/O.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the block device (e.g. `/dev/sda`, `/dev/nvme0n1`).
    ///
    /// # Errors
    ///
    /// Returns [`DriveWipeError::DeviceNotFound`] if the path does not exist,
    /// or [`DriveWipeError::Io`] if the device cannot be opened (e.g.
    /// insufficient privileges).
    pub fn open(path: &Path) -> Result<Self> {
        // Validate the path is a block device (prevents arbitrary file overwrite).
        use std::os::unix::fs::FileTypeExt;
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_block_device() {
                    return Err(DriveWipeError::DeviceError(format!(
                        "{} is not a block device",
                        path.display()
                    )));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
            }
            Err(e) => {
                return Err(DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: e,
                });
            }
        }

        // Unmount all partitions of this device before opening for raw I/O.
        unmount_device(path);

        // Open with O_NOFOLLOW to prevent symlink attacks (TOCTOU mitigation).
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_DIRECT | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => DriveWipeError::DeviceNotFound(path.to_path_buf()),
                _ => DriveWipeError::Io {
                    path: path.to_path_buf(),
                    source: e,
                },
            })?;

        // Determine capacity by seeking to the end of the device.
        let capacity = file
            .seek(SeekFrom::End(0))
            .map_err(|e| DriveWipeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Seek back to the beginning so the fd is in a known state.
        file.seek(SeekFrom::Start(0))
            .map_err(|e| DriveWipeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Query the device's logical block size via ioctl(BLKSSZGET).
        // Falls back to 512 bytes if the ioctl fails.
        let mut block_size: u32 = 512;
        let ret = unsafe {
            libc::ioctl(
                file.as_raw_fd(),
                libc::BLKSSZGET as libc::c_ulong,
                &mut block_size,
            )
        };
        if ret == -1 {
            log::warn!(
                "BLKSSZGET ioctl failed on {}: {}, using default 512-byte sectors",
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

impl RawDeviceIo for LinuxDeviceIo {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        // `FileExt::write_at` maps to `pwrite(2)` on Unix.
        self.file
            .write_at(buf, offset)
            .map_err(DriveWipeError::IoGeneric)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        // `FileExt::read_at` maps to `pread(2)` on Unix.
        self.file
            .read_at(buf, offset)
            .map_err(DriveWipeError::IoGeneric)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        self.file.sync_all().map_err(DriveWipeError::IoGeneric)
    }
}

/// Unmount all mounted partitions of a block device before opening for raw I/O.
///
/// Reads `/proc/mounts` to find partitions belonging to the device (e.g.
/// `/dev/sda1`, `/dev/sda2` for `/dev/sda`) and unmounts each one.
fn unmount_device(path: &Path) {
    let dev_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_string(),
        None => return,
    };

    let mounts = match std::fs::read_to_string("/proc/mounts") {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to read /proc/mounts for unmount check: {}", e);
            return;
        }
    };

    // Find all mount entries whose device starts with our base device name
    // (e.g. /dev/sda matches /dev/sda1, /dev/sda2, etc.)
    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let Some(mount_dev) = parts.next() else {
            continue;
        };
        let Some(mount_point) = parts.next() else {
            continue;
        };

        // Check if this mount is a partition of our device.
        if mount_dev.starts_with(&format!("/dev/{dev_name}")) {
            log::info!("Unmounting {} (mounted at {})", mount_dev, mount_point);
            let result = Command::new("umount").arg(mount_point).output();
            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully unmounted {}", mount_point);
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to unmount {}: {}", mount_point, stderr.trim());
                }
                Err(e) => {
                    log::warn!("Failed to run umount {}: {}", mount_point, e);
                }
            }
        }
    }
}
