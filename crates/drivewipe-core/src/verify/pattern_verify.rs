use async_trait::async_trait;
use std::sync::Mutex;

use crossbeam_channel::Sender;
use uuid::Uuid;

use super::Verifier;
use crate::error::{DriveWipeError, Result};
use crate::io::{DEFAULT_BLOCK_SIZE, RawDeviceIo, allocate_aligned_buffer};
use crate::progress::ProgressEvent;
use crate::wipe::patterns::PatternGenerator;

/// Verifies that device contents match the expected pattern by reading back
/// every block and comparing against a freshly generated pattern stream.
///
/// The caller must supply a pattern generator that produces the same byte
/// stream that was written to the device (i.e., a freshly constructed copy of
/// the same pattern type used in the final pass).
pub struct PatternVerifier {
    /// The pattern generator is wrapped in a `Mutex` so that the `verify`
    /// method (which takes `&self` per the `Verifier` trait) can call
    /// `PatternGenerator::fill(&mut self, ...)`.
    pattern: Mutex<Box<dyn PatternGenerator + Send>>,
}

impl PatternVerifier {
    pub fn new(pattern: Box<dyn PatternGenerator + Send>) -> Self {
        Self {
            pattern: Mutex::new(pattern),
        }
    }
}

#[async_trait]
impl Verifier for PatternVerifier {
    async fn verify(
        &self,
        device: &mut dyn RawDeviceIo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Result<bool> {
        let total_bytes = device.capacity();

        let _ = progress_tx.send(ProgressEvent::VerificationStarted { session_id });

        let verify_start = std::time::Instant::now();

        let mut expected_buf = allocate_aligned_buffer(DEFAULT_BLOCK_SIZE, 4096);
        let mut bytes_verified: u64 = 0;

        // Create a Send-able pointer for spawn_blocking
        let ptr_parts: [usize; 2] = unsafe { std::mem::transmute(device as *mut dyn RawDeviceIo) };

        while bytes_verified < total_bytes {
            let remaining = total_bytes - bytes_verified;
            let chunk_len = (remaining as usize).min(DEFAULT_BLOCK_SIZE);
            let expected_slice = &mut expected_buf[..chunk_len];

            // Fill expected buffer with the pattern
            {
                let mut pattern = match self.pattern.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => {
                        log::warn!("Pattern lock was poisoned, recovering");
                        poisoned.into_inner()
                    }
                };
                pattern.fill(expected_slice);
            }

            let pass_offset = bytes_verified;
            let expected_data = expected_slice.to_vec();

            // Perform the read in a blocking task
            let (read_res, read_data) = tokio::task::spawn_blocking(move || {
                let device_ref = unsafe { 
                    let wide_ptr: *mut dyn RawDeviceIo = std::mem::transmute(ptr_parts);
                    &mut *wide_ptr
                };
                let mut temp_buf = vec![0u8; chunk_len];
                let res = device_ref.read_at(pass_offset, &mut temp_buf);
                (res, temp_buf)
            }).await.map_err(|e| DriveWipeError::IoGeneric(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

            let bytes_read = read_res?;

            // Compare only the bytes we actually read
            if read_data[..bytes_read] != expected_data[..bytes_read] {
                // Find the first mismatch for diagnostic reporting
                for (i, (actual, expected)) in read_data[..bytes_read]
                    .iter()
                    .zip(expected_data[..bytes_read].iter())
                    .enumerate()
                {
                    if actual != expected {
                        let offset = bytes_verified + i as u64;

                        let _ = progress_tx.send(ProgressEvent::VerificationCompleted {
                            session_id,
                            passed: false,
                            duration_secs: verify_start.elapsed().as_secs_f64(),
                        });

                        return Err(DriveWipeError::VerificationFailed {
                            offset,
                            expected: *expected,
                            actual: *actual,
                        });
                    }
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
