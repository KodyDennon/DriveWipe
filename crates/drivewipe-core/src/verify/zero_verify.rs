use async_trait::async_trait;
use crossbeam_channel::Sender;
use uuid::Uuid;

use super::Verifier;
use crate::error::{DriveWipeError, Result};
use crate::io::{DEFAULT_BLOCK_SIZE, DeviceWrapper, RawDeviceIo};
use crate::progress::ProgressEvent;

/// Optimized verifier that checks whether the entire device is filled with zeros.
///
/// Uses `u64`-aligned reads to check 8 bytes at a time, which is significantly
/// faster than a byte-by-byte comparison.
pub struct ZeroVerifier;

impl ZeroVerifier {
    /// Check whether a buffer is entirely zero using u64-aligned reads.
    /// Falls back to byte-by-byte checking for any trailing bytes that are
    /// not aligned to an 8-byte boundary.
    fn is_zero(buf: &[u8]) -> bool {
        // Process the bulk of the buffer as u64 chunks
        //
        // SAFETY: `align_to` is safe for integer types. The prefix/suffix
        // slices cover any bytes that fall outside the aligned region.
        let (prefix, aligned, suffix) = unsafe { buf.align_to::<u64>() };

        // Check any unaligned prefix bytes
        for &b in prefix {
            if b != 0 {
                return false;
            }
        }

        // Check aligned u64 words (8 bytes at a time)
        for &word in aligned {
            if word != 0 {
                return false;
            }
        }

        // Check any unaligned suffix bytes
        for &b in suffix {
            if b != 0 {
                return false;
            }
        }

        true
    }

    /// Find the offset of the first non-zero byte within a buffer.
    fn first_nonzero_offset(buf: &[u8]) -> Option<usize> {
        buf.iter().position(|&b| b != 0)
    }
}

#[async_trait]
impl Verifier for ZeroVerifier {
    async fn verify(
        &self,
        device: &mut dyn RawDeviceIo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<bool> {
        let total_bytes = device.capacity();

        let _ = progress_tx.send(ProgressEvent::VerificationStarted { session_id });

        let verify_start = std::time::Instant::now();

        let mut bytes_verified: u64 = 0;

        // Pre-allocate a reusable buffer to avoid per-iteration allocation.
        let mut reusable_buf: Vec<u8> = vec![0u8; DEFAULT_BLOCK_SIZE];

        while bytes_verified < total_bytes {
            let remaining = total_bytes - bytes_verified;
            let chunk_len = (remaining as usize).min(DEFAULT_BLOCK_SIZE);

            let pass_offset = bytes_verified;
            let device_wrapper = DeviceWrapper::new(device);

            // Take ownership of the buffer, send it to the blocking task,
            // and reclaim it afterwards to avoid re-allocating each iteration.
            let send_buf = std::mem::take(&mut reusable_buf);

            let (read_res, read_data) = tokio::task::spawn_blocking(move || {
                // SAFETY: device outlives this task; exclusive access is
                // maintained because we .await immediately after spawn.
                let device_ref = unsafe { device_wrapper.get_mut() };
                let mut buf = send_buf;
                buf.resize(chunk_len, 0);
                let res = device_ref.read_at(pass_offset, &mut buf[..chunk_len]);
                (res, buf)
            })
            .await
            .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

            // Reclaim buffer for reuse
            reusable_buf = read_data;

            let bytes_read = read_res?;

            if !Self::is_zero(&reusable_buf[..bytes_read]) {
                // Find the exact byte offset of the first non-zero value
                if let Some(local_offset) = Self::first_nonzero_offset(&reusable_buf[..bytes_read])
                {
                    let offset = bytes_verified + local_offset as u64;
                    let actual = reusable_buf[local_offset];

                    let _ = progress_tx.send(ProgressEvent::VerificationCompleted {
                        session_id,
                        passed: false,
                        duration_secs: verify_start.elapsed().as_secs_f64(),
                    });

                    return Err(DriveWipeError::VerificationFailed {
                        offset,
                        expected: 0x00,
                        actual,
                    });
                }
            }

            bytes_verified += bytes_read as u64;

            let _ = progress_tx.send(ProgressEvent::VerificationProgress {
                session_id,
                bytes_verified,
                total_bytes,
            });
        }

        let duration = verify_start.elapsed().as_secs_f64();

        let _ = progress_tx.send(ProgressEvent::VerificationCompleted {
            session_id,
            passed: true,
            duration_secs: duration,
        });

        Ok(true)
    }
}
