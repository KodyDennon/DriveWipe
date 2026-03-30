use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How the drive is physically connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Sata,
    Nvme,
    Usb,
    Scsi,
    Sas,
    Unknown,
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sata => write!(f, "SATA"),
            Self::Nvme => write!(f, "NVMe"),
            Self::Usb => write!(f, "USB"),
            Self::Scsi => write!(f, "SCSI"),
            Self::Sas => write!(f, "SAS"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Type of storage medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveType {
    Hdd,
    Ssd,
    Nvme,
    Unknown,
}

impl fmt::Display for DriveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hdd => write!(f, "HDD"),
            Self::Ssd => write!(f, "SSD"),
            Self::Nvme => write!(f, "NVMe"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// ATA security state of a drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtaSecurityState {
    NotSupported,
    Disabled,
    Enabled,
    Locked,
    Frozen,
    CountExpired,
}

impl fmt::Display for AtaSecurityState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported => write!(f, "Not Supported"),
            Self::Disabled => write!(f, "Disabled"),
            Self::Enabled => write!(f, "Enabled"),
            Self::Locked => write!(f, "Locked"),
            Self::Frozen => write!(f, "Frozen"),
            Self::CountExpired => write!(f, "Count Expired"),
        }
    }
}

/// Information about hidden areas on a drive (HPA/DCO).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HiddenAreaInfo {
    /// Whether Host Protected Area is enabled.
    pub hpa_enabled: bool,
    /// Size of the HPA in bytes, if detected.
    pub hpa_size: Option<u64>,
    /// Native (true hardware) max LBA reported by READ NATIVE MAX ADDRESS.
    pub hpa_native_max_lba: Option<u64>,
    /// Current max LBA as reported by IDENTIFY DEVICE.
    pub hpa_current_max_lba: Option<u64>,
    /// Whether Device Configuration Overlay is enabled.
    pub dco_enabled: bool,
    /// Size of the DCO in bytes, if detected.
    pub dco_size: Option<u64>,
    /// Factory maximum LBA (true capacity before DCO restrictions).
    pub dco_factory_max_lba: Option<u64>,
    /// Features restricted by DCO (e.g., "SMART disabled", "48-bit LBA disabled").
    #[serde(default)]
    pub dco_features_restricted: Vec<String>,
}

/// Comprehensive information about a detected drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    /// OS device path (e.g., /dev/sda, /dev/rdisk2, \\.\PhysicalDrive0).
    pub path: PathBuf,
    /// Drive model name.
    pub model: String,
    /// Drive serial number.
    pub serial: String,
    /// Drive firmware revision.
    pub firmware_rev: String,
    /// Total capacity in bytes.
    pub capacity: u64,
    /// Logical block size in bytes.
    pub block_size: u32,
    /// Physical block size in bytes, if different from logical.
    pub physical_block_size: Option<u32>,
    /// Storage medium type.
    pub drive_type: DriveType,
    /// Connection interface.
    pub transport: Transport,
    /// Whether this is the boot/system drive.
    pub is_boot_drive: bool,
    /// Whether the drive is removable (USB, etc.).
    pub is_removable: bool,
    /// ATA security state (for ATA/SATA drives).
    pub ata_security: AtaSecurityState,
    /// Hidden area information.
    pub hidden_areas: HiddenAreaInfo,
    /// Whether the drive supports TRIM/UNMAP.
    pub supports_trim: bool,
    /// Whether the drive is a self-encrypting drive (SED).
    pub is_sed: bool,
    /// SMART health status (if available).
    pub smart_healthy: Option<bool>,
    /// Partition table type (e.g., "gpt", "mbr").
    pub partition_table: Option<String>,
    /// Number of partitions.
    pub partition_count: u32,
}

impl DriveInfo {
    /// Human-readable capacity string (e.g., "500 GB", "2 TB").
    pub fn capacity_display(&self) -> String {
        format_bytes(self.capacity)
    }

    /// Whether firmware erase commands are likely to work on this drive.
    pub fn firmware_erase_likely_supported(&self) -> bool {
        match self.transport {
            Transport::Usb => false,
            Transport::Sata => self.ata_security != AtaSecurityState::NotSupported,
            Transport::Nvme => true,
            _ => false,
        }
    }

    /// Suggest the best wipe method for this drive.
    pub fn suggested_method(&self) -> &'static str {
        match (self.drive_type, self.transport) {
            (DriveType::Nvme, _) | (_, Transport::Nvme) => "nvme-format-crypto",
            (DriveType::Ssd, Transport::Sata) => "ata-erase-enhanced",
            (DriveType::Ssd, Transport::Usb) => "dod-short",
            (DriveType::Hdd, _) => "dod-short",
            _ => "random",
        }
    }
}

impl fmt::Display for DriveInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} [{}] ({})",
            self.path.display(),
            self.model,
            self.serial,
            self.capacity_display(),
            self.drive_type,
        )
    }
}

/// Outcome of a single wipe pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassResult {
    pub pass_number: u32,
    pub pattern_name: String,
    pub bytes_written: u64,
    pub duration_secs: f64,
    pub throughput_mbps: f64,
    pub verified: bool,
    pub verification_passed: Option<bool>,
}

/// Overall result of a wipe operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WipeOutcome {
    Success,
    SuccessWithWarnings,
    Failed,
    Cancelled,
    Interrupted,
}

impl fmt::Display for WipeOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::SuccessWithWarnings => write!(f, "Success (with warnings)"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Interrupted => write!(f, "Interrupted"),
        }
    }
}

/// Complete result of a wipe session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeResult {
    pub session_id: uuid::Uuid,
    pub device_path: PathBuf,
    pub device_serial: String,
    pub device_model: String,
    pub device_capacity: u64,
    pub method_id: String,
    pub method_name: String,
    pub outcome: WipeOutcome,
    pub passes: Vec<PassResult>,
    pub total_bytes_written: u64,
    pub total_duration_secs: f64,
    pub average_throughput_mbps: f64,
    pub verification_passed: Option<bool>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub hostname: String,
    pub operator: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Format a byte count into a human-readable string using decimal (SI) units
/// to match `parse_capacity`.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_000;
    const MB: u64 = 1_000_000;
    const GB: u64 = 1_000_000_000;
    const TB: u64 = 1_000_000_000_000;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Format a throughput value in MB/s (decimal SI units).
pub fn format_throughput(bytes_per_sec: f64) -> String {
    let mb = bytes_per_sec / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB/s", mb / 1000.0)
    } else {
        format!("{mb:.1} MB/s")
    }
}
