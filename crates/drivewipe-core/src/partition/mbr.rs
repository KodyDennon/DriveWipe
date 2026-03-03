use serde::{Deserialize, Serialize};

use super::types::Partition;
use crate::error::{DriveWipeError, Result};

/// MBR signature at offset 510-511.
const MBR_SIGNATURE: [u8; 2] = [0x55, 0xAA];

/// Parsed MBR partition table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MbrTable {
    /// Disk signature (bytes 440-443).
    pub disk_signature: u32,
    /// Primary partitions (up to 4).
    pub partitions: Vec<Partition>,
}

impl MbrTable {
    /// Parse an MBR table from the first 512 bytes of a device.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 512 {
            return Err(DriveWipeError::InvalidPartitionTable(
                "Not enough data for MBR".to_string(),
            ));
        }

        // Check signature
        if data[510] != MBR_SIGNATURE[0] || data[511] != MBR_SIGNATURE[1] {
            return Err(DriveWipeError::InvalidPartitionTable(
                "MBR signature not found".to_string(),
            ));
        }

        let disk_signature = u32::from_le_bytes(data[440..444].try_into().unwrap());

        let mut partitions = Vec::new();

        // 4 partition entries starting at offset 446, each 16 bytes
        for i in 0..4u32 {
            let offset = 446 + (i as usize * 16);
            let entry = &data[offset..offset + 16];

            let status = entry[0];
            let partition_type = entry[4];

            // Skip empty entries
            if partition_type == 0x00 {
                continue;
            }

            let start_lba = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
            let size_sectors = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as u64;

            if size_sectors == 0 {
                continue;
            }

            let end_lba = start_lba + size_sectors - 1;

            partitions.push(Partition {
                index: i,
                name: format!("Partition {}", i + 1),
                type_id: format!("{partition_type:#04X}"),
                unique_id: None,
                start_lba,
                end_lba,
                size_bytes: size_sectors * 512,
                attributes: 0,
                bootable: status == 0x80,
            });
        }

        Ok(MbrTable {
            disk_signature,
            partitions,
        })
    }
}

/// Well-known MBR partition type bytes.
pub mod mbr_types {
    pub const EMPTY: u8 = 0x00;
    pub const FAT12: u8 = 0x01;
    pub const FAT16_SMALL: u8 = 0x04;
    pub const EXTENDED: u8 = 0x05;
    pub const FAT16_LARGE: u8 = 0x06;
    pub const NTFS: u8 = 0x07;
    pub const FAT32: u8 = 0x0B;
    pub const FAT32_LBA: u8 = 0x0C;
    pub const LINUX: u8 = 0x83;
    pub const LINUX_SWAP: u8 = 0x82;
    pub const LINUX_LVM: u8 = 0x8E;
    pub const GPT_PROTECTIVE: u8 = 0xEE;
    pub const EFI_SYSTEM: u8 = 0xEF;
}
