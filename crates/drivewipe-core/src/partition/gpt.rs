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
        let header_size = u32::from_le_bytes(header[12..16].try_into().unwrap_or([0; 4])) as usize;
        if header_size < 92 || 512 + header_size > data.len() {
            return false;
        }

        let stored_header_crc = u32::from_le_bytes(header[16..20].try_into().unwrap_or([0; 4]));

        // Zero the CRC field for computation
        let mut header_copy = header[..header_size].to_vec();
        header_copy[16..20].fill(0);
        let computed_header_crc = crc32fast::hash(&header_copy);

        if stored_header_crc != computed_header_crc {
            return false;
        }

        // Partition entry array CRC32 is at offset 88 in the header
        let stored_entry_crc = u32::from_le_bytes(header[88..92].try_into().unwrap_or([0; 4]));

        let partition_entry_lba = u64::from_le_bytes(header[72..80].try_into().unwrap_or([0; 8]));
        let entry_count = u32::from_le_bytes(header[80..84].try_into().unwrap_or([0; 4]));
        let entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap_or([0; 4]));

        let entries_offset = (partition_entry_lba * 512) as usize;
        let entries_len = (entry_count * entry_size) as usize;

        if entries_offset + entries_len > data.len() {
            return false;
        }

        let computed_entry_crc =
            crc32fast::hash(&data[entries_offset..entries_offset + entries_len]);

        stored_entry_crc == computed_entry_crc
    }

    /// Serialize the GPT table to bytes (Header + Entries).
    ///
    /// This generates the data for LBA 1 (Primary Header) and the partition entries.
    /// It automatically recalculates CRC32 checksums.
    ///
    /// Returns (Header Bytes, Entry Array Bytes).
    pub fn to_bytes(
        &self,
        header_lba: u64,
        backup_lba: u64,
        entries_lba: u64,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        // 1. Serialize Partition Entries
        let mut entries_bytes = vec![0u8; (self.entry_count * self.entry_size) as usize];

        for (i, part) in self.partitions.iter().enumerate() {
            if i as u32 >= self.entry_count {
                break;
            }
            let offset = (i as u32 * self.entry_size) as usize;
            let entry = &mut entries_bytes[offset..offset + self.entry_size as usize];

            // Type GUID
            let type_guid = parse_guid(&part.type_id).unwrap_or([0; 16]);
            entry[0..16].copy_from_slice(&type_guid);

            // Unique GUID
            let unique_guid =
                parse_guid(part.unique_id.as_deref().unwrap_or_default()).unwrap_or([0; 16]);
            entry[16..32].copy_from_slice(&unique_guid);

            // LBAs
            entry[32..40].copy_from_slice(&part.start_lba.to_le_bytes());
            entry[40..48].copy_from_slice(&part.end_lba.to_le_bytes());

            // Attributes
            entry[48..56].copy_from_slice(&part.attributes.to_le_bytes());

            // Name (UTF-16LE)
            let name_utf16: Vec<u16> = part.name.encode_utf16().take(36).collect(); // Max 36 chars (72 bytes)
            for (j, char_code) in name_utf16.iter().enumerate() {
                let char_bytes = char_code.to_le_bytes();
                entry[56 + j * 2] = char_bytes[0];
                entry[56 + j * 2 + 1] = char_bytes[1];
            }
        }

        let entries_crc = crc32fast::hash(&entries_bytes);

        // 2. Serialize Header
        let mut header = vec![0u8; 512]; // Standard 512-byte header

        // Signature "EFI PART"
        header[0..8].copy_from_slice(GPT_SIGNATURE);

        // Revision 1.0 (00 00 01 00)
        header[8..12].copy_from_slice(&[0x00, 0x00, 0x01, 0x00]);

        // Header size (92 bytes)
        header[12..16].copy_from_slice(&92u32.to_le_bytes());

        // CRC32 (zero initially)
        header[16..20].fill(0);

        // Reserved
        header[20..24].fill(0);

        // Current LBA (Primary: 1, Backup: Last LBA)
        header[24..32].copy_from_slice(&header_lba.to_le_bytes());

        // Backup LBA (Primary: Last LBA, Backup: 1)
        header[32..40].copy_from_slice(&backup_lba.to_le_bytes());

        // Usable LBAs
        header[40..48].copy_from_slice(&self.first_usable_lba.to_le_bytes());
        header[48..56].copy_from_slice(&self.last_usable_lba.to_le_bytes());

        // Disk GUID
        let disk_guid = parse_guid(&self.disk_guid).unwrap_or([0; 16]);
        header[56..72].copy_from_slice(&disk_guid);

        // Partition Entries Starting LBA
        header[72..80].copy_from_slice(&entries_lba.to_le_bytes());

        // Number of Partition Entries
        header[80..84].copy_from_slice(&self.entry_count.to_le_bytes());

        // Size of Partition Entry
        header[84..88].copy_from_slice(&self.entry_size.to_le_bytes());

        // Partition Entries CRC32
        header[88..92].copy_from_slice(&entries_crc.to_le_bytes());

        // Calculate Header CRC32
        // We only checksum the first 92 bytes as per spec (header_size)
        let header_crc = crc32fast::hash(&header[0..92]);
        header[16..20].copy_from_slice(&header_crc.to_le_bytes());

        Ok((header, entries_bytes))
    }

    /// Write the GPT table to the device (Primary and Backup).
    pub fn write(&self, device: &mut dyn crate::io::RawDeviceIo) -> Result<()> {
        let capacity_sectors = device.capacity() / 512;
        let last_lba = capacity_sectors - 1;

        // 1. Write Protective MBR at LBA 0
        let pmbr = generate_protective_mbr(capacity_sectors);
        device.write_at(0, &pmbr)?;

        // 2. Write Primary GPT (LBA 1)
        let (primary_header, entries) = self.to_bytes(1, last_lba, 2)?;

        // Write Primary Header at LBA 1
        device.write_at(512, &primary_header)?;

        // Write Partition Entries starting at LBA 2
        device.write_at(1024, &entries)?;

        // 3. Write Backup GPT (Last LBA)
        // Entries usually start at Last LBA - 33 (for 128 entries of 128 bytes = 16KB = 32 sectors)
        let entries_sectors = (entries.len() as u64).div_ceil(512);
        let backup_entries_lba = last_lba - entries_sectors;

        let (backup_header, _) = self.to_bytes(last_lba, 1, backup_entries_lba)?;

        // Write Backup Entries
        device.write_at(backup_entries_lba * 512, &entries)?;

        // Write Backup Header at Last LBA
        device.write_at(last_lba * 512, &backup_header)?;

        device.sync()?;

        Ok(())
    }
}

