//! ATA security state query.
//!
//! Reads ATA security state from IDENTIFY DEVICE to determine whether a drive
//! is frozen, locked, enabled, etc. This information is critical for determining
//! whether ATA Secure Erase can be performed and whether an unfreeze cycle
//! is needed.
//!
//! IDENTIFY DEVICE security-related words:
//! - Word 82: Command set supported (bit 1 = Security feature set)
//! - Word 85: Command set enabled (bit 1 = Security enabled)
//! - Word 89: Time required for Normal Erase (minutes)
//! - Word 90: Time required for Enhanced Erase (minutes)
//! - Word 128: Security status
//!   - Bit 0: Security supported
//!   - Bit 1: Security enabled
//!   - Bit 2: Security locked
//!   - Bit 3: Security frozen
//!   - Bit 4: Security count expired
//!   - Bit 5: Enhanced erase supported

use drivewipe_core::error::{DriveWipeError, Result};
use log;

use crate::kernel_module::{DwAtaSecurityState, KernelModule, set_device_path};

/// Detailed ATA security state for a drive.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AtaSecurityInfo {
    /// Device path.
    pub device: String,
    /// Whether the security feature set is supported.
    pub supported: bool,
    /// Whether security is currently enabled (password set).
    pub enabled: bool,
    /// Whether the drive is locked (data inaccessible).
    pub locked: bool,
    /// Whether the drive is frozen (security commands rejected).
    pub frozen: bool,
    /// Whether the security erase attempt count has expired.
    pub count_expired: bool,
    /// Whether enhanced erase is supported.
    pub enhanced_erase_supported: bool,
    /// Estimated time for normal erase in minutes (0 = not reported).
    pub erase_time_normal_min: u16,
    /// Estimated time for enhanced erase in minutes (0 = not reported).
    pub erase_time_enhanced_min: u16,
    /// Human-readable summary of the security state.
    pub summary: String,
}

impl AtaSecurityInfo {
    /// Whether this drive can be ATA secure erased right now.
    pub fn can_erase(&self) -> bool {
        self.supported && !self.frozen && !self.locked && !self.count_expired
    }

    /// Whether this drive needs an unfreeze cycle before erase.
    pub fn needs_unfreeze(&self) -> bool {
        self.supported && self.frozen
    }

    /// Convert to the core crate's `AtaSecurityState` enum.
    pub fn to_core_state(&self) -> drivewipe_core::types::AtaSecurityState {
        use drivewipe_core::types::AtaSecurityState;
        if !self.supported {
            AtaSecurityState::NotSupported
        } else if self.count_expired {
            AtaSecurityState::CountExpired
        } else if self.locked {
            AtaSecurityState::Locked
        } else if self.frozen {
            AtaSecurityState::Frozen
        } else if self.enabled {
            AtaSecurityState::Enabled
        } else {
            AtaSecurityState::Disabled
        }
    }
}

/// Query ATA security state for a device.
/// Tries kernel module first, then SG_IO IDENTIFY DEVICE fallback.
pub fn query_ata_security(device_path: &str) -> Result<AtaSecurityInfo> {
    if let Ok(info) = query_via_kernel_module(device_path) {
        return Ok(info);
    }
    query_via_sg_io(device_path)
}

// ── Kernel module path ───────────────────────────────────────────────────────

fn query_via_kernel_module(device_path: &str) -> Result<AtaSecurityInfo> {
    let km = KernelModule::open()?;
    let mut state = DwAtaSecurityState::default();
    set_device_path(&mut state.device, device_path);
    km.ata_security_state(&mut state)?;

    let info = AtaSecurityInfo {
        device: device_path.to_string(),
        supported: state.supported != 0,
        enabled: state.enabled != 0,
        locked: state.locked != 0,
        frozen: state.frozen != 0,
        count_expired: state.count_expired != 0,
        enhanced_erase_supported: state.enhanced_erase_supported != 0,
        erase_time_normal_min: state.erase_time_normal,
        erase_time_enhanced_min: state.erase_time_enhanced,
        summary: String::new(),
    };

    Ok(AtaSecurityInfo {
        summary: build_summary(&info),
        ..info
    })
}

// ── SG_IO fallback ───────────────────────────────────────────────────────────

const SG_IO: u32 = 0x2285;
const ATA_16: u8 = 0x85;
const ATA_PROTO_PIO_DATA_IN: u8 = 4 << 1;
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

fn query_via_sg_io(device_path: &str) -> Result<AtaSecurityInfo> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(device_path)
        .map_err(|e| {
            DriveWipeError::DeviceError(format!(
                "Cannot open {device_path} for ATA security query: {e}"
            ))
        })?;
    let fd = file.as_raw_fd();

    // IDENTIFY DEVICE
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
        return Err(DriveWipeError::Ioctl {
            operation: "IDENTIFY DEVICE for ATA security".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }
    if hdr.status != 0 {
        return Err(DriveWipeError::DeviceError(format!(
            "IDENTIFY DEVICE failed: status={}, host_status={}, driver_status={}",
            hdr.status, hdr.host_status, hdr.driver_status
        )));
    }

    let info = parse_identify_security(&data, device_path);
    log::info!("ATA security on {}: {}", device_path, info.summary);
    Ok(info)
}

