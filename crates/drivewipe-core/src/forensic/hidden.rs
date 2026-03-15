use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::RawDeviceIo;

/// Results of hidden area detection (HPA/DCO, hidden partitions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenAreaResult {
    /// Whether HPA (Host Protected Area) was detected.
    pub hpa_detected: bool,
    /// HPA size in bytes, if detected.
    pub hpa_size: Option<u64>,
    /// Whether DCO (Device Configuration Overlay) was detected.
    pub dco_detected: bool,
    /// DCO size in bytes, if detected.
    pub dco_size: Option<u64>,
    /// Hidden partitions found.
    pub hidden_partitions: Vec<HiddenPartition>,
    /// Unallocated gaps between partitions.
    pub unallocated_gaps: Vec<UnallocatedGap>,
    /// Summary message.
    pub summary: String,
}

/// A detected hidden partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenPartition {
    /// Start offset in bytes.
    pub start_offset: u64,
    /// Size in bytes.
    pub size: u64,
    /// Description or type.
    pub description: String,
}

/// An unallocated gap between partitions that may contain hidden data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnallocatedGap {
    /// Start offset in bytes.
    pub start_offset: u64,
    /// Size in bytes.
    pub size: u64,
    /// Whether data remnants were found in this gap.
    pub has_data: bool,
    /// Description.
    pub description: String,
}