/// Generate a standard protective MBR for a GPT-partitioned disk.
fn generate_protective_mbr(capacity_sectors: u64) -> [u8; 512] {
    let mut mbr = [0u8; 512];

    // Partition 1 entry at offset 446
    let entry = &mut mbr[446..462];
    entry[0] = 0x00; // Status
    entry[1] = 0x00; // CHS Start
    entry[2] = 0x02;
    entry[3] = 0x00;
    entry[4] = 0xEE; // Type GPT Protective
    entry[5] = 0xFF; // CHS End
    entry[6] = 0xFF;
    entry[7] = 0xFF;

    // LBA Start (1)
    entry[8..12].copy_from_slice(&1u32.to_le_bytes());

    // Size in sectors (min(total-1, 0xFFFFFFFF))
    let size = (capacity_sectors - 1).min(0xFFFFFFFF) as u32;
    entry[12..16].copy_from_slice(&size.to_le_bytes());

    // Signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;

    mbr
}

/// Parse a mixed-endian GUID string into 16 bytes.
fn parse_guid(guid: &str) -> Option<[u8; 16]> {
    let hex_str = guid.replace('-', "");
    if hex_str.len() != 32 {
        return None;
    }

    let bytes = hex::decode(hex_str).ok()?;
    if bytes.len() != 16 {
        return None;
    }

    // Mixed endian conversion
    let mut out = [0u8; 16];
    out[0] = bytes[3];
    out[1] = bytes[2];
    out[2] = bytes[1];
    out[3] = bytes[0];
    out[4] = bytes[5];
    out[5] = bytes[4];
    out[6] = bytes[7];
    out[7] = bytes[6];
    out[8] = bytes[8];
    out[9] = bytes[9];
    out[10] = bytes[10];
    out[11] = bytes[11];
    out[12] = bytes[12];
    out[13] = bytes[13];
    out[14] = bytes[14];
    out[15] = bytes[15];
    Some(out)
}

/// Format a mixed-endian GUID from 16 bytes.
fn format_guid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        bytes[3],
        bytes[2],
        bytes[1],
        bytes[0],
        bytes[5],
        bytes[4],
        bytes[7],
        bytes[6],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}
