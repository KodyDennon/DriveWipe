//! HPA (Host Protected Area) detection and removal.
//!
//! The HPA is a hidden region at the end of a drive that the BIOS or OEM can
//! configure to hide sectors from the OS. Forensic tools must detect and
//! optionally remove the HPA to ensure complete sanitization.
//!
//! Detection works by comparing the drive's current max LBA (IDENTIFY DEVICE
//! words 60-61 / 100-103) with the native max LBA (READ NATIVE MAX ADDRESS).
//!
//! Two paths:
//! 1. Kernel module: `DW_IOC_HPA_DETECT` / `DW_IOC_HPA_REMOVE`
//! 2. SG_IO fallback: ATA_16 CDB with READ NATIVE MAX ADDRESS (0xF8 / 0x27)

use drivewipe_core::error::{DriveWipeError, Result};
use log;

use crate::kernel_module::{set_device_path, DwHpaInfo, KernelModule};

/// HPA detection result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HpaStatus {
    /// Device path.
    pub device: String,
    /// Current maximum addressable LBA (from IDENTIFY DEVICE).
    pub current_max_lba: u64,
    /// Native maximum LBA (from READ NATIVE MAX ADDRESS).
    pub native_max_lba: u64,
    /// Whether an HPA is present (native > current).
    pub hpa_present: bool,
    /// Number of sectors hidden by the HPA.
    pub hpa_sectors: u64,
    /// Size of the HPA in bytes (sectors * 512).
    pub hpa_bytes: u64,
}

/// Detect HPA on a device. Tries kernel module first, then SG_IO fallback.
pub fn detect_hpa(device_path: &str) -> Result<HpaStatus> {
    // Try kernel module first.
    if let Ok(status) = detect_hpa_kernel_module(device_path) {
        return Ok(status);
    }

    // Fall back to SG_IO.
    detect_hpa_sg_io(device_path)
}

/// Remove HPA from a device (set max address to native max).
/// This is a **destructive** operation — the HPA cannot be restored once removed
/// unless the drive supports DCO.
pub fn remove_hpa(device_path: &str) -> Result<HpaStatus> {
    // Try kernel module first.
    if let Ok(status) = remove_hpa_kernel_module(device_path) {
        return Ok(status);
    }

    // Fall back to SG_IO.
    remove_hpa_sg_io(device_path)
}

// ── Kernel module path ───────────────────────────────────────────────────────

fn detect_hpa_kernel_module(device_path: &str) -> Result<HpaStatus> {
    let km = KernelModule::open()?;
    let mut info = DwHpaInfo::default();
    set_device_path(&mut info.device, device_path);
    km.hpa_detect(&mut info)?;

    Ok(HpaStatus {
        device: device_path.to_string(),
        current_max_lba: info.current_max_lba,
        native_max_lba: info.native_max_lba,
        hpa_present: info.hpa_present != 0,
        hpa_sectors: info.hpa_sectors,
        hpa_bytes: info.hpa_sectors * 512,
    })
}

fn remove_hpa_kernel_module(device_path: &str) -> Result<HpaStatus> {
    let km = KernelModule::open()?;
    let mut info = DwHpaInfo::default();
    set_device_path(&mut info.device, device_path);
    km.hpa_remove(&mut info)?;

    Ok(HpaStatus {
        device: device_path.to_string(),
        current_max_lba: info.current_max_lba,
        native_max_lba: info.native_max_lba,
        hpa_present: info.hpa_present != 0,
        hpa_sectors: info.hpa_sectors,
        hpa_bytes: info.hpa_sectors * 512,
    })
}

// ── SG_IO fallback ───────────────────────────────────────────────────────────

/// SG_IO ioctl number.
const SG_IO: u32 = 0x2285;

/// ATA_16 CDB opcode (SAT).
const ATA_16: u8 = 0x85;

/// ATA protocol: non-data.
const ATA_PROTO_NON_DATA: u8 = 3 << 1;

/// ATA protocol: PIO data-in.
const ATA_PROTO_PIO_DATA_IN: u8 = 4 << 1;

/// SG_IO direction: no data transfer.
const SG_DXFER_NONE: i32 = -1;

/// SG_IO direction: from device.
const SG_DXFER_FROM_DEV: i32 = -3;

/// ATA command: READ NATIVE MAX ADDRESS (28-bit).
const ATA_CMD_READ_NATIVE_MAX: u8 = 0xF8;

/// ATA command: READ NATIVE MAX ADDRESS EXT (48-bit).
const ATA_CMD_READ_NATIVE_MAX_EXT: u8 = 0x27;

