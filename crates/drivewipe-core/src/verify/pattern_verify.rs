use async_trait::async_trait;
use std::sync::Mutex;

use crossbeam_channel::Sender;
use uuid::Uuid;

use super::Verifier;
use crate::error::{DriveWipeError, Result};
use crate::io::{DEFAULT_BLOCK_SIZE, DeviceWrapper, RawDeviceIo, allocate_aligned_buffer};
use crate::progress::ProgressEvent;
use crate::wipe::patterns::PatternGenerator;

/// Verifies that device contents match the expected pattern by reading back
/// every block and comparing against a regenerated pattern stream.
///
/// The caller must supply a generator that produces the same bytes that were
/// written. Because [`PatternGenerator::fill_at`] is keyed to the absolute
/// device offset, this holds for every pattern type — including random passes,
/// provided the generator is the same instance that performed the write (or one
/// rebuilt from its recorded seed). Passing a newly constructed [`RandomFill`]
/// would compare against an unrelated keystream and always fail.
///
/// [`RandomFill`]: crate::wipe::patterns::RandomFill
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

    /// Reclaim the pattern generator once verification is done.
    ///
    /// A wipe session needs its generator back after verifying a pass so the
    /// same instance — and therefore, for random passes, the same keystream —
    /// can be reused for the final verification.
    pub fn into_pattern(self) -> Box<dyn PatternGenerator + Send> {
        self.pattern.into_inner().unwrap_or_else(|p| p.into_inner())
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

        // The read buffer must be page-aligned: O_DIRECT rejects reads into a
        // buffer that is not aligned to the device's logical block size, and a
        // plain Vec<u8> carries no such guarantee.
        let mut reusable_buf = allocate_aligned_buffer(DEFAULT_BLOCK_SIZE, 4096);

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
                pattern.fill_at(bytes_verified, expected_slice);
            }

            let pass_offset = bytes_verified;
            // Copy expected data before sending to the blocking task.
            let expected_data: Vec<u8> = expected_slice.to_vec();

            let device_wrapper = DeviceWrapper::new(device);
            let send_buf = reusable_buf;

            // Perform the read in a blocking task, moving the aligned buffer in
            // and back out so its alignment survives.
            let (read_res, read_data) = tokio::task::spawn_blocking(move || {
                // SAFETY: device outlives this task; exclusive access is
                // maintained because we .await immediately after spawn.
                let device_ref = unsafe { device_wrapper.get_mut() };
                let mut buf = send_buf;
                let res = device_ref.read_at(pass_offset, &mut buf[..chunk_len]);
                (res, buf)
            })
            .await
            .map_err(|e| DriveWipeError::IoGeneric(std::io::Error::other(e.to_string())))?;

            // Reclaim buffer for reuse
            reusable_buf = read_data;

            let bytes_read = read_res?;

            // Compare only the bytes we actually read
            if reusable_buf[..bytes_read] != expected_data[..bytes_read] {
                // Find the first mismatch for diagnostic reporting
                for (i, (actual, expected)) in reusable_buf[..bytes_read]
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
