use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::time::Instant;

use chrono::Utc;
use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};
use crate::io::RawDeviceIo;
use crate::progress::ProgressEvent;
use crate::session::CancellationToken;

use super::image::{CloneImage, CloneImageHeader};
use super::{CloneConfig, CloneMode, CloneResult};

/// Clone a block device to a compressed/encrypted image file.
pub async fn clone_device_to_image(
    source: &mut dyn RawDeviceIo,
    image_path: &std::path::Path,
    config: &CloneConfig,
    progress_tx: &Sender<ProgressEvent>,
    cancel_token: &CancellationToken,
) -> Result<CloneResult> {
    let session_id = Uuid::new_v4();
    let source_capacity = source.capacity();
    let started_at = Utc::now();

    let _ = progress_tx.send(ProgressEvent::CloneStarted {
        session_id,
        source: config.source.display().to_string(),
        target: image_path.display().to_string(),
        total_bytes: source_capacity,
    });

    let file = File::create(image_path).map_err(|e| DriveWipeError::Io {
        path: image_path.to_path_buf(),
        source: e,
    })?;
    let mut writer = BufWriter::new(file);

    let block_size = config.block_size;
    let chunk_count = source_capacity.div_ceil(block_size as u64);

    // Encryption setup
    let use_encryption = config.encrypt && config.password.is_some();
    let (enc_key, mut enc_nonce, enc_salt_hex, enc_nonce_hex) = if use_encryption {
        let password = config.password.as_ref().unwrap();
        let salt = crate::crypto::encrypt::generate_salt();
        let nonce = crate::crypto::encrypt::generate_nonce();
        let key = crate::crypto::encrypt::derive_key(password.as_bytes(), &salt, 100_000);
        (
            Some(key),
            Some(nonce),
            Some(hex::encode(salt)),
            Some(hex::encode(nonce)),
        )
    } else {
        (None, None, None, None)
    };

    let header = CloneImageHeader {
        version: 1,
        source_model: "Unknown".to_string(), // In a real impl, we'd pass DriveInfo
        source_serial: "Unknown".to_string(),
        source_capacity,
        block_size: block_size as u32,
        chunk_count,
        compression: config.compression,
        encrypted: use_encryption,
        encryption_salt: enc_salt_hex,
        encryption_nonce: enc_nonce_hex,
        source_hash: None,
        created_at: started_at,
    };

    CloneImage::write_header(&mut writer, &header)?;

    let mut bytes_copied: u64 = 0;
    let start = Instant::now();
    let mut last_progress = Instant::now();

    let device_wrapper = crate::io::DeviceWrapper::new(source);

    for i in 0..chunk_count {
        if cancel_token.is_cancelled() {
            return Err(DriveWipeError::Cancelled);
        }

        let offset = i * block_size as u64;
        let remaining = source_capacity - offset;
        let read_len = (remaining as usize).min(block_size);

        let (n, read_res) = tokio::task::spawn_blocking(move || {
            let source_ref = unsafe { device_wrapper.get_mut() };
            let mut temp_buf = vec![0u8; read_len];
            let res = source_ref.read_at(offset, &mut temp_buf);
            (res, temp_buf)
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

        let n = n?;
        if n == 0 {
            break;
        }

        if use_encryption {
            let chunk_nonce = enc_nonce.as_ref().unwrap();
            CloneImage::write_encrypted_chunk(
                &mut writer,
                &read_res[..n],
                config.compression,
                enc_key.as_ref(),
                Some(chunk_nonce),
            )?;
            crate::crypto::encrypt::increment_nonce(enc_nonce.as_mut().unwrap());
        } else {
            CloneImage::write_chunk(&mut writer, &read_res[..n], config.compression)?;
        }

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
                total_bytes: source_capacity,
                throughput_bps: bytes_copied as f64 / start.elapsed().as_secs_f64(),
            });
            last_progress = Instant::now();
        }
    }

    writer.flush().map_err(|e| DriveWipeError::Io {
        path: image_path.to_path_buf(),
        source: e,
    })?;

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
        target: image_path.to_path_buf(),
        mode: CloneMode::Image,
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