/// Parse IDENTIFY DEVICE data for security information.
pub fn parse_identify_security(identify: &[u8; 512], device_path: &str) -> AtaSecurityInfo {
    // Word 82 (bytes 164-165): Command set supported.
    let word82 = u16::from_le_bytes([identify[164], identify[165]]);
    let security_feature_supported = (word82 & (1 << 1)) != 0;

    // Word 128 (bytes 256-257): Security status.
    let word128 = u16::from_le_bytes([identify[256], identify[257]]);
    let supported = (word128 & (1 << 0)) != 0 && security_feature_supported;
    let enabled = (word128 & (1 << 1)) != 0;
    let locked = (word128 & (1 << 2)) != 0;
    let frozen = (word128 & (1 << 3)) != 0;
    let count_expired = (word128 & (1 << 4)) != 0;
    let enhanced_erase_supported = (word128 & (1 << 5)) != 0;

    // Word 89 (bytes 178-179): Normal erase time.
    let erase_time_normal_min = u16::from_le_bytes([identify[178], identify[179]]);

    // Word 90 (bytes 180-181): Enhanced erase time.
    let erase_time_enhanced_min = u16::from_le_bytes([identify[180], identify[181]]);

    let info = AtaSecurityInfo {
        device: device_path.to_string(),
        supported,
        enabled,
        locked,
        frozen,
        count_expired,
        enhanced_erase_supported,
        erase_time_normal_min,
        erase_time_enhanced_min,
        summary: String::new(),
    };

    AtaSecurityInfo {
        summary: build_summary(&info),
        ..info
    }
}

/// Build a human-readable summary of the security state.
fn build_summary(info: &AtaSecurityInfo) -> String {
    if !info.supported {
        return "Security: Not Supported".to_string();
    }

    let mut parts = vec!["Security: Supported".to_string()];

    if info.enabled {
        parts.push("ENABLED".to_string());
    }
    if info.locked {
        parts.push("LOCKED".to_string());
    }
    if info.frozen {
        parts.push("FROZEN".to_string());
    }
    if info.count_expired {
        parts.push("COUNT EXPIRED".to_string());
    }
    if info.enhanced_erase_supported {
        parts.push("Enhanced Erase OK".to_string());
    }

    if info.erase_time_normal_min > 0 {
        parts.push(format!("Normal erase: ~{}min", info.erase_time_normal_min));
    }
    if info.erase_time_enhanced_min > 0 {
        parts.push(format!(
            "Enhanced erase: ~{}min",
            info.erase_time_enhanced_min
        ));
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_security_not_supported() {
        let data = [0u8; 512];
        let info = parse_identify_security(&data, "/dev/sda");
        assert!(!info.supported);
        assert!(!info.enabled);
        assert!(!info.frozen);
        assert!(info.summary.contains("Not Supported"));
    }

    #[test]
    fn test_parse_security_frozen() {
        let mut data = [0u8; 512];
        // Word 82 bit 1: security feature set supported
        data[164] = 0x02;
        // Word 128: supported=1, frozen=1
        data[256] = 0x09; // bits 0 and 3
        data[257] = 0x00;

        let info = parse_identify_security(&data, "/dev/sda");
        assert!(info.supported);
        assert!(!info.enabled);
        assert!(info.frozen);
        assert!(info.needs_unfreeze());
        assert!(!info.can_erase());
        assert!(info.summary.contains("FROZEN"));
    }

    #[test]
    fn test_parse_security_ready_for_erase() {
        let mut data = [0u8; 512];
        // Word 82 bit 1: security feature set supported
        data[164] = 0x02;
        // Word 128: supported=1, enhanced_erase=1
        data[256] = 0x21; // bits 0 and 5
        data[257] = 0x00;
        // Word 89: normal erase time = 30 minutes
        data[178] = 30;
        data[179] = 0;

        let info = parse_identify_security(&data, "/dev/sda");
        assert!(info.supported);
        assert!(info.can_erase());
        assert!(!info.needs_unfreeze());
        assert!(info.enhanced_erase_supported);
        assert_eq!(info.erase_time_normal_min, 30);
    }

    #[test]
    fn test_to_core_state() {
        let mut data = [0u8; 512];
        data[164] = 0x02;
        data[256] = 0x09; // supported + frozen

        let info = parse_identify_security(&data, "/dev/sda");
        assert_eq!(
            info.to_core_state(),
            drivewipe_core::types::AtaSecurityState::Frozen
        );
    }
}
