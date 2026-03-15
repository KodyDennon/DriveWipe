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

        let disk_signature = u32::from_le_bytes(
            data[440..444]
                .try_into()
                .map_err(|_| DriveWipeError::InvalidPartitionTable("Malformed MBR".to_string()))?,
        );

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

            let start_lba = u32::from_le_bytes(entry[8..12].try_into().unwrap_or([0; 4])) as u64;
            let size_sectors =
                u32::from_le_bytes(entry[12..16].try_into().unwrap_or([0; 4])) as u64;

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

    /// Serialize the MBR table to 512 bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; 512];

        // 1. Boot code (0-440) - filled with zeros for now, or could preserve existing if read
        // For a wipe tool, zeroing boot code is safer unless we explicitly want to preserve it.
        // We'll leave it as zeros.

        // 2. Disk signature (440-444)
        buffer[440..444].copy_from_slice(&self.disk_signature.to_le_bytes());

        // 3. Nulls (444-446)
        buffer[444] = 0;
        buffer[445] = 0;

        // 4. Partition entries (446-510)
        // We have up to 4 partitions.
        for (i, part) in self.partitions.iter().enumerate() {
            if i >= 4 {
                break;
            }
            let offset = 446 + (i * 16);
            let entry = &mut buffer[offset..offset + 16];

            // Status (0x80 = bootable, 0x00 = inactive)
            entry[0] = if part.bootable { 0x80 } else { 0x00 };

            // CHS Start (1-4) - We'll use 0xFFFFFF for LBA addressing if possible,
            // or calculate valid CHS if < 8GB. For modern drives, LBA is king.
            // A simple approach is to max out CHS to indicate LBA usage.
            entry[1] = 0xFE;
            entry[2] = 0xFF;
            entry[3] = 0xFF;

            // Partition Type (4)
            // Parse from hex string "0x83" or name
            let type_byte =
                u8::from_str_radix(part.type_id.trim_start_matches("0x"), 16).unwrap_or(0x83);
            entry[4] = type_byte;

            // CHS End (5-8)
            entry[5] = 0xFE;
            entry[6] = 0xFF;
            entry[7] = 0xFF;

            // LBA Start (8-12)
            let start_lba = part.start_lba as u32; // MBR uses 32-bit LBA
            entry[8..12].copy_from_slice(&start_lba.to_le_bytes());

            // Number of Sectors (12-16)
            let sectors = (part.end_lba - part.start_lba + 1) as u32;
            entry[12..16].copy_from_slice(&sectors.to_le_bytes());
        }

        // 5. Boot signature (510-512)
        buffer[510] = 0x55;
        buffer[511] = 0xAA;

        buffer
    }
    /// Write the MBR table to the device (LBA 0).
    pub fn write(&self, device: &mut dyn crate::io::RawDeviceIo) -> Result<()> {
        let bytes = self.to_bytes();
        device.write_at(0, &bytes)?;
        device.sync()?;
        Ok(())
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
