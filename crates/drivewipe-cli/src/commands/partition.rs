use std::path::PathBuf;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;

/// Run the `partition list` subcommand.
pub async fn list(_config: &DriveWipeConfig, device: &str) -> Result<()> {
    let enumerator = drive::create_enumerator();
    let drive_info = enumerator
        .inspect(&PathBuf::from(device))
        .await
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

    let device_wrapper = drivewipe_core::io::DeviceWrapper::new(device_io.as_mut());
    let buf = tokio::task::spawn_blocking(move || {
        let device_ref = unsafe { device_wrapper.get_mut() };
        let mut b = vec![0u8; 34 * 512]; // GPT needs at least 34 sectors
        device_ref.read_at(0, &mut b).map(|_| b)
    }).await.map_err(|e| anyhow::anyhow!("Task failed: {}", e))?
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

/// Run the `partition create` subcommand.
pub async fn create(
    _config: &DriveWipeConfig,
    device: &str,
    start_lba: u64,
    end_lba: u64,
    type_id: &str,
    name: &str,
) -> Result<()> {
    let mut device_io = drivewipe_core::io::open_device(&PathBuf::from(device), true)
        .context("Failed to open device for writing")?;

    let device_wrapper = drivewipe_core::io::DeviceWrapper::new(device_io.as_mut());
    let buf = tokio::task::spawn_blocking(move || {
        let device_ref = unsafe { device_wrapper.get_mut() };
        let mut b = vec![0u8; 34 * 512];
        device_ref.read_at(0, &mut b).map(|_| b)
    })
    .await?
    .context("Failed to read partition table")?;

    let mut table = drivewipe_core::partition::PartitionTable::parse(&buf)
        .context("Failed to parse partition table")?;

    drivewipe_core::partition::ops::create_partition(
        &mut *device_io,
        &mut table,
        start_lba,
        end_lba,
        type_id,
        name,
    )
    .context("Failed to create partition")?;

    drivewipe_core::partition::ops::write_table(&mut *device_io, &table)
        .context("Failed to write partition table")?;

    println!("Successfully created partition on {}", device);
    Ok(())
}

/// Run the `partition delete` subcommand.
pub async fn delete(_config: &DriveWipeConfig, device: &str, index: u32) -> Result<()> {
    let mut device_io = drivewipe_core::io::open_device(&PathBuf::from(device), true)
        .context("Failed to open device for writing")?;

    let device_wrapper = drivewipe_core::io::DeviceWrapper::new(device_io.as_mut());
    let buf = tokio::task::spawn_blocking(move || {
        let device_ref = unsafe { device_wrapper.get_mut() };
        let mut b = vec![0u8; 34 * 512];
        device_ref.read_at(0, &mut b).map(|_| b)
    })
    .await?
    .context("Failed to read partition table")?;

    let mut table = drivewipe_core::partition::PartitionTable::parse(&buf)
        .context("Failed to parse partition table")?;

    drivewipe_core::partition::ops::delete_partition(&mut *device_io, &mut table, index)
        .context("Failed to delete partition")?;

    drivewipe_core::partition::ops::write_table(&mut *device_io, &table)
        .context("Failed to write partition table")?;

    println!("Successfully deleted partition {} from {}", index, device);
    Ok(())
}

/// Run the `partition resize` subcommand.
pub async fn resize(_config: &DriveWipeConfig, device: &str, index: u32, new_end: u64) -> Result<()> {
    let mut device_io = drivewipe_core::io::open_device(&PathBuf::from(device), true)
        .context("Failed to open device for writing")?;

    let device_wrapper = drivewipe_core::io::DeviceWrapper::new(device_io.as_mut());
    let buf = tokio::task::spawn_blocking(move || {
        let device_ref = unsafe { device_wrapper.get_mut() };
        let mut b = vec![0u8; 34 * 512];
        device_ref.read_at(0, &mut b).map(|_| b)
    })
    .await?
    .context("Failed to read partition table")?;

    let mut table = drivewipe_core::partition::PartitionTable::parse(&buf)
        .context("Failed to parse partition table")?;

    drivewipe_core::partition::ops::resize_partition(&mut *device_io, &mut table, index, new_end)
        .context("Failed to resize partition")?;

    drivewipe_core::partition::ops::write_table(&mut *device_io, &table)
        .context("Failed to write partition table")?;

    println!("Successfully resized partition {} on {}", index, device);
    Ok(())
}
