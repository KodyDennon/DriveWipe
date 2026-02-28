use crossbeam_channel::Sender;
use uuid::Uuid;

use super::Verifier;
use crate::error::{DriveWipeError, Result};
use crate::io::{DEFAULT_BLOCK_SIZE, RawDeviceIo};
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

impl Verifier for ZeroVerifier {
    fn verify(
        &self,
        device: &mut dyn RawDeviceIo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<bool> {
        let total_bytes = device.capacity();

        let _ = progress_tx.send(ProgressEvent::VerificationStarted { session_id });

        let verify_start = std::time::Instant::now();

        let mut buf = vec![0u8; DEFAULT_BLOCK_SIZE];
        let mut bytes_verified: u64 = 0;

        while bytes_verified < total_bytes {
            let remaining = total_bytes - bytes_verified;
            let chunk_len = (remaining as usize).min(DEFAULT_BLOCK_SIZE);
            let read_slice = &mut buf[..chunk_len];

            let bytes_read = device.read_at(bytes_verified, read_slice)?;

            if !Self::is_zero(&read_slice[..bytes_read]) {
                // Find the exact byte offset of the first non-zero value
                if let Some(local_offset) = Self::first_nonzero_offset(&read_slice[..bytes_read]) {
                    let offset = bytes_verified + local_offset as u64;
                    let actual = read_slice[local_offset];

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