/// Restore a block device from a DriveWipe clone image.
pub async fn restore_image_to_device(
    image_path: &std::path::Path,
    target: &mut dyn RawDeviceIo,
    config: &CloneConfig,
    progress_tx: &Sender<ProgressEvent>,
    cancel_token: &CancellationToken,
) -> Result<CloneResult> {
    let session_id = Uuid::new_v4();
    let started_at = Utc::now();

    let file = File::open(image_path).map_err(|e| DriveWipeError::Io {
        path: image_path.to_path_buf(),
        source: e,
    })?;
    let mut reader = BufReader::new(file);

    let header = CloneImage::read_header(&mut reader)?;
    let target_capacity = target.capacity();

    if header.source_capacity > target_capacity {
        return Err(DriveWipeError::Clone(format!(
            "Target device too small: image needs {} bytes, target has {}",
            header.source_capacity, target_capacity
        )));
    }

    // Decryption setup
    let (dec_key, mut dec_nonce) = if header.encrypted {
        let password = config.password.as_ref().ok_or_else(|| {
            DriveWipeError::Encryption(
                "Image is encrypted but no password was provided".to_string(),
            )
        })?;
        let salt_hex = header.encryption_salt.as_ref().ok_or_else(|| {
            DriveWipeError::Encryption("Encrypted image missing salt in header".to_string())
        })?;
        let nonce_hex = header.encryption_nonce.as_ref().ok_or_else(|| {
            DriveWipeError::Encryption("Encrypted image missing nonce in header".to_string())
        })?;
        let salt = hex::decode(salt_hex)
            .map_err(|e| DriveWipeError::Encryption(format!("Invalid salt hex: {e}")))?;
        let nonce_vec = hex::decode(nonce_hex)
            .map_err(|e| DriveWipeError::Encryption(format!("Invalid nonce hex: {e}")))?;
        let key = crate::crypto::encrypt::derive_key(password.as_bytes(), &salt, 100_000);
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&nonce_vec);
        (Some(key), Some(nonce))
    } else {
        (None, None)
    };

    let _ = progress_tx.send(ProgressEvent::CloneStarted {
        session_id,
        source: image_path.display().to_string(),
        target: config.target.display().to_string(),
        total_bytes: header.source_capacity,
    });

    let mut bytes_restored: u64 = 0;
    let start = Instant::now();
    let mut last_progress = Instant::now();

    let target_wrapper = crate::io::DeviceWrapper::new(target);

    for i in 0..header.chunk_count {
        if cancel_token.is_cancelled() {
            return Err(DriveWipeError::Cancelled);
        }

        let chunk_data = if header.encrypted {
            let chunk_nonce = dec_nonce.as_ref().unwrap();
            let data = CloneImage::read_encrypted_chunk(
                &mut reader,
                header.compression,
                dec_key.as_ref(),
                Some(chunk_nonce),
            )?;
            crate::crypto::encrypt::increment_nonce(dec_nonce.as_mut().unwrap());
            data
        } else {
            CloneImage::read_chunk(&mut reader, header.compression)?
        };

        let n = chunk_data.len();
        let offset = i * header.block_size as u64;

        let write_res = tokio::task::spawn_blocking(move || {
            let target_ref = unsafe { target_wrapper.get_mut() };
            target_ref.write_at(offset, &chunk_data)
        })
        .await
        .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

        write_res?;
        bytes_restored += n as u64;

        if last_progress.elapsed().as_secs_f64() >= 0.5 {
            let _ = progress_tx.send(ProgressEvent::CloneProgress {
                session_id,
                bytes_copied: bytes_restored,
                total_bytes: header.source_capacity,
                throughput_bps: bytes_restored as f64 / start.elapsed().as_secs_f64(),
            });
            last_progress = Instant::now();
        }
    }

    let sync_res = tokio::task::spawn_blocking(move || {
        let target_ref = unsafe { target_wrapper.get_mut() };
        target_ref.sync()
    })
    .await
    .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;
    sync_res?;

    let duration = start.elapsed().as_secs_f64();
    let throughput_mbps = if duration > 0.0 {
        (bytes_restored as f64 / (1024.0 * 1024.0)) / duration
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
        source: image_path.to_path_buf(),
        target: config.target.clone(),
        mode: CloneMode::Image,
        bytes_copied: bytes_restored,
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
