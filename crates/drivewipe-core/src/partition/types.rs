use serde::{Deserialize, Serialize};

/// Type of partition table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartitionTableType {
    Gpt,
    Mbr,
    Hybrid,
}

/// A single partition entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    /// Partition index (0-based).
    pub index: u32,
    /// Partition name (GPT only).
    pub name: String,
    /// Type GUID (GPT) or type byte (MBR).
    pub type_id: String,
    /// Unique partition GUID (GPT only).
    pub unique_id: Option<String>,
    /// Start LBA.
    pub start_lba: u64,
    /// End LBA (inclusive).
    pub end_lba: u64,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Attribute flags.
    pub attributes: u64,
    /// Whether this is a bootable partition (MBR active flag).
    pub bootable: bool,
}

impl Partition {
    /// Size in sectors (assuming 512-byte sectors).
    pub fn size_sectors(&self) -> u64 {
        self.end_lba - self.start_lba + 1
    }
}

/// Well-known GPT partition type GUIDs.
pub mod gpt_types {
    pub const EFI_SYSTEM: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
    pub const MICROSOFT_BASIC_DATA: &str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";
    pub const LINUX_FILESYSTEM: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";
    pub const LINUX_SWAP: &str = "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F";
    pub const APPLE_HFS_PLUS: &str = "48465300-0000-11AA-AA11-00306543ECAC";
    pub const APPLE_APFS: &str = "7C3457EF-0000-11AA-AA11-00306543ECAC";
    pub const MICROSOFT_RESERVED: &str = "E3C9E316-0B5C-4DB8-817D-F92DF00215AE";
}

/// Alignment helpers.
pub fn align_to_1mib(lba: u64, sector_size: u32) -> u64 {
    let sectors_per_mib = 1024 * 1024 / sector_size as u64;
    ((lba + sectors_per_mib - 1) / sectors_per_mib) * sectors_per_mib
}

pub fn align_to_4k(lba: u64, sector_size: u32) -> u64 {
    let sectors_per_4k = 4096 / sector_size as u64;
    ((lba + sectors_per_4k - 1) / sectors_per_4k) * sectors_per_4k
}
