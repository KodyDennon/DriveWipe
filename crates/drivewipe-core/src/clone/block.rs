use serde::{Deserialize, Serialize};
use std::time::Instant;

use chrono::Utc;
use crossbeam_channel::Sender;
use uuid::Uuid;

use super::{CloneConfig, CloneMode, CloneResult};
use crate::error::{DriveWipeError, Result};
use crate::io::RawDeviceIo;
use crate::progress::ProgressEvent;
use crate::session::CancellationToken;

/// Perform a sector-by-sector block clone from source to target.
pub async fn clone_block(
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

    // Target must be at least as large as source (or we truncate)
    let copy_bytes = source_capacity.min(target_capacity);

    let _ = progress_tx.send(ProgressEvent::CloneStarted {
        session_id,
        source: config.source.display().to_string(),
        target: config.target.display().to_string(),
        total_bytes: copy_bytes,
    });

    let block_size = config.block_size;
    let mut bytes_copied: u64 = 0;
    let start = Instant::now();
    let mut last_progress = Instant::now();
    let mut throughput_bytes: u64 = 0;
    let mut throughput_timer = Instant::now();

    while bytes_copied < copy_bytes {
        if cancel_token.is_cancelled() {
            return Err(DriveWipeError::Cancelled);
        }

        let remaining = copy_bytes - bytes_copied;
        let read_len = (remaining as usize).min(block_size);

        // Use spawn_blocking for I/O since RawDeviceIo is synchronous
        let source_wrapper = crate::io::DeviceWrapper::new(source);

        let (n, read_res) = tokio::task::spawn_blocking(move || {
            let source_ref = unsafe { source_wrapper.get_mut() };
            let mut temp_buf = vec![0u8; read_len];
            let res = source_ref.read_at(bytes_copied, &mut temp_buf);
            (res, temp_buf)
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

        let n = n?;
        if n == 0 {
            break;
        }

        let target_wrapper = crate::io::DeviceWrapper::new(target);
        let write_res = tokio::task::spawn_blocking(move || {
            let target_ref = unsafe { target_wrapper.get_mut() };
            target_ref.write_at(bytes_copied, &read_res[..n])
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

        write_res?;
        bytes_copied += n as u64;
        throughput_bytes += n as u64;

        // Progress update every 500ms
        if last_progress.elapsed().as_secs_f64() >= 0.5 {
            let elapsed = throughput_timer.elapsed().as_secs_f64();
            let throughput_bps = if elapsed > 0.1 {
                throughput_bytes as f64 / elapsed
            } else {
                0.0
            };

            let _ = progress_tx.send(ProgressEvent::CloneProgress {
                session_id,
                bytes_copied,
                total_bytes: copy_bytes,
                throughput_bps,
            });
            last_progress = Instant::now();

            if elapsed >= 2.0 {
                throughput_timer = Instant::now();
                throughput_bytes = 0;
            }
        }
    }

    let target_wrapper = crate::io::DeviceWrapper::new(target);
    tokio::task::spawn_blocking(move || {
        let target_ref = unsafe { target_wrapper.get_mut() };
        target_ref.sync()
    })
    .await
    .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))??;

    let duration = start.elapsed().as_secs_f64();
    let throughput_mbps = if duration > 0.0 {
        (bytes_copied as f64 / (1024.0 * 1024.0)) / duration
    } else {
        0.0
    };

    // Verification pass
    let v_res = if config.verify {
        verify_clone(
            source,
            target,
            copy_bytes,
            block_size,
            session_id,
            progress_tx,
            cancel_token,
        )
        .await?
    } else {
        VerificationResult {
            verified: false,
            passed: None,
            source_hash: None,
            target_hash: None,
        }
    };

    let _ = progress_tx.send(ProgressEvent::CloneCompleted {
        session_id,
        duration_secs: duration,
        verified: v_res.verified,
    });

    Ok(CloneResult {
        session_id,
        source: config.source.clone(),
        target: config.target.clone(),
        mode: CloneMode::Block,
        bytes_copied,
        duration_secs: duration,
        throughput_mbps,
        verified: v_res.verified,
        verification_passed: v_res.passed,
        source_hash: v_res.source_hash,
        target_hash: v_res.target_hash,
        started_at,
        completed_at: Utc::now(),
    })
}

/// Result of a clone verification pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verified: bool,
    pub passed: Option<bool>,
    pub source_hash: Option<String>,
    pub target_hash: Option<String>,
}

async fn verify_clone(
    source: &mut dyn RawDeviceIo,
    target: &mut dyn RawDeviceIo,
    total_bytes: u64,
    block_size: usize,
    session_id: Uuid,
    progress_tx: &Sender<ProgressEvent>,
    cancel_token: &CancellationToken,
) -> Result<VerificationResult> {
    use blake3::Hasher;

    let _ = progress_tx.send(ProgressEvent::VerificationStarted { session_id });

    let mut source_hasher = Hasher::new();
    let mut target_hasher = Hasher::new();
    let mut offset: u64 = 0;

    while offset < total_bytes {
        if cancel_token.is_cancelled() {
            return Err(DriveWipeError::Cancelled);
        }

        let remaining = total_bytes - offset;
        let read_len = (remaining as usize).min(block_size);

        let source_wrapper = crate::io::DeviceWrapper::new(source);
        let target_wrapper = crate::io::DeviceWrapper::new(target);

        let (sn_res, tn_res, s_hash_chunk, t_hash_chunk) = tokio::task::spawn_blocking(move || {
            let source_ref = unsafe { source_wrapper.get_mut() };
            let target_ref = unsafe { target_wrapper.get_mut() };

            let mut s_buf = vec![0u8; read_len];
            let mut t_buf = vec![0u8; read_len];

            let sn = source_ref.read_at(offset, &mut s_buf);
            let tn = target_ref.read_at(offset, &mut t_buf);

            (sn, tn, s_buf, t_buf)
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

        let sn = sn_res?;
        let tn = tn_res?;

        source_hasher.update(&s_hash_chunk[..sn]);
        target_hasher.update(&t_hash_chunk[..tn]);

        offset += sn.max(tn) as u64;

        let _ = progress_tx.send(ProgressEvent::VerificationProgress {
            session_id,
            bytes_verified: offset,
            total_bytes,
        });
    }

    let source_hash = source_hasher.finalize().to_hex().to_string();
    let target_hash = target_hasher.finalize().to_hex().to_string();
    let passed = source_hash == target_hash;

    let _ = progress_tx.send(ProgressEvent::VerificationCompleted {
        session_id,
        passed,
        duration_secs: 0.0,
    });

    Ok(VerificationResult {
        verified: true,
        passed: Some(passed),
        source_hash: Some(source_hash),
        target_hash: Some(target_hash),
    })
}
