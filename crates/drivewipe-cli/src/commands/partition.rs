use std::path::PathBuf;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;

/// Run the `partition list` subcommand.
pub fn list(_config: &DriveWipeConfig, device: &str) -> Result<()> {
    let enumerator = drive::create_enumerator();
    let drive_info = enumerator
        .inspect(&PathBuf::from(device))
        .context("Failed to inspect device")?;

    println!(
        "Partition Table: {} {}",
        drive_info.model, drive_info.serial
    );
    println!("  Capacity: {}", drive_info.capacity_display());

    if let Some(ref table_type) = drive_info.partition_table {
        println!("  Table type: {}", table_type);
    }
    println!("  Partitions: {}", drive_info.partition_count);

    // Read partition table from device
    let mut device_io = drivewipe_core::io::open_device(&PathBuf::from(device), false)
        .context("Failed to open device")?;

    let mut buf = vec![0u8; 34 * 512]; // GPT needs at least 34 sectors
    device_io
        .read_at(0, &mut buf)
        .context("Failed to read partition table")?;

    match drivewipe_core::partition::PartitionTable::parse(&buf) {
        Ok(table) => {
            println!(
                "\n  {:>4}  {:>12}  {:>12}  {:>12}  Name/Type",
                "#", "Start LBA", "End LBA", "Size"
            );
            println!("  {}", "-".repeat(60));

            for part in table.partitions() {
                println!(
                    "  {:>4}  {:>12}  {:>12}  {:>12}  {}",
                    part.index,
                    part.start_lba,
                    part.end_lba,
                    drivewipe_core::format_bytes(part.size_bytes),
                    if part.name.is_empty() {
                        &part.type_id
                    } else {
                        &part.name
                    },
                );
            }
        }
        Err(e) => {
            println!("\n  Could not parse partition table: {}", e);
        }
    }

    Ok(())
}
