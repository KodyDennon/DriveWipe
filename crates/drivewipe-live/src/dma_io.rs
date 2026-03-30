//! Zero-copy DMA I/O via the DriveWipe kernel module.
//!
//! When the kernel module is loaded, DMA I/O uses `dma_alloc_coherent()` kernel
//! buffers for zero-copy transfers, bypassing the page cache entirely. This
//! provides maximum throughput for wipe operations.
//!
//! Falls back to normal `pread`/`pwrite` with `O_DIRECT` when the kernel
//! module is unavailable.

use drivewipe_core::error::{DriveWipeError, Result};
use log;

use crate::kernel_module::{DwDmaRequest, KernelModule, set_device_path};

/// DMA I/O handle for a specific device.
pub struct DmaIo {
    device_path: String,
    module: Option<KernelModule>,
    /// Cached file handle for userspace O_DIRECT fallback.
    fallback_file: Option<std::fs::File>,
}

impl DmaIo {
    /// Create a new DMA I/O handle. Attempts to open the kernel module;
    /// falls back to userspace I/O if unavailable.
    pub fn new(device_path: &str) -> Self {
        let module = KernelModule::open().ok();
        if module.is_some() {
            log::info!("DMA I/O: using kernel module for {}", device_path);
        } else {
            log::info!(
                "DMA I/O: kernel module unavailable, using userspace I/O for {}",
                device_path
            );
        }

        // Pre-open the fallback file handle with O_DIRECT when not using the kernel module.
        let fallback_file = if module.is_none() {
            Self::open_direct(device_path).ok()
        } else {
            None
        };

        Self {
            device_path: device_path.to_string(),
            module,
            fallback_file,
        }
    }

    /// Open the device with O_DIRECT for userspace fallback.
    fn open_direct(device_path: &str) -> Result<std::fs::File> {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_DIRECT)
            .open(device_path)
            .map_err(|e| DriveWipeError::Io {
                path: device_path.into(),
                source: e,
            })
    }

    /// Whether we are using kernel module DMA (true) or userspace fallback (false).
    pub fn is_dma_active(&self) -> bool {
        self.module.is_some()
    }

    /// Write data to the device at the given byte offset.
    pub fn write(&self, offset: u64, data: &[u8]) -> Result<u64> {
        if let Some(ref km) = self.module {
            self.write_dma(km, offset, data)
        } else {
            self.write_direct(offset, data)
        }
    }

    /// Read data from the device at the given byte offset.
    pub fn read(&self, offset: u64, buf: &mut [u8]) -> Result<u64> {
        if let Some(ref km) = self.module {
            self.read_dma(km, offset, buf)
        } else {
            self.read_direct(offset, buf)
        }
    }

    // ── Kernel module DMA path ───────────────────────────────────────────────

    fn write_dma(&self, km: &KernelModule, offset: u64, data: &[u8]) -> Result<u64> {
        let mut req = DwDmaRequest::default();
        set_device_path(&mut req.device, &self.device_path);
        req.offset = offset;
        req.length = data.len() as u64;
        req.data_ptr = data.as_ptr() as u64;
        req.write = 1;

        km.dma_io(&mut req)?;
        Ok(req.bytes_transferred)
    }

    fn read_dma(&self, km: &KernelModule, offset: u64, buf: &mut [u8]) -> Result<u64> {
        let mut req = DwDmaRequest::default();
        set_device_path(&mut req.device, &self.device_path);
        req.offset = offset;
        req.length = buf.len() as u64;
        req.data_ptr = buf.as_mut_ptr() as u64;
        req.write = 0;

        km.dma_io(&mut req)?;
        Ok(req.bytes_transferred)
    }

    // ── Userspace O_DIRECT fallback ──────────────────────────────────────────

    fn fallback_fd(&self) -> Result<std::os::unix::io::RawFd> {
        use std::os::unix::io::AsRawFd;
        self.fallback_file
            .as_ref()
            .map(|f| f.as_raw_fd())
            .ok_or_else(|| DriveWipeError::Io {
                path: self.device_path.clone().into(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Fallback file handle not available",
                ),
            })
    }

    fn write_direct(&self, offset: u64, data: &[u8]) -> Result<u64> {
        let fd = self.fallback_fd()?;
        let ret = unsafe {
            libc::pwrite(
                fd,
                data.as_ptr() as *const libc::c_void,
                data.len(),
                offset as i64,
            )
        };

        if ret < 0 {
            return Err(DriveWipeError::Io {
                path: self.device_path.clone().into(),
                source: std::io::Error::last_os_error(),
            });
        }

        Ok(ret as u64)
    }

    fn read_direct(&self, offset: u64, buf: &mut [u8]) -> Result<u64> {
        let fd = self.fallback_fd()?;
        let ret = unsafe {
            libc::pread(
                fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                offset as i64,
            )
        };

        if ret < 0 {
            return Err(DriveWipeError::Io {
                path: self.device_path.clone().into(),
                source: std::io::Error::last_os_error(),
            });
        }

        Ok(ret as u64)
    }
}
