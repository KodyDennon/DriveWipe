//! DCO (Device Configuration Overlay) detection and removal.
//!
//! The DCO allows drive manufacturers to hide capacity and disable features
//! (like 48-bit LBA, SMART, etc.) below even the HPA level. Unlike HPA,
//! DCO requires the DEVICE CONFIGURATION feature set (ATA command 0xB1).
//!
//! Commands:
//! - DEVICE CONFIGURATION IDENTIFY (0xB1, feature 0xC2): Read DCO data
//! - DEVICE CONFIGURATION RESTORE (0xB1, feature 0xC3): Restore factory settings
//! - DEVICE CONFIGURATION FREEZE LOCK (0xB1, feature 0xC5): Prevent further changes

use drivewipe_core::error::{DriveWipeError, Result};
use log;

use crate::kernel_module::{DwDcoInfo, KernelModule, set_device_path};

/// ATA command: DEVICE CONFIGURATION (0xB1).
const ATA_CMD_DEVICE_CONFIG: u8 = 0xB1;

/// Feature register values for DEVICE CONFIGURATION subcommands.
const DCO_IDENTIFY_FEATURE: u8 = 0xC2;
const DCO_RESTORE_FEATURE: u8 = 0xC3;
const DCO_FREEZE_FEATURE: u8 = 0xC5;

/// DCO detection result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DcoStatus {
    /// Device path.
    pub device: String,
    /// Whether DCO is present (factory max differs from current).
    pub dco_present: bool,
    /// Factory maximum LBA (true drive capacity).
    pub factory_max_lba: u64,
    /// Current maximum LBA (may be reduced by DCO).
    pub current_max_lba: u64,
    /// Number of sectors hidden by DCO.
    pub dco_hidden_sectors: u64,
    /// Size of DCO-hidden area in bytes.
    pub dco_hidden_bytes: u64,
    /// Features restricted by DCO (human-readable descriptions).
    pub restricted_features: Vec<String>,
    /// Raw 512-byte DCO IDENTIFY response data.
    pub raw_dco_data: Option<Vec<u8>>,
}

/// Detect DCO on a device. Tries kernel module first, then SG_IO fallback.
pub fn detect_dco(device_path: &str) -> Result<DcoStatus> {
    if let Ok(status) = detect_dco_kernel_module(device_path) {
        return Ok(status);
    }
    detect_dco_sg_io(device_path)
}

/// Restore DCO to factory settings (removes all DCO restrictions).
/// **WARNING**: This is irreversible unless the drive is power-cycled.
pub fn restore_dco(device_path: &str) -> Result<DcoStatus> {
    if let Ok(status) = restore_dco_kernel_module(device_path) {
        return Ok(status);
    }
    restore_dco_sg_io(device_path)
}

/// Freeze the DCO lock (prevents further DCO modifications until power cycle).
pub fn freeze_dco(device_path: &str) -> Result<()> {
    if let Ok(()) = freeze_dco_kernel_module(device_path) {
        return Ok(());
    }
    freeze_dco_sg_io(device_path)
}

// ── Kernel module path ───────────────────────────────────────────────────────

fn detect_dco_kernel_module(device_path: &str) -> Result<DcoStatus> {
    let km = KernelModule::open()?;
    let mut info = DwDcoInfo::default();
    set_device_path(&mut info.device, device_path);
    km.dco_detect(&mut info)?;

    let restricted_features = parse_dco_features(&info.dco_features);

    Ok(DcoStatus {
        device: device_path.to_string(),
        dco_present: info.dco_present != 0,
        factory_max_lba: info.dco_real_max_lba,
        current_max_lba: info.dco_current_max,
        dco_hidden_sectors: info.dco_real_max_lba.saturating_sub(info.dco_current_max),
        dco_hidden_bytes: info.dco_real_max_lba.saturating_sub(info.dco_current_max) * 512,
        restricted_features,
        raw_dco_data: Some(info.dco_features.to_vec()),
    })
}

fn restore_dco_kernel_module(device_path: &str) -> Result<DcoStatus> {
    let km = KernelModule::open()?;
    let mut info = DwDcoInfo::default();
    set_device_path(&mut info.device, device_path);
    km.dco_restore(&mut info)?;

    Ok(DcoStatus {
        device: device_path.to_string(),
        dco_present: false,
        factory_max_lba: info.dco_real_max_lba,
        current_max_lba: info.dco_real_max_lba, // After restore, current = factory
        dco_hidden_sectors: 0,
        dco_hidden_bytes: 0,
        restricted_features: vec![],
        raw_dco_data: None,
    })
}

