use std::time::Instant;

use chrono::Utc;
use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::io::RawDeviceIo;
use crate::partition::PartitionTable;
use crate::progress::ProgressEvent;
use crate::session::CancellationToken;

use super::{CloneConfig, CloneMode, CloneResult};

/// Perform a partition-aware clone that copies the partition table and each
/// partition individually, skipping unallocated space.
pub async fn clone_partition_aware(
    source: &mut dyn RawDeviceIo,
    target: &mut dyn RawDeviceIo,
    config: &CloneConfig,
    progress_tx: &Sender<ProgressEvent>,
    cancel_token: &CancellationToken,
) -> Result<CloneResult> {
    let session_id = Uuid::new_v4();
    let source_capacity = source.capacity();
    let target_capacity = target.capacity();
    let started_at = Utc::now();

    log::info!(
        "Partition-aware clone: source={} bytes, target={} bytes",
        source_capacity,
        target_capacity,
    );

    let _ = progress_tx.send(ProgressEvent::CloneStarted {
        session_id,
        source: config.source.display().to_string(),
        target: config.target.display().to_string(),
        total_bytes: source_capacity,
    });

    // Read source partition table header (enough for GPT: 34 sectors)
    let header_sectors = 34;
    let header_size = header_sectors * 512;
    let source_wrapper = crate::io::DeviceWrapper::new(source);
    let header_buf = tokio::task::spawn_blocking(move || {
        let source_ref = unsafe { source_wrapper.get_mut() };
        let mut buf = vec![0u8; header_size];
        let res = source_ref.read_at(0, &mut buf);
        (res, buf)
    })
    .await
    .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

    let (read_res, header_buf) = header_buf;
    read_res?;

    let source_table = match PartitionTable::parse(&header_buf) {
        Ok(table) => table,
        Err(e) => {
            log::warn!(
                "Failed to parse partition table: {}. Falling back to block clone.",
                e
            );
            return super::block::clone_block(source, target, config, progress_tx, cancel_token)
                .await;
        }
    };

    log::info!("Source partition table: {:?}", source_table.table_type());

    // Calculate total bytes to copy (header + all partitions)
    let partitions = source_table.partitions();
    let mut regions: Vec<(u64, u64)> = Vec::new(); // (offset, length) pairs

    // Always copy the partition table area
    regions.push((0, header_size as u64));

    let mut total_data_bytes: u64 = header_size as u64;

    for part in &partitions {
        let part_start = part.start_lba * 512;
        let part_size = part.size_bytes;
        let part_end = part_start + part_size;

        if part_end > target_capacity {
            log::warn!(
                "Partition #{} (LBA {}-{}) exceeds target capacity, skipping",
                part.index,
                part.start_lba,
                part.end_lba
            );
            continue;
        }

        regions.push((part_start, part_size));
        total_data_bytes += part_size;
    }

    // Copy each region
    let block_size = config.block_size;
    let mut bytes_copied: u64 = 0;
    let start = Instant::now();
    let mut last_progress = Instant::now();

    for (region_offset, region_size) in &regions {
        let mut offset_in_region: u64 = 0;

        while offset_in_region < *region_size {
            if cancel_token.is_cancelled() {
                return Err(DriveWipeError::Cancelled);
            }

            let remaining = *region_size - offset_in_region;
            let chunk_len = (remaining as usize).min(block_size);
            let abs_offset = region_offset + offset_in_region;

            // Read from source
            let source_wrapper = crate::io::DeviceWrapper::new(source);
            let (n, read_buf) = tokio::task::spawn_blocking(move || {
                let src = unsafe { source_wrapper.get_mut() };
                let mut buf = vec![0u8; chunk_len];
                let res = src.read_at(abs_offset, &mut buf);
                (res, buf)
            })
            .await
            .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

            let n = n?;
            if n == 0 {
                break;
            }

            // Write to target at same offset
            let target_wrapper = crate::io::DeviceWrapper::new(target);
            let write_res = tokio::task::spawn_blocking(move || {
                let tgt = unsafe { target_wrapper.get_mut() };
                tgt.write_at(abs_offset, &read_buf[..n])
            })
            .await
            .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

            write_res?;
            offset_in_region += n as u64;
            bytes_copied += n as u64;

            // Bandwidth throttling
            if let Some(limit_bps) = config.bandwidth_limit_bps {
                let elapsed = start.elapsed().as_secs_f64();
                let target_elapsed = bytes_copied as f64 / limit_bps as f64;
                if target_elapsed > elapsed {
                    let sleep_ms = ((target_elapsed - elapsed) * 1000.0) as u64;
                    tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
                }
            }

            if last_progress.elapsed().as_secs_f64() >= 0.5 {
                let _ = progress_tx.send(ProgressEvent::CloneProgress {
                    session_id,
                    bytes_copied,
                    total_bytes: total_data_bytes,
                    throughput_bps: bytes_copied as f64 / start.elapsed().as_secs_f64(),
                });
                last_progress = Instant::now();
            }
        }
    }

    // Sync target
    let target_wrapper = crate::io::DeviceWrapper::new(target);
    tokio::task::spawn_blocking(move || {
        let tgt = unsafe { target_wrapper.get_mut() };
        tgt.sync()
    })
    .await
    .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))??;

    let duration = start.elapsed().as_secs_f64();
    let throughput_mbps = if duration > 0.0 {
        (bytes_copied as f64 / (1024.0 * 1024.0)) / duration
    } else {
        0.0
    };

    let _ = progress_tx.send(ProgressEvent::CloneCompleted {
        session_id,
        duration_secs: duration,
        verified: false,
    });

    Ok(CloneResult {
        session_id,
        source: config.source.clone(),
        target: config.target.clone(),
        mode: CloneMode::Partition,
        bytes_copied,
        duration_secs: duration,
        throughput_mbps,
        verified: false,
        verification_passed: None,
        source_hash: None,
        target_hash: None,
        started_at,
        completed_at: Utc::now(),
    })
}
