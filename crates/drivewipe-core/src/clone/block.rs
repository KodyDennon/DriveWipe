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
pub fn clone_block(
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
    let mut buf = vec![0u8; block_size];
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

        let n = source.read_at(bytes_copied, &mut buf[..read_len])?;
        if n == 0 {
            break;
        }

        target.write_at(bytes_copied, &buf[..n])?;
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

    target.sync()?;

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
        )?
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

fn verify_clone(
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
    let mut source_buf = vec![0u8; block_size];
    let mut target_buf = vec![0u8; block_size];
    let mut offset: u64 = 0;

    while offset < total_bytes {
        if cancel_token.is_cancelled() {
            return Err(DriveWipeError::Cancelled);
        }

        let remaining = total_bytes - offset;
        let read_len = (remaining as usize).min(block_size);

        let sn = source.read_at(offset, &mut source_buf[..read_len])?;
        let tn = target.read_at(offset, &mut target_buf[..read_len])?;

        source_hasher.update(&source_buf[..sn]);
        target_hasher.update(&target_buf[..tn]);

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
