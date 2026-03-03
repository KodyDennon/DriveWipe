//! `/dev/drivewipe` kernel module interface.
//!
//! Provides a typed Rust wrapper around the custom DriveWipe kernel module's
//! ioctl API. The kernel module offers direct ATA/NVMe passthrough, HPA/DCO
//! commands, and DMA-coherent buffer management.
//!
//! All ioctl structs match the C definitions in `kernel/drivewipe/drivewipe_ioctl.h`.

use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsRawFd, RawFd};

use drivewipe_core::error::{DriveWipeError, Result};

// ── ioctl numbers ────────────────────────────────────────────────────────────

// Magic number 'D' (0x44), sequential command numbers.
const DW_IOC_MAGIC: u8 = b'D';

const DW_IOC_ATA_CMD: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x01, size_of::<DwAtaCmd>());
const DW_IOC_NVME_CMD: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x02, size_of::<DwNvmeCmd>());
const DW_IOC_HPA_DETECT: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x10, size_of::<DwHpaInfo>());
const DW_IOC_HPA_REMOVE: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x11, size_of::<DwHpaInfo>());
const DW_IOC_DCO_DETECT: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x20, size_of::<DwDcoInfo>());
const DW_IOC_DCO_RESTORE: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x21, size_of::<DwDcoInfo>());
const DW_IOC_DCO_FREEZE: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x22, size_of::<DwDcoInfo>());
const DW_IOC_DMA_IO: u64 = make_ioctl_rw(DW_IOC_MAGIC, 0x30, size_of::<DwDmaRequest>());
const DW_IOC_ATA_SEC_STATE: u64 =
    make_ioctl_rw(DW_IOC_MAGIC, 0x40, size_of::<DwAtaSecurityState>());
const DW_IOC_MODULE_INFO: u64 = make_ioctl_r(DW_IOC_MAGIC, 0x50, size_of::<DwModuleInfo>());

/// Construct an _IOWR ioctl number at compile time.
const fn make_ioctl_rw(magic: u8, nr: u8, size: usize) -> u64 {
    // _IOWR = direction(3) << 30 | size << 16 | magic << 8 | nr
    (3u64 << 30) | ((size as u64) << 16) | ((magic as u64) << 8) | (nr as u64)
}

/// Construct an _IOR ioctl number at compile time.
const fn make_ioctl_r(magic: u8, nr: u8, size: usize) -> u64 {
    (2u64 << 30) | ((size as u64) << 16) | ((magic as u64) << 8) | (nr as u64)
}

/// Compile-time sizeof helper.
const fn size_of<T>() -> usize {
    std::mem::size_of::<T>()
}

// ── Capability flags ─────────────────────────────────────────────────────────

pub const DW_CAP_ATA: u32 = 1 << 0;
pub const DW_CAP_NVME: u32 = 1 << 1;
pub const DW_CAP_HPA: u32 = 1 << 2;
pub const DW_CAP_DCO: u32 = 1 << 3;
pub const DW_CAP_DMA: u32 = 1 << 4;
pub const DW_CAP_ATA_SECURITY: u32 = 1 << 5;

