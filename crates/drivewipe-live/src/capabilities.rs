//! Live environment capability detection and feature gating.
//!
//! Probes the system to determine which live environment features are actually
//! available. Features are gated based on:
//! - Kernel module presence and reported capabilities
//! - System power management support (for unfreeze)
//! - Available block devices and their interfaces
//! - Network boot status

use std::fs;
use std::path::Path;

use log;

use crate::detect::{LiveDetection, detect_live_environment};
use crate::kernel_module::{self, KernelModule};

/// Comprehensive capabilities of the live environment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveCapabilities {
    /// Live environment detection results.
    pub detection: LiveDetection,

    /// Kernel module capabilities.
    pub kernel_module: KernelModuleCapabilities,

    /// System capabilities.
    pub system: SystemCapabilities,

    /// Hardware capabilities.
    pub hardware: HardwareCapabilities,
}

/// Capabilities exposed by the DriveWipe kernel module.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KernelModuleCapabilities {
    /// Whether the kernel module is loaded and accessible.
    pub loaded: bool,
    /// Module version string (e.g., "1.0.0").
    pub version: Option<String>,
    /// ATA passthrough available.
    pub ata_passthrough: bool,
    /// NVMe passthrough available.
    pub nvme_passthrough: bool,
    /// HPA detect/remove available.
    pub hpa_support: bool,
    /// DCO detect/restore/freeze available.
    pub dco_support: bool,
    /// DMA-coherent I/O available.
    pub dma_io: bool,
    /// ATA security state query available.
    pub ata_security: bool,
    /// Raw capabilities bitmask from module.
    pub raw_capabilities: u32,
}

/// System-level capabilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemCapabilities {
    /// System supports S3 suspend (required for unfreeze).
    pub suspend_supported: bool,
    /// Network interfaces available (for PXE reporting).
    pub network_interfaces: Vec<String>,
    /// Whether booted via PXE.
    pub pxe_booted: bool,
    /// Total system RAM in bytes.
    pub total_ram: u64,
    /// Number of CPU cores.
    pub cpu_cores: u32,
    /// Kernel version string.
    pub kernel_version: String,
}

/// Hardware-level capabilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HardwareCapabilities {
    /// Number of SATA drives detected.
    pub sata_drives: u32,
    /// Number of NVMe drives detected.
    pub nvme_drives: u32,
    /// Number of USB drives detected.
    pub usb_drives: u32,
    /// Whether any drives are currently frozen.
    pub any_frozen: bool,
    /// Whether SG_IO is available (for fallback operations).
    pub sg_io_available: bool,
}

impl LiveCapabilities {
    /// Probe the system and build a complete capabilities report.
    pub fn probe() -> Self {
        let detection = detect_live_environment();
        let kernel_module = probe_kernel_module();
        let system = probe_system(&detection);
        let hardware = probe_hardware();

        log::info!(
            "Live capabilities probed: module={}, suspend={}, sata={}, nvme={}",
            kernel_module.loaded,
            system.suspend_supported,
            hardware.sata_drives,
            hardware.nvme_drives
        );

        Self {
            detection,
            kernel_module,
            system,
            hardware,
        }
    }

    /// Whether the full live environment is available.
    pub fn is_fully_live(&self) -> bool {
        self.detection.is_live && self.kernel_module.loaded
    }

    /// Whether HPA operations are available (kernel module or SG_IO).
    pub fn can_detect_hpa(&self) -> bool {
        self.kernel_module.hpa_support || self.hardware.sg_io_available
    }

    /// Whether DCO operations are available (kernel module or SG_IO).
    pub fn can_detect_dco(&self) -> bool {
        self.kernel_module.dco_support || self.hardware.sg_io_available
    }

    /// Whether drive unfreeze is possible.
    pub fn can_unfreeze(&self) -> bool {
        self.system.suspend_supported && self.hardware.any_frozen
    }

    /// Whether DMA I/O is available (requires kernel module).
    pub fn can_dma_io(&self) -> bool {
        self.kernel_module.dma_io
    }
}

