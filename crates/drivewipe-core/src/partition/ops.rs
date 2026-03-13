use crate::error::{DriveWipeError, Result};
use crate::io::RawDeviceIo;

use super::PartitionTable;
use super::types::Partition;

/// Preview of a partition operation (dry-run result).
#[derive(Debug, Clone)]
pub struct OperationPreview {
    pub description: String,
    pub affected_partitions: Vec<u32>,
    pub data_loss_risk: bool,
}

/// Create a new partition in the given table.
///
/// For GPT tables: inserts a new entry with the specified LBA range, type GUID, and name.
/// For MBR tables: inserts a new primary partition entry (max 4).
///
/// The table is modified in memory. Call `write_table()` afterward to persist.
pub fn create_partition(
    _device: &mut dyn RawDeviceIo,
    table: &mut PartitionTable,
    start_lba: u64,
    end_lba: u64,
    type_id: &str,
    name: &str,
) -> Result<Partition> {
    if start_lba >= end_lba {
        return Err(DriveWipeError::Partition(
            "Start LBA must be less than end LBA".to_string(),
        ));
    }

    match table {
        PartitionTable::Gpt(gpt) => {
            // Check for overlap with existing partitions
            for existing in &gpt.partitions {
                if start_lba <= existing.end_lba && end_lba >= existing.start_lba {
                    return Err(DriveWipeError::Partition(format!(
                        "New partition overlaps with partition {} ({} - {})",
                        existing.index, existing.start_lba, existing.end_lba
                    )));
                }
            }

            // Check bounds
            if start_lba < gpt.first_usable_lba || end_lba > gpt.last_usable_lba {
                return Err(DriveWipeError::Partition(format!(
                    "Partition extends outside usable range ({} - {})",
                    gpt.first_usable_lba, gpt.last_usable_lba
                )));
            }

            // Find next available index
            let next_index = gpt
                .partitions
                .iter()
                .map(|p| p.index)
                .max()
                .map(|m| m + 1)
                .unwrap_or(0);

            if next_index >= gpt.entry_count {
                return Err(DriveWipeError::Partition(
                    "No free partition entries in GPT table".to_string(),
                ));
            }

            let partition = Partition {
                index: next_index,
                name: name.to_string(),
                type_id: type_id.to_string(),
                unique_id: None, // Would be generated as a random UUID
                start_lba,
                end_lba,
                size_bytes: (end_lba - start_lba + 1) * 512,
                attributes: 0,
                bootable: false,
            };

            gpt.partitions.push(partition.clone());
            Ok(partition)
        }
        PartitionTable::Mbr(mbr) => {
            if mbr.partitions.len() >= 4 {
                return Err(DriveWipeError::Partition(
                    "MBR supports maximum 4 primary partitions".to_string(),
                ));
            }

            // Check for overlap
            for existing in &mbr.partitions {
                if start_lba <= existing.end_lba && end_lba >= existing.start_lba {
                    return Err(DriveWipeError::Partition(format!(
                        "New partition overlaps with partition {} ({} - {})",
                        existing.index, existing.start_lba, existing.end_lba
                    )));
                }
            }

            // Find next free slot (0-3)
            let used_indices: Vec<u32> = mbr.partitions.iter().map(|p| p.index).collect();
            let next_index = (0..4u32)
                .find(|i| !used_indices.contains(i))
                .ok_or_else(|| {
                    DriveWipeError::Partition("No free MBR partition slots".to_string())
                })?;

            let partition = Partition {
                index: next_index,
                name: name.to_string(),
                type_id: type_id.to_string(),
                unique_id: None,
                start_lba,
                end_lba,
                size_bytes: (end_lba - start_lba + 1) * 512,
                attributes: 0,
                bootable: false,
            };

            mbr.partitions.push(partition.clone());
            Ok(partition)
        }
    }
}

/// Delete a partition from the table by index.
pub fn delete_partition(
    _device: &mut dyn RawDeviceIo,
    table: &mut PartitionTable,
    partition_index: u32,
) -> Result<()> {
    match table {
        PartitionTable::Gpt(gpt) => {
            let pos = gpt
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;
            gpt.partitions.remove(pos);
            Ok(())
        }
        PartitionTable::Mbr(mbr) => {
            let pos = mbr
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;
            mbr.partitions.remove(pos);
            Ok(())
        }
    }
}