/// ATA command: SET MAX ADDRESS (28-bit).
const ATA_CMD_SET_MAX: u8 = 0xF9;

/// ATA command: SET MAX ADDRESS EXT (48-bit).
const ATA_CMD_SET_MAX_EXT: u8 = 0x37;

/// ATA command: IDENTIFY DEVICE.
const ATA_CMD_IDENTIFY: u8 = 0xEC;

/// SG_IO header structure (matches kernel sg_io_hdr_t).
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

/// Execute an ATA command via SG_IO (ATA_16 CDB).
fn sg_io_ata16_non_data(
    fd: std::os::unix::io::RawFd,
    command: u8,
    device_byte: u8,
) -> Result<[u8; 14]> {
    let mut sense = [0u8; 32];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_NON_DATA;
    cdb[2] = 0x20; // CK_COND=1 to get descriptor sense with register values
    cdb[13] = device_byte;
    cdb[14] = command;

    let mut hdr = SgIoHdr {
        interface_id: b'S' as i32,
        dxfer_direction: SG_DXFER_NONE,
        cmd_len: 16,
        mx_sb_len: sense.len() as u8,
        cmdp: cdb.as_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 10_000,
        ..Default::default()
    };

    let ret = unsafe { libc::ioctl(fd, SG_IO as _, &mut hdr as *mut _) };
    if ret < 0 {
        return Err(DriveWipeError::Ioctl {
            operation: format!("SG_IO READ NATIVE MAX ({:#04x})", command),
            source: std::io::Error::last_os_error(),
        });
    }

    // Extract the ATA register values from sense data.
    // Descriptor format sense data starts with 0x72.
    if sense[0] == 0x72 && sense.len() >= 22 {
        let mut regs = [0u8; 14];
        regs.copy_from_slice(&sense[8..22]);
        Ok(regs)
    } else {
        // Try to extract from fixed-format sense or return zeroed.
        Ok([0u8; 14])
    }
}

