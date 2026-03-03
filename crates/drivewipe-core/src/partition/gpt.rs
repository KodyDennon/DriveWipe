use serde::{Deserialize, Serialize};

use super::types::Partition;
use crate::error::{DriveWipeError, Result};

/// GPT signature at LBA 1 offset 0.
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

/// Parsed GPT partition table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptTable {
    /// Disk GUID.
    pub disk_guid: String,
    /// First usable LBA.
    pub first_usable_lba: u64,
    /// Last usable LBA.
    pub last_usable_lba: u64,
    /// Number of partition entries.
    pub entry_count: u32,
    /// Size of each partition entry.
    pub entry_size: u32,
    /// Parsed partitions.
    pub partitions: Vec<Partition>,
}

impl GptTable {
    /// Parse a GPT table from raw disk data (must include LBA 0-33 at minimum).
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 1024 {
            return Err(DriveWipeError::InvalidPartitionTable(
                "Not enough data for GPT header".to_string(),
            ));
        }

        // GPT header is at LBA 1 (offset 512)
        let header = &data[512..];

        // Check signature
        if &header[0..8] != GPT_SIGNATURE {
            return Err(DriveWipeError::InvalidPartitionTable(
                "GPT signature not found".to_string(),
            ));
        }

        let first_usable_lba = u64::from_le_bytes(header[40..48].try_into().unwrap());
        let last_usable_lba = u64::from_le_bytes(header[48..56].try_into().unwrap());

        let mut disk_guid_bytes = [0u8; 16];
        disk_guid_bytes.copy_from_slice(&header[56..72]);
        let disk_guid = format_guid(&disk_guid_bytes);

        let partition_entry_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
        let entry_count = u32::from_le_bytes(header[80..84].try_into().unwrap());
        let entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap());

        // Parse partition entries
        let entries_offset = (partition_entry_lba * 512) as usize;
        let mut partitions = Vec::new();

        for i in 0..entry_count {
            let offset = entries_offset + (i as usize * entry_size as usize);
            if offset + entry_size as usize > data.len() {
                break;
            }

            let entry = &data[offset..offset + entry_size as usize];

            // Check if entry is empty (all-zero type GUID)
            let type_guid_bytes = &entry[0..16];
            if type_guid_bytes.iter().all(|&b| b == 0) {
                continue;
            }

            let type_id = format_guid(type_guid_bytes.try_into().unwrap());

            let mut unique_guid_bytes = [0u8; 16];
            unique_guid_bytes.copy_from_slice(&entry[16..32]);
            let unique_id = format_guid(&unique_guid_bytes);

            let start_lba = u64::from_le_bytes(entry[32..40].try_into().unwrap());
            let end_lba = u64::from_le_bytes(entry[40..48].try_into().unwrap());
            let attributes = u64::from_le_bytes(entry[48..56].try_into().unwrap());

            // Name is UTF-16LE in bytes 56..128
            let name_bytes = &entry[56..entry_size.min(128) as usize];
            let name: String = name_bytes
                .chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
                .take_while(|&c| c != 0)
                .map(|c| char::from_u32(c as u32).unwrap_or('?'))
                .collect();

            partitions.push(Partition {
                index: i,
                name,
                type_id,
                unique_id: Some(unique_id),
                start_lba,
                end_lba,
                size_bytes: (end_lba - start_lba + 1) * 512,
                attributes,
                bootable: false,
            });
        }

        Ok(GptTable {
            disk_guid,
            first_usable_lba,
            last_usable_lba,
            entry_count,
            entry_size,
            partitions,
        })
    }

    /// Validate CRC32 checksums of the GPT header and entry array.
    pub fn validate_crc(&self, data: &[u8]) -> bool {
        if data.len() < 1024 {
            return false;
        }

        let header = &data[512..];

        // Header CRC32 is at offset 16 in the header, covering bytes 0..92
        // with the CRC field itself zeroed during calculation
        let header_size = u32::from_le_bytes(
            header[12..16].try_into().unwrap_or([0; 4]),
        ) as usize;
        if header_size < 92 || 512 + header_size > data.len() {
            return false;
        }

        let stored_header_crc = u32::from_le_bytes(
            header[16..20].try_into().unwrap_or([0; 4]),
        );

        // Zero the CRC field for computation
        let mut header_copy = header[..header_size].to_vec();
        header_copy[16..20].fill(0);
        let computed_header_crc = crc32fast::hash(&header_copy);

        if stored_header_crc != computed_header_crc {
            return false;
        }

        // Partition entry array CRC32 is at offset 88 in the header
        let stored_entry_crc = u32::from_le_bytes(
            header[88..92].try_into().unwrap_or([0; 4]),
        );

        let partition_entry_lba = u64::from_le_bytes(
            header[72..80].try_into().unwrap_or([0; 8]),
        );
        let entry_count = u32::from_le_bytes(
            header[80..84].try_into().unwrap_or([0; 4]),
        );
        let entry_size = u32::from_le_bytes(
            header[84..88].try_into().unwrap_or([0; 4]),
        );

        let entries_offset = (partition_entry_lba * 512) as usize;
        let entries_len = (entry_count * entry_size) as usize;

        if entries_offset + entries_len > data.len() {
            return false;
        }

        let computed_entry_crc = crc32fast::hash(&data[entries_offset..entries_offset + entries_len]);

        stored_entry_crc == computed_entry_crc
    }
}

/// Format a mixed-endian GUID from 16 bytes.
fn format_guid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        bytes[3], bytes[2], bytes[1], bytes[0],
        bytes[5], bytes[4],
        bytes[7], bytes[6],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}