fn freeze_dco_kernel_module(device_path: &str) -> Result<()> {
    let km = KernelModule::open()?;
    let mut info = DwDcoInfo::default();
    set_device_path(&mut info.device, device_path);
    km.dco_freeze(&mut info)
}

// ── SG_IO fallback ───────────────────────────────────────────────────────────

/// SG_IO header (same as hpa.rs — shared definition).
const SG_IO: u32 = 0x2285;
const ATA_16: u8 = 0x85;
const ATA_PROTO_PIO_DATA_IN: u8 = 4 << 1;
const ATA_PROTO_NON_DATA: u8 = 3 << 1;
const SG_DXFER_NONE: i32 = -1;
const SG_DXFER_FROM_DEV: i32 = -3;

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

fn detect_dco_sg_io(device_path: &str) -> Result<DcoStatus> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(device_path)
        .map_err(|e| DriveWipeError::DcoError(format!("Cannot open {device_path}: {e}")))?;
    let fd = file.as_raw_fd();

    // DCO IDENTIFY: ATA command 0xB1, feature 0xC2, 512 bytes PIO data-in.
    let mut sense = [0u8; 32];
    let mut data = [0u8; 512];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_PIO_DATA_IN;
    cdb[2] = 0x0E; // t_length=SECTOR_COUNT, t_dir=FROM_DEV, byte_block=BLOCKS
    cdb[4] = DCO_IDENTIFY_FEATURE; // Feature register
    cdb[6] = 1; // Sector count
    cdb[14] = ATA_CMD_DEVICE_CONFIG; // Command register (0xB1)

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
        return Err(DriveWipeError::DcoError(format!(
            "DCO IDENTIFY SG_IO failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    if hdr.status != 0 {
        return Err(DriveWipeError::DcoError(format!(
            "DCO IDENTIFY failed: status={}, host_status={}, driver_status={}",
            hdr.status, hdr.host_status, hdr.driver_status
        )));
    }

    // Parse DCO IDENTIFY data.
    // Words 1-3 (bytes 2-7): factory maximum LBA (48-bit).
    let factory_max = u16::from_le_bytes([data[2], data[3]]) as u64
        | ((u16::from_le_bytes([data[4], data[5]]) as u64) << 16)
        | ((u16::from_le_bytes([data[6], data[7]]) as u64) << 32);

    // Get current max from IDENTIFY DEVICE for comparison.
    let current_max = get_current_max_lba(fd)?;

    let dco_present = factory_max > current_max && factory_max > 0;
    let hidden_sectors = if dco_present {
        factory_max - current_max
    } else {
        0
    };
    let restricted_features = parse_dco_features(&data);

    log::info!(
        "DCO detection on {}: factory={}, current={}, dco={}",
        device_path,
        factory_max,
        current_max,
        if dco_present {
            format!("{} sectors hidden", hidden_sectors)
        } else {
            "none".to_string()
        }
    );

    Ok(DcoStatus {
        device: device_path.to_string(),
        dco_present,
        factory_max_lba: factory_max,
        current_max_lba: current_max,
        dco_hidden_sectors: hidden_sectors,
        dco_hidden_bytes: hidden_sectors * 512,
        restricted_features,
        raw_dco_data: Some(data.to_vec()),
    })
}

fn restore_dco_sg_io(device_path: &str) -> Result<DcoStatus> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path)
        .map_err(|e| DriveWipeError::DcoError(format!("Cannot open {device_path}: {e}")))?;
    let fd = file.as_raw_fd();

    // DCO RESTORE: ATA command 0xB1, feature 0xC3, non-data.
    let mut sense = [0u8; 32];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_NON_DATA;
    cdb[2] = 0x20; // CK_COND
    cdb[4] = DCO_RESTORE_FEATURE;
    cdb[14] = ATA_CMD_DEVICE_CONFIG;

    let mut hdr = SgIoHdr {
        interface_id: b'S' as i32,
        dxfer_direction: SG_DXFER_NONE,
        cmd_len: 16,
        mx_sb_len: sense.len() as u8,
        cmdp: cdb.as_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 30_000, // DCO RESTORE can take a while
        ..Default::default()
    };

    let ret = unsafe { libc::ioctl(fd, SG_IO as _, &mut hdr as *mut _) };
    if ret < 0 {
        return Err(DriveWipeError::HiddenAreaRemovalFailed {
            reason: format!(
                "DCO RESTORE SG_IO failed: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    if hdr.status != 0 {
        return Err(DriveWipeError::HiddenAreaRemovalFailed {
            reason: format!(
                "DCO RESTORE failed: status={}, host_status={}, driver_status={}",
                hdr.status, hdr.host_status, hdr.driver_status
            ),
        });
    }

    log::info!("DCO restored to factory settings on {}", device_path);

    // Re-detect to confirm.
    detect_dco_sg_io(device_path)
}

fn freeze_dco_sg_io(device_path: &str) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path)
        .map_err(|e| DriveWipeError::DcoError(format!("Cannot open {device_path}: {e}")))?;
    let fd = file.as_raw_fd();

    let mut sense = [0u8; 32];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_NON_DATA;
    cdb[2] = 0x20;
    cdb[4] = DCO_FREEZE_FEATURE;
    cdb[14] = ATA_CMD_DEVICE_CONFIG;

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
        return Err(DriveWipeError::DcoFrozen);
    }

    log::info!("DCO frozen on {}", device_path);
    Ok(())
}