// ── Shared structures (mirrors kernel/drivewipe/drivewipe_ioctl.h) ───────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwAtaCmd {
    pub command: u8,
    pub feature: u8,
    pub device: u8,
    pub protocol: u8,
    pub sector_count: u16,
    pub lba: u64,
    pub data_len: u32,
    pub data_ptr: u64,
    pub timeout_ms: u32,
    // Output fields
    pub status: u8,
    pub error: u8,
    pub result_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwNvmeCmd {
    pub opcode: u8,
    pub flags: u8,
    pub nsid: u32,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
    pub data_len: u32,
    pub data_ptr: u64,
    pub timeout_ms: u32,
    pub result: u32,
    pub status: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwHpaInfo {
    pub device: [u8; 64],
    pub current_max_lba: u64,
    pub native_max_lba: u64,
    pub hpa_present: u8,
    pub hpa_sectors: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwDcoInfo {
    pub device: [u8; 64],
    pub dco_present: u8,
    pub dco_real_max_lba: u64,
    pub dco_current_max: u64,
    pub dco_features: [u8; 512],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwAtaSecurityState {
    pub device: [u8; 64],
    pub supported: u8,
    pub enabled: u8,
    pub locked: u8,
    pub frozen: u8,
    pub count_expired: u8,
    pub enhanced_erase_supported: u8,
    pub erase_time_normal: u16,
    pub erase_time_enhanced: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwDmaRequest {
    pub device: [u8; 64],
    pub offset: u64,
    pub length: u64,
    pub data_ptr: u64,
    pub write: u8,
    pub bytes_transferred: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DwModuleInfo {
    pub version_major: u32,
    pub version_minor: u32,
    pub version_patch: u32,
    pub capabilities: u32,
}

// ── Zero-initialize helpers ──────────────────────────────────────────────────

impl Default for DwAtaCmd {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwNvmeCmd {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwHpaInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwDcoInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwAtaSecurityState {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwDmaRequest {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
impl Default for DwModuleInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

// ── Device handle ────────────────────────────────────────────────────────────

/// Handle to the `/dev/drivewipe` kernel module character device.
pub struct KernelModule {
    file: File,
}

impl KernelModule {
    /// Open the kernel module device. Returns an error if the device
    /// doesn't exist or the caller lacks `CAP_SYS_RAWIO`.
    pub fn open() -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/drivewipe")
            .map_err(|e| {
                DriveWipeError::KernelModuleNotLoaded(format!("Failed to open /dev/drivewipe: {e}"))
            })?;
        Ok(Self { file })
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Execute a raw ioctl, returning an error on failure.
    unsafe fn ioctl<T>(&self, request: u64, arg: &mut T) -> Result<()> {
        let ret = unsafe { libc::ioctl(self.fd(), request as _, arg as *mut T) };
        if ret < 0 {
            return Err(DriveWipeError::KernelModuleError(format!(
                "ioctl {:#x} failed: {}",
                request,
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    }

    /// Query module version and capabilities.
    pub fn module_info(&self) -> Result<DwModuleInfo> {
        let mut info = DwModuleInfo::default();
        unsafe { self.ioctl(DW_IOC_MODULE_INFO, &mut info)? };
        Ok(info)
    }

    /// Send a raw ATA command via the kernel module.
    pub fn ata_command(&self, cmd: &mut DwAtaCmd) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_ATA_CMD, cmd) }
    }

    /// Send a raw NVMe admin command via the kernel module.
    pub fn nvme_command(&self, cmd: &mut DwNvmeCmd) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_NVME_CMD, cmd) }
    }

    /// Detect HPA on a device.
    pub fn hpa_detect(&self, info: &mut DwHpaInfo) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_HPA_DETECT, info) }
    }

    /// Remove HPA from a device (set max address to native max).
    pub fn hpa_remove(&self, info: &mut DwHpaInfo) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_HPA_REMOVE, info) }
    }

    /// Detect DCO on a device.
    pub fn dco_detect(&self, info: &mut DwDcoInfo) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_DCO_DETECT, info) }
    }

    /// Restore DCO factory settings on a device.
    pub fn dco_restore(&self, info: &mut DwDcoInfo) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_DCO_RESTORE, info) }
    }

    /// Freeze DCO on a device (prevents further modification).
    pub fn dco_freeze(&self, info: &mut DwDcoInfo) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_DCO_FREEZE, info) }
    }

    /// Query ATA security state of a device.
    pub fn ata_security_state(&self, state: &mut DwAtaSecurityState) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_ATA_SEC_STATE, state) }
    }

    /// Perform DMA I/O via the kernel module.
    pub fn dma_io(&self, req: &mut DwDmaRequest) -> Result<()> {
        unsafe { self.ioctl(DW_IOC_DMA_IO, req) }
    }
}

/// Write a device path into a fixed-size byte array for ioctl structs.
pub fn set_device_path(buf: &mut [u8; 64], path: &str) {
    let bytes = path.as_bytes();
    let len = bytes.len().min(63); // Leave room for null terminator
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = 0;
}