/// Resize a partition by changing its end LBA.
///
/// Only shrinking is safe without filesystem cooperation. Growing requires
/// ensuring no overlap and that the filesystem can expand.
pub fn resize_partition(
    _device: &mut dyn RawDeviceIo,
    table: &mut PartitionTable,
    partition_index: u32,
    new_end_lba: u64,
) -> Result<()> {
    match table {
        PartitionTable::Gpt(gpt) => {
            let part_pos = gpt
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;

            let start_lba = gpt.partitions[part_pos].start_lba;
            let old_end = gpt.partitions[part_pos].end_lba;

            if new_end_lba < start_lba {
                return Err(DriveWipeError::Partition(
                    "New end LBA is before partition start".to_string(),
                ));
            }

            if new_end_lba > gpt.last_usable_lba {
                return Err(DriveWipeError::Partition(format!(
                    "New end LBA exceeds usable range (max: {})",
                    gpt.last_usable_lba
                )));
            }

            // Check for overlap with other partitions when growing
            if new_end_lba > old_end {
                for (i, other) in gpt.partitions.iter().enumerate() {
                    if i != part_pos && new_end_lba >= other.start_lba && start_lba <= other.end_lba
                    {
                        return Err(DriveWipeError::Partition(format!(
                            "Resize would overlap with partition {}",
                            other.index
                        )));
                    }
                }
            }

            gpt.partitions[part_pos].end_lba = new_end_lba;
            gpt.partitions[part_pos].size_bytes = (new_end_lba - start_lba + 1) * 512;
            Ok(())
        }
        PartitionTable::Mbr(mbr) => {
            let part_pos = mbr
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;

            let start_lba = mbr.partitions[part_pos].start_lba;
            let old_end = mbr.partitions[part_pos].end_lba;

            if new_end_lba < start_lba {
                return Err(DriveWipeError::Partition(
                    "New end LBA is before partition start".to_string(),
                ));
            }

            // Check overlap when growing
            if new_end_lba > old_end {
                for (i, other) in mbr.partitions.iter().enumerate() {
                    if i != part_pos && new_end_lba >= other.start_lba && start_lba <= other.end_lba
                    {
                        return Err(DriveWipeError::Partition(format!(
                            "Resize would overlap with partition {}",
                            other.index
                        )));
                    }
                }
            }

            mbr.partitions[part_pos].end_lba = new_end_lba;
            mbr.partitions[part_pos].size_bytes = (new_end_lba - start_lba + 1) * 512;
            Ok(())
        }
    }
}

/// Move a partition to a new start LBA, preserving its size.
///
/// This only modifies the table. Data movement must be handled separately
/// (e.g., by the clone module for block-level copy).
pub fn move_partition(
    _device: &mut dyn RawDeviceIo,
    table: &mut PartitionTable,
    partition_index: u32,
    new_start_lba: u64,
) -> Result<()> {
    match table {
        PartitionTable::Gpt(gpt) => {
            // Find the partition and compute the new bounds
            let part_pos = gpt
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;

            let size_sectors =
                gpt.partitions[part_pos].end_lba - gpt.partitions[part_pos].start_lba;
            let new_end_lba = new_start_lba + size_sectors;

            if new_start_lba < gpt.first_usable_lba {
                return Err(DriveWipeError::Partition(format!(
                    "New start LBA is before usable range (min: {})",
                    gpt.first_usable_lba
                )));
            }
            if new_end_lba > gpt.last_usable_lba {
                return Err(DriveWipeError::Partition(format!(
                    "Moved partition would exceed usable range (max: {})",
                    gpt.last_usable_lba
                )));
            }

            // Check overlap with other partitions
            for (i, other) in gpt.partitions.iter().enumerate() {
                if i != part_pos && new_start_lba <= other.end_lba && new_end_lba >= other.start_lba
                {
                    return Err(DriveWipeError::Partition(format!(
                        "Move would overlap with partition {}",
                        other.index
                    )));
                }
            }

            // Apply move
            gpt.partitions[part_pos].start_lba = new_start_lba;
            gpt.partitions[part_pos].end_lba = new_end_lba;
            Ok(())
        }
        PartitionTable::Mbr(mbr) => {
            let part_pos = mbr
                .partitions
                .iter()
                .position(|p| p.index == partition_index)
                .ok_or_else(|| {
                    DriveWipeError::Partition(format!("Partition {} not found", partition_index))
                })?;

            let size_sectors =
                mbr.partitions[part_pos].end_lba - mbr.partitions[part_pos].start_lba;
            let new_end_lba = new_start_lba + size_sectors;

            // Check overlap
            for (i, other) in mbr.partitions.iter().enumerate() {
                if i != part_pos && new_start_lba <= other.end_lba && new_end_lba >= other.start_lba
                {
                    return Err(DriveWipeError::Partition(format!(
                        "Move would overlap with partition {}",
                        other.index
                    )));
                }
            }

            mbr.partitions[part_pos].start_lba = new_start_lba;
            mbr.partitions[part_pos].end_lba = new_end_lba;
            Ok(())
        }
    }
}

/// Preview an operation without making changes.
pub fn preview_operation(description: &str, affected: &[u32], data_loss: bool) -> OperationPreview {
    OperationPreview {
        description: description.to_string(),
        affected_partitions: affected.to_vec(),
        data_loss_risk: data_loss,
    }
}

/// Write the partition table to the device.
///
/// This persists all changes made to the in-memory `PartitionTable` struct.
/// For GPT, this writes both the Primary (LBA 1) and Backup (Last LBA) tables.
/// For MBR, this writes the MBR at LBA 0.
pub fn write_table(device: &mut dyn RawDeviceIo, table: &PartitionTable) -> Result<()> {
    match table {
        PartitionTable::Gpt(gpt) => gpt.write(device),
        PartitionTable::Mbr(mbr) => mbr.write(device),
    }
}