/// Get current max LBA from IDENTIFY DEVICE.
fn get_current_max_lba(fd: std::os::unix::io::RawFd) -> Result<u64> {
    let mut sense = [0u8; 32];
    let mut data = [0u8; 512];
    let mut cdb = [0u8; 16];
    cdb[0] = ATA_16;
    cdb[1] = ATA_PROTO_PIO_DATA_IN;
    cdb[2] = 0x0E;
    cdb[6] = 1;
    cdb[14] = 0xEC; // IDENTIFY DEVICE

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
        return Err(DriveWipeError::DcoError(format!(
            "IDENTIFY DEVICE failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    // Check 48-bit LBA support (word 83 bit 10).
    let word83 = u16::from_le_bytes([data[166], data[167]]);
    if (word83 & (1 << 10)) != 0 {
        let w100 = u16::from_le_bytes([data[200], data[201]]) as u64;
        let w101 = u16::from_le_bytes([data[202], data[203]]) as u64;
        let w102 = u16::from_le_bytes([data[204], data[205]]) as u64;
        let w103 = u16::from_le_bytes([data[206], data[207]]) as u64;
        Ok(w100 | (w101 << 16) | (w102 << 32) | (w103 << 48))
    } else {
        let w60 = u16::from_le_bytes([data[120], data[121]]) as u64;
        let w61 = u16::from_le_bytes([data[122], data[123]]) as u64;
        Ok(w60 | (w61 << 16))
    }
}

/// Parse DCO IDENTIFY feature restriction bits into human-readable strings.
fn parse_dco_features(data: &[u8]) -> Vec<String> {
    let mut features = Vec::new();

    if data.len() < 16 {
        return features;
    }

    // Word 2 (bytes 4-5): Feature disable bits per DCO spec.
    let word2 = if data.len() >= 6 {
        u16::from_le_bytes([data[4], data[5]])
    } else {
        return features;
    };

    if word2 & (1 << 0) != 0 {
        features.push("SMART disabled".to_string());
    }
    if word2 & (1 << 1) != 0 {
        features.push("Security disabled".to_string());
    }
    if word2 & (1 << 2) != 0 {
        features.push("Write cache disabled".to_string());
    }
    if word2 & (1 << 3) != 0 {
        features.push("Read look-ahead disabled".to_string());
    }
    if word2 & (1 << 4) != 0 {
        features.push("48-bit LBA disabled".to_string());
    }
    if word2 & (1 << 5) != 0 {
        features.push("AAM disabled".to_string());
    }
    if word2 & (1 << 6) != 0 {
        features.push("TCQ disabled".to_string());
    }
    if word2 & (1 << 7) != 0 {
        features.push("SATA features disabled".to_string());
    }
    if word2 & (1 << 8) != 0 {
        features.push("HPA feature disabled".to_string());
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dco_features_empty() {
        let data = [0u8; 512];
        let features = parse_dco_features(&data);
        assert!(features.is_empty());
    }

    #[test]
    fn test_parse_dco_features_smart_disabled() {
        let mut data = [0u8; 512];
        data[4] = 0x01; // word 2, bit 0 = SMART disabled
        let features = parse_dco_features(&data);
        assert_eq!(features, vec!["SMART disabled"]);
    }

    #[test]
    fn test_parse_dco_features_multiple() {
        let mut data = [0u8; 512];
        // SMART + 48-bit LBA disabled
        data[4] = 0x11; // bits 0 and 4
        let features = parse_dco_features(&data);
        assert_eq!(features.len(), 2);
        assert!(features.contains(&"SMART disabled".to_string()));
        assert!(features.contains(&"48-bit LBA disabled".to_string()));
    }
}