/// Detect hidden areas on a device by analyzing partition table gaps and
/// scanning for data in unallocated regions.
///
/// Note: True HPA/DCO detection requires ATA passthrough commands which need
/// elevated privileges and Linux SG_IO. This function focuses on partition-level
/// analysis which works cross-platform.
pub fn detect_hidden_areas(device: &mut dyn RawDeviceIo) -> Result<HiddenAreaResult> {
    let capacity = device.capacity();
    let mut hidden_partitions = Vec::new();
    let mut unallocated_gaps = Vec::new();
    let mut summary_parts = Vec::new();

    // Read partition table
    let mut header_buf = vec![0u8; 34 * 512];
    let bytes_read = device.read_at(0, &mut header_buf)?;
    if bytes_read < 512 {
        return Ok(HiddenAreaResult {
            hpa_detected: false,
            hpa_size: None,
            dco_detected: false,
            dco_size: None,
            hidden_partitions,
            unallocated_gaps,
            summary: "Could not read partition table".to_string(),
        });
    }

    // Parse partition table and find gaps
    match crate::partition::PartitionTable::parse(&header_buf[..bytes_read]) {
        Ok(table) => {
            let partitions = table.partitions();

            // Sort partitions by start LBA
            let mut sorted: Vec<_> = partitions.iter().collect();
            sorted.sort_by_key(|p| p.start_lba);

            // Check for gap before first partition (after partition table area)
            let table_end: u64 = match table.table_type() {
                crate::partition::PartitionTableType::Gpt => 34 * 512, // GPT header + entries
                crate::partition::PartitionTableType::Mbr => 512,      // MBR only
                _ => 512,
            };

            if let Some(first) = sorted.first() {
                let first_start = first.start_lba * 512;
                if first_start > table_end + 1048576 {
                    // Gap > 1 MiB before first partition
                    let gap_size = first_start - table_end;
                    let has_data = check_region_has_data(device, table_end, gap_size.min(65536));
                    unallocated_gaps.push(UnallocatedGap {
                        start_offset: table_end,
                        size: gap_size,
                        has_data,
                        description: format!("Gap before first partition ({} bytes)", gap_size),
                    });
                    if has_data {
                        summary_parts.push(format!(
                            "Data found in {} byte gap before first partition",
                            gap_size
                        ));
                    }
                }
            }

            // Check gaps between partitions
            for window in sorted.windows(2) {
                let prev_end = window[0].end_lba * 512 + 512;
                let next_start = window[1].start_lba * 512;

                if next_start > prev_end + 1048576 {
                    // Gap > 1 MiB
                    let gap_size = next_start - prev_end;
                    let has_data = check_region_has_data(device, prev_end, gap_size.min(65536));
                    unallocated_gaps.push(UnallocatedGap {
                        start_offset: prev_end,
                        size: gap_size,
                        has_data,
                        description: format!(
                            "Gap between partitions #{} and #{} ({} bytes)",
                            window[0].index, window[1].index, gap_size
                        ),
                    });
                    if has_data {
                        summary_parts.push(format!(
                            "Data found in {} byte gap between partitions #{} and #{}",
                            gap_size, window[0].index, window[1].index
                        ));
                    }
                }
            }

            // Check gap after last partition
            if let Some(last) = sorted.last() {
                let last_end = last.end_lba * 512 + 512;
                // For GPT, backup table is at the end, so account for that
                let usable_end = match table.table_type() {
                    crate::partition::PartitionTableType::Gpt => capacity - (34 * 512),
                    crate::partition::PartitionTableType::Mbr => capacity,
                    _ => capacity,
                };

                if usable_end > last_end + 1048576 {
                    let gap_size = usable_end - last_end;
                    let has_data = check_region_has_data(device, last_end, gap_size.min(65536));
                    unallocated_gaps.push(UnallocatedGap {
                        start_offset: last_end,
                        size: gap_size,
                        has_data,
                        description: format!("Gap after last partition ({} bytes)", gap_size),
                    });
                    if has_data {
                        summary_parts.push(format!(
                            "Data found in {} byte gap after last partition",
                            gap_size
                        ));
                    }
                }
            }

            // Check for protective MBR pointing to unusual areas
            if table.table_type() == crate::partition::PartitionTableType::Mbr {
                // Look for hidden type partitions (0x11, 0x14, 0x16, 0x17)
                // These are just the partition entries, not custom detection
                for part in &partitions {
                    let name_lower = part.name.to_lowercase();
                    if name_lower.contains("hidden") || name_lower.contains("diagnostic") {
                        hidden_partitions.push(HiddenPartition {
                            start_offset: part.start_lba * 512,
                            size: part.size_bytes,
                            description: format!("Hidden/diagnostic partition: {}", part.name),
                        });
                        summary_parts.push(format!(
                            "Hidden partition '{}' at LBA {}",
                            part.name, part.start_lba
                        ));
                    }
                }
            }
        }
        Err(e) => {
            summary_parts.push(format!("Could not parse partition table: {}", e));
        }
    }

    let gaps_with_data = unallocated_gaps.iter().filter(|g| g.has_data).count();
    let total_gaps = unallocated_gaps.len();

    // Note: HPA/DCO detection requires ATA passthrough via the drivewipe-live
    // crate (Linux only). In non-live mode, we can only detect partition-level
    // hidden areas. Full HPA/DCO probing is available in the live environment.
    let hpa_dco_note = "HPA/DCO: requires live environment for detection";

    // Build the final summary string, always including the HPA/DCO note
    let summary = if summary_parts.is_empty() {
        if total_gaps == 0 {
            format!("No hidden areas detected. {}", hpa_dco_note)
        } else {
            format!(
                "No hidden areas detected. {} unallocated gap(s) found, {} with data. {}",
                total_gaps, gaps_with_data, hpa_dco_note
            )
        }
    } else {
        format!("{}. {}", summary_parts.join("; "), hpa_dco_note)
    };

    Ok(HiddenAreaResult {
        hpa_detected: false,
        hpa_size: None,
        dco_detected: false,
        dco_size: None,
        hidden_partitions,
        unallocated_gaps,
        summary,
    })
}

/// Check if a region of the device contains non-zero data.
/// Reads up to `max_check_bytes` from the region and returns true if any non-zero bytes found.
fn check_region_has_data(device: &mut dyn RawDeviceIo, offset: u64, max_check_bytes: u64) -> bool {
    let check_size = max_check_bytes.min(65536) as usize;
    let mut buf = vec![0u8; check_size];
    match device.read_at(offset, &mut buf) {
        Ok(n) if n > 0 => buf[..n].iter().any(|&b| b != 0),
        _ => false,
    }
}