fn probe_kernel_module() -> KernelModuleCapabilities {
    let km = match KernelModule::open() {
        Ok(km) => km,
        Err(_) => {
            return KernelModuleCapabilities {
                loaded: false,
                version: None,
                ata_passthrough: false,
                nvme_passthrough: false,
                hpa_support: false,
                dco_support: false,
                dma_io: false,
                ata_security: false,
                raw_capabilities: 0,
            };
        }
    };

    match km.module_info() {
        Ok(info) => {
            let caps = info.capabilities;
            KernelModuleCapabilities {
                loaded: true,
                version: Some(format!(
                    "{}.{}.{}",
                    info.version_major, info.version_minor, info.version_patch
                )),
                ata_passthrough: (caps & kernel_module::DW_CAP_ATA) != 0,
                nvme_passthrough: (caps & kernel_module::DW_CAP_NVME) != 0,
                hpa_support: (caps & kernel_module::DW_CAP_HPA) != 0,
                dco_support: (caps & kernel_module::DW_CAP_DCO) != 0,
                dma_io: (caps & kernel_module::DW_CAP_DMA) != 0,
                ata_security: (caps & kernel_module::DW_CAP_ATA_SECURITY) != 0,
                raw_capabilities: caps,
            }
        }
        Err(_) => KernelModuleCapabilities {
            loaded: true,
            version: None,
            ata_passthrough: false,
            nvme_passthrough: false,
            hpa_support: false,
            dco_support: false,
            dma_io: false,
            ata_security: false,
            raw_capabilities: 0,
        },
    }
}

fn probe_system(detection: &LiveDetection) -> SystemCapabilities {
    let suspend_supported = fs::read_to_string("/sys/power/state")
        .map(|s| s.contains("mem"))
        .unwrap_or(false);

    let network_interfaces = list_network_interfaces();

    let total_ram = read_meminfo_total();
    let cpu_cores = read_cpu_count();
    let kernel_version = fs::read_to_string("/proc/version")
        .unwrap_or_default()
        .split_whitespace()
        .nth(2)
        .unwrap_or("unknown")
        .to_string();

    SystemCapabilities {
        suspend_supported,
        network_interfaces,
        pxe_booted: detection.pxe_booted,
        total_ram,
        cpu_cores,
        kernel_version,
    }
}

fn probe_hardware() -> HardwareCapabilities {
    let mut sata_drives = 0u32;
    let mut nvme_drives = 0u32;
    let mut usb_drives = 0u32;

    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("nvme") && !name_str.contains('p') {
                nvme_drives += 1;
            } else if name_str.starts_with("sd") {
                // Check if USB.
                let subsystem = format!("/sys/block/{name_str}/device/subsystem");
                if let Ok(target) = fs::read_link(&subsystem) {
                    let target_str = target.to_string_lossy();
                    if target_str.contains("usb") {
                        usb_drives += 1;
                    } else {
                        sata_drives += 1;
                    }
                } else {
                    sata_drives += 1;
                }
            }
        }
    }

    let sg_io_available = Path::new("/dev/sg0").exists() || sata_drives > 0; // SG_IO works on /dev/sd* too

    HardwareCapabilities {
        sata_drives,
        nvme_drives,
        usb_drives,
        any_frozen: false, // Will be checked lazily to avoid slow IDENTIFY on every probe
        sg_io_available,
    }
}

fn list_network_interfaces() -> Vec<String> {
    let mut interfaces = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "lo" {
                interfaces.push(name);
            }
        }
    }
    interfaces
}

fn read_meminfo_total() -> u64 {
    if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
        for line in contents.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb * 1024; // Convert from kB to bytes
                    }
                }
            }
        }
    }
    0
}

fn read_cpu_count() -> u32 {
    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        return contents
            .lines()
            .filter(|l| l.starts_with("processor"))
            .count() as u32;
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_doesnt_panic() {
        // Probing should work on any system without panicking.
        let caps = LiveCapabilities::probe();
        // On a dev machine, we shouldn't be in a live environment.
        // But the important thing is no panic.
        let _ = caps.is_fully_live();
        let _ = caps.can_detect_hpa();
        let _ = caps.can_detect_dco();
    }
}