/// Execute IDENTIFY DEVICE via SG_IO and return the 512-byte response.
fn sg_io_identify_device(fd: std::os::unix::io::RawFd) -> Result<[u8; 512]> {
    let mut sense = [0u8; 32];
    let mut data = [0u8; 512];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_PIO_DATA_IN;
    cdb[2] = 0x0E; // t_length=SECTOR_COUNT, t_dir=FROM_DEV, byte_block=BLOCKS, ck_cond=0
    cdb[6] = 1; // sector count
    cdb[14] = ATA_CMD_IDENTIFY;

    let mut hdr = SgIoHdr {
        interface_id: b'S' as i32,
        dxfer_direction: SG_DXFER_FROM_DEV,
        cmd_len: 16,
        mx_sb_len: sense.len() as u8,
        dxfer_len: 512,
        dxferp: data.as_mut_ptr(),
        cmdp: cdb.as_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 10_000,
        ..Default::default()
    };

    let ret = unsafe { libc::ioctl(fd, SG_IO as _, &mut hdr as *mut _) };
    if ret < 0 {
        return Err(DriveWipeError::Ioctl {
            operation: "SG_IO IDENTIFY DEVICE".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }
    if hdr.status != 0 {
        return Err(DriveWipeError::HpaError(format!(
            "IDENTIFY DEVICE failed: status={}, host_status={}, driver_status={}",
            hdr.status, hdr.host_status, hdr.driver_status
        )));
    }
    Ok(data)
}

/// Read current max LBA from IDENTIFY DEVICE data (words as u16 LE).
fn identify_current_max_lba(identify: &[u8; 512]) -> u64 {
    // Words 100-103 (48-bit LBA capacity), only valid if word 83 bit 10 is set.
    let word83 = u16::from_le_bytes([identify[166], identify[167]]);
    let supports_48bit = (word83 & (1 << 10)) != 0;

    if supports_48bit {
        let w100 = u16::from_le_bytes([identify[200], identify[201]]) as u64;
        let w101 = u16::from_le_bytes([identify[202], identify[203]]) as u64;
        let w102 = u16::from_le_bytes([identify[204], identify[205]]) as u64;
        let w103 = u16::from_le_bytes([identify[206], identify[207]]) as u64;
        w100 | (w101 << 16) | (w102 << 32) | (w103 << 48)
    } else {
        // Words 60-61 (28-bit LBA capacity).
        let w60 = u16::from_le_bytes([identify[120], identify[121]]) as u64;
        let w61 = u16::from_le_bytes([identify[122], identify[123]]) as u64;
        w60 | (w61 << 16)
    }
}

fn detect_hpa_sg_io(device_path: &str) -> Result<HpaStatus> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(device_path)
        .map_err(|e| DriveWipeError::HpaError(format!("Cannot open {device_path}: {e}")))?;
    let fd = file.as_raw_fd();

    // Get current max LBA from IDENTIFY DEVICE.
    let identify = sg_io_identify_device(fd)?;
    let current_max = identify_current_max_lba(&identify);

    // Get native max LBA via READ NATIVE MAX ADDRESS.
    // Try 48-bit first.
    let word83 = u16::from_le_bytes([identify[166], identify[167]]);
    let supports_48bit = (word83 & (1 << 10)) != 0;

    let native_max = if supports_48bit {
        // READ NATIVE MAX ADDRESS EXT (0x27), device byte = 0x40 (LBA mode)
        let regs = sg_io_ata16_non_data(fd, ATA_CMD_READ_NATIVE_MAX_EXT, 0x40)?;
        // Parse 48-bit LBA from descriptor sense registers.
        parse_48bit_lba_from_regs(&regs)
    } else {
        // READ NATIVE MAX ADDRESS (0xF8), device byte = 0x40 (LBA mode)
        let regs = sg_io_ata16_non_data(fd, ATA_CMD_READ_NATIVE_MAX, 0x40)?;
        parse_28bit_lba_from_regs(&regs)
    };

    let hpa_present = native_max > current_max;
    let hpa_sectors = if hpa_present {
        native_max - current_max
    } else {
        0
    };

    log::info!(
        "HPA detection on {}: current={}, native={}, hpa={}",
        device_path,
        current_max,
        native_max,
        if hpa_present {
            format!("{} sectors", hpa_sectors)
        } else {
            "none".to_string()
        }
    );

    Ok(HpaStatus {
        device: device_path.to_string(),
        current_max_lba: current_max,
        native_max_lba: native_max,
        hpa_present,
        hpa_sectors,
        hpa_bytes: hpa_sectors * 512,
    })
}

fn remove_hpa_sg_io(device_path: &str) -> Result<HpaStatus> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path)
        .map_err(|e| DriveWipeError::HpaError(format!("Cannot open {device_path}: {e}")))?;
    let fd = file.as_raw_fd();

    // First detect current state.
    let identify = sg_io_identify_device(fd)?;
    let current_max = identify_current_max_lba(&identify);
    let word83 = u16::from_le_bytes([identify[166], identify[167]]);
    let supports_48bit = (word83 & (1 << 10)) != 0;

    let native_max = if supports_48bit {
        let regs = sg_io_ata16_non_data(fd, ATA_CMD_READ_NATIVE_MAX_EXT, 0x40)?;
        parse_48bit_lba_from_regs(&regs)
    } else {
        let regs = sg_io_ata16_non_data(fd, ATA_CMD_READ_NATIVE_MAX, 0x40)?;
        parse_28bit_lba_from_regs(&regs)
    };

    if native_max <= current_max {
        return Ok(HpaStatus {
            device: device_path.to_string(),
            current_max_lba: current_max,
            native_max_lba: native_max,
            hpa_present: false,
            hpa_sectors: 0,
            hpa_bytes: 0,
        });
    }

    // SET MAX ADDRESS to native max.
    let cmd = if supports_48bit {
        ATA_CMD_SET_MAX_EXT
    } else {
        ATA_CMD_SET_MAX
    };

    // Build the SET MAX ADDRESS CDB with native_max LBA.
    let mut sense = [0u8; 32];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_NON_DATA;
    cdb[2] = 0x20; // CK_COND

    if supports_48bit {
        // 48-bit LBA in CDB bytes 3-8 (HOB then current register pairs)
        cdb[4] = ((native_max >> 24) & 0xFF) as u8; // LBA high (HOB)
        cdb[6] = ((native_max >> 8) & 0xFF) as u8; // LBA mid (HOB)
        cdb[8] = (native_max & 0xFF) as u8; // LBA low (HOB)
        cdb[10] = ((native_max >> 40) & 0xFF) as u8; // LBA high
        cdb[12] = ((native_max >> 16) & 0xFF) as u8; // LBA mid
        cdb[13] = 0x40; // Device: LBA mode
    } else {
        cdb[8] = (native_max & 0xFF) as u8;
        cdb[10] = ((native_max >> 8) & 0xFF) as u8;
        cdb[12] = ((native_max >> 16) & 0xFF) as u8;
        cdb[13] = 0x40 | ((native_max >> 24) & 0x0F) as u8;
    }
    cdb[14] = cmd;

    let mut hdr = SgIoHdr {
        interface_id: b'S' as i32,
        dxfer_direction: SG_DXFER_NONE,
        cmd_len: 16,
        mx_sb_len: sense.len() as u8,
        cmdp: cdb.as_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 10_000,
        ..Default::default()
    };

    let ret = unsafe { libc::ioctl(fd, SG_IO as _, &mut hdr as *mut _) };
    if ret < 0 {
        return Err(DriveWipeError::HiddenAreaRemovalFailed {
            reason: format!(
                "SET MAX ADDRESS ioctl failed: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    if hdr.status != 0 {
        return Err(DriveWipeError::HiddenAreaRemovalFailed {
            reason: format!(
                "SET MAX ADDRESS failed: status={}, host_status={}, driver_status={}",
                hdr.status, hdr.host_status, hdr.driver_status
            ),
        });
    }

    log::info!(
        "HPA removed on {}: max LBA set from {} to {}",
        device_path,
        current_max,
        native_max
    );

    Ok(HpaStatus {
        device: device_path.to_string(),
        current_max_lba: native_max, // After removal, current = native
        native_max_lba: native_max,
        hpa_present: false,
        hpa_sectors: 0,
        hpa_bytes: 0,
    })
}

/// Parse a 48-bit LBA from ATA descriptor sense register data.
fn parse_48bit_lba_from_regs(regs: &[u8; 14]) -> u64 {
    // Descriptor format: [sector_count_ext, sector_count, lba_low_ext, lba_low,
    //                     lba_mid_ext, lba_mid, lba_high_ext, lba_high, ...]
    let lba_low = regs[3] as u64;
    let lba_mid = regs[5] as u64;
    let lba_high = regs[7] as u64;
    let lba_low_ext = regs[2] as u64;
    let lba_mid_ext = regs[4] as u64;
    let lba_high_ext = regs[6] as u64;

    lba_low
        | (lba_mid << 8)
        | (lba_high << 16)
        | (lba_low_ext << 24)
        | (lba_mid_ext << 32)
        | (lba_high_ext << 40)
}

/// Parse a 28-bit LBA from ATA descriptor sense register data.
fn parse_28bit_lba_from_regs(regs: &[u8; 14]) -> u64 {
    let lba_low = regs[3] as u64;
    let lba_mid = regs[5] as u64;
    let lba_high = regs[7] as u64;
    let device = regs[9] as u64;

    lba_low | (lba_mid << 8) | (lba_high << 16) | ((device & 0x0F) << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_current_max_lba_28bit() {
        let mut identify = [0u8; 512];
        // Set words 60-61 to 1000000 sectors (28-bit).
        // word 60 = 1000000 & 0xFFFF = 0x4240, word 61 = 1000000 >> 16 = 0x000F
        let sectors: u64 = 1_000_000;
        identify[120] = (sectors & 0xFF) as u8;
        identify[121] = ((sectors >> 8) & 0xFF) as u8;
        identify[122] = ((sectors >> 16) & 0xFF) as u8;
        identify[123] = ((sectors >> 24) & 0xFF) as u8;
        // word 83: bit 10 NOT set (no 48-bit support)
        identify[166] = 0;
        identify[167] = 0;

        assert_eq!(identify_current_max_lba(&identify), sectors);
    }

    #[test]
    fn test_identify_current_max_lba_48bit() {
        let mut identify = [0u8; 512];
        // Set word 83 bit 10 (48-bit LBA support).
        identify[166] = 0x00;
        identify[167] = 0x04; // bit 10

        // Set words 100-103 to a large 48-bit value.
        let sectors: u64 = 3_907_029_168; // ~2TB drive
        identify[200] = (sectors & 0xFF) as u8;
        identify[201] = ((sectors >> 8) & 0xFF) as u8;
        identify[202] = ((sectors >> 16) & 0xFF) as u8;
        identify[203] = ((sectors >> 24) & 0xFF) as u8;
        identify[204] = ((sectors >> 32) & 0xFF) as u8;
        identify[205] = ((sectors >> 40) & 0xFF) as u8;
        identify[206] = 0;
        identify[207] = 0;

        assert_eq!(identify_current_max_lba(&identify), sectors);
    }

    #[test]
    fn test_parse_28bit_lba() {
        let mut regs = [0u8; 14];
        regs[3] = 0x40; // lba_low
        regs[5] = 0x42; // lba_mid
        regs[7] = 0x0F; // lba_high
        regs[9] = 0x40; // device (LBA mode)
        assert_eq!(parse_28bit_lba_from_regs(&regs), 0x000F_4240);
    }
}
