pub mod filesystem;
pub mod gpt;
pub mod mbr;
pub mod ops;
pub mod types;

pub use types::{Partition, PartitionTableType};

use crate::error::Result;

/// Parsed partition table (GPT or MBR).
pub enum PartitionTable {
    Gpt(gpt::GptTable),
    Mbr(mbr::MbrTable),
}

impl PartitionTable {
    /// Parse a partition table from the first sectors of a device.
    pub fn parse(data: &[u8]) -> Result<Self> {
        // Try GPT first (has protective MBR + GPT header at LBA 1)
        if data.len() >= 1024 {
            if let Ok(gpt_table) = gpt::GptTable::parse(data) {
                return Ok(PartitionTable::Gpt(gpt_table));
            }
        }

        // Fall back to MBR
        if data.len() >= 512 {
            let mbr_table = mbr::MbrTable::parse(data)?;
            return Ok(PartitionTable::Mbr(mbr_table));
        }

        Err(crate::error::DriveWipeError::InvalidPartitionTable(
            "Data too small to contain a partition table".to_string(),
        ))
    }

    /// Get the partition table type.
    pub fn table_type(&self) -> PartitionTableType {
        match self {
            PartitionTable::Gpt(_) => PartitionTableType::Gpt,
            PartitionTable::Mbr(_) => PartitionTableType::Mbr,
        }
    }

    /// Get all partitions.
    pub fn partitions(&self) -> Vec<&Partition> {
        match self {
            PartitionTable::Gpt(t) => t.partitions.iter().collect(),
            PartitionTable::Mbr(t) => t.partitions.iter().collect(),
        }
    }
}
