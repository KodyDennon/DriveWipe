use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use crossbeam_channel::Sender;
use uuid::Uuid;

use crate::config::DriveWipeConfig;
use crate::error::{DriveWipeError, Result};
use crate::io::{allocate_aligned_buffer, RawDeviceIo, DEFAULT_BLOCK_SIZE};
use crate::progress::ProgressEvent;
use crate::resume::WipeState;
use crate::types::*;
use crate::verify::{Verifier, zero_verify::ZeroVerifier, pattern_verify::PatternVerifier};
use crate::wipe::WipeMethod;

/// A cooperative cancellation token that can be shared across threads.
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Reset the token so it can be reused for a new batch of operations
    /// without reinstalling signal handlers.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn clone_token(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// The main wipe session orchestrator. Coordinates the wipe engine by driving
/// the method's passes, writing blocks to the device, tracking progress,
/// managing resume state, and emitting progress events.
pub struct WipeSession {
    pub session_id: Uuid,
    pub drive_info: DriveInfo,
    pub method: Box<dyn WipeMethod>,
    pub config: DriveWipeConfig,
    pub verify_after: bool,
}

impl WipeSession {
    pub fn new(
        drive_info: DriveInfo,
        method: Box<dyn WipeMethod>,
        config: DriveWipeConfig,
    ) -> Self {
        let verify_after = config.auto_verify;
        Self {
            session_id: Uuid::new_v4(),
            drive_info,
            method,
            config,
            verify_after,
        }
    }

    /// Execute the wipe operation.
    ///
    /// Runs all passes defined by the wipe method, writing pattern data to the
    /// device block by block, emitting progress events, and optionally resuming
    /// from a prior interrupted session.
    ///
    /// Pass numbers in the `WipeState` and `PassResult` are 1-indexed for
    /// display purposes, but calls to `WipeMethod::pattern_for_pass()` use
    /// 0-indexed pass numbers as required by that API.
    pub fn execute(
        &self,
        device: &mut dyn RawDeviceIo,
        progress_tx: &Sender<ProgressEvent>,
        cancel_token: &CancellationToken,
        resume_state: Option<WipeState>,
    ) -> Result<WipeResult> {
        let total_bytes = self.drive_info.capacity;
        let total_passes = self.method.pass_count();
        let session_start = Instant::now();

        // When resuming, reuse the original session's UUID so that events
        // and the final WipeResult carry a consistent identity.
        let session_id = resume_state
            .as_ref()
            .map(|s| s.session_id)
            .unwrap_or(self.session_id);

        let started_at = resume_state
            .as_ref()
            .map(|s| s.started_at)
            .unwrap_or_else(Utc::now);

        // Determine resume point.
        // `current_pass` in WipeState is 1-indexed (1 = first pass).
        let (start_pass_1indexed, start_offset) = if let Some(ref state) = resume_state {
            (state.current_pass, state.bytes_written_this_pass)
        } else {
            (1, 0)
        };

        let mut total_bytes_written: u64 = resume_state
            .as_ref()
            .map(|s| s.total_bytes_written)
            .unwrap_or(0);

        // Sessions directory for state persistence
        let sessions_dir = self.config.sessions_dir.clone();

        // Create initial wipe state for persistence
        let mut wipe_state = resume_state.unwrap_or_else(|| {
            WipeState::new(
                session_id,
                self.drive_info.path.clone(),
                self.drive_info.serial.clone(),
                self.drive_info.model.clone(),
                total_bytes,
                self.method.id().to_string(),
                total_passes,
                self.verify_after,
            )
        });

        // Send SessionStarted event
        let _ = progress_tx.send(ProgressEvent::SessionStarted {
            session_id: session_id,
            device_path: self.drive_info.path.display().to_string(),
            device_serial: self.drive_info.serial.clone(),
            method_id: self.method.id().to_string(),
            method_name: self.method.name().to_string(),
            total_bytes,
            total_passes,
        });

        let mut pass_results: Vec<PassResult> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let state_save_interval = self.config.state_save_interval_secs;

        // Iterate passes: pass_1idx is 1-indexed, pass_0idx is 0-indexed.
        for pass_1idx in start_pass_1indexed..=total_passes {
            let pass_0idx = pass_1idx - 1;
            let pass_start = Instant::now();

            // Get the pattern generator for this pass (0-indexed)
            let mut pattern = self.method.pattern_for_pass(pass_0idx);
            let pattern_name = pattern.name().to_string();

            let _ = progress_tx.send(ProgressEvent::PassStarted {
                session_id: session_id,
                pass_number: pass_1idx,
                pass_name: pattern_name.clone(),
            });

            // Allocate a page-aligned write buffer for O_DIRECT / F_NOCACHE.
            let mut buffer = allocate_aligned_buffer(DEFAULT_BLOCK_SIZE, 4096);
            let mut bytes_written_this_pass: u64 = if pass_1idx == start_pass_1indexed {
                start_offset
            } else {
                0
            };

            let mut last_state_save = Instant::now();
            let mut throughput_timer = Instant::now();
            let mut throughput_bytes: u64 = 0;

            while bytes_written_this_pass < total_bytes {
                // Check for cancellation
                if cancel_token.is_cancelled() {
                    wipe_state.update_progress(
                        pass_1idx,
                        bytes_written_this_pass,
                        total_bytes_written,
                    );
                    let _ = wipe_state.save(&sessions_dir);

                    let _ = progress_tx.send(ProgressEvent::Interrupted {
                        session_id: session_id,
                        reason: "User cancelled".to_string(),
                        bytes_written: total_bytes_written,
                    });

                    let total_duration = session_start.elapsed().as_secs_f64();
                    let avg_throughput = if total_duration > 0.0 {
                        (total_bytes_written as f64 / (1024.0 * 1024.0)) / total_duration
                    } else {
                        0.0
                    };

                    return Ok(WipeResult {
                        session_id: session_id,
                        device_path: self.drive_info.path.clone(),
                        device_serial: self.drive_info.serial.clone(),
                        device_model: self.drive_info.model.clone(),
                        device_capacity: total_bytes,
                        method_id: self.method.id().to_string(),
                        method_name: self.method.name().to_string(),
                        outcome: WipeOutcome::Cancelled,
                        passes: pass_results,
                        total_bytes_written,
                        total_duration_secs: total_duration,
                        average_throughput_mbps: avg_throughput,
                        verification_passed: None,
                        started_at,
                        completed_at: Utc::now(),
                        hostname: hostname::get()
                            .ok()
                            .and_then(|h| h.into_string().ok())
                            .unwrap_or_default(),
                        operator: self.config.operator_name.clone(),
                        warnings,
                        errors: vec![],
                    });
                }

                // Determine how many bytes to write this iteration
                let remaining = total_bytes - bytes_written_this_pass;
                let write_len = (remaining as usize).min(buffer.len());
                let write_buf = &mut buffer[..write_len];

                // Fill buffer with the pattern
                pattern.fill(write_buf);

                // Write to device at the current offset
                match device.write_at(bytes_written_this_pass, write_buf) {
                    Ok(n) => {
                        bytes_written_this_pass += n as u64;
                        total_bytes_written += n as u64;
                        throughput_bytes += n as u64;
                    }
                    Err(e) => {
                        wipe_state.update_progress(
                            pass_1idx,
                            bytes_written_this_pass,
                            total_bytes_written,
                        );
                        let _ = wipe_state.save(&sessions_dir);

                        let msg =
                            format!("Write error at offset {bytes_written_this_pass}: {e}");
                        let _ = progress_tx.send(ProgressEvent::Error {
                            session_id: session_id,
                            message: msg,
                        });

                        return Err(e);
                    }
                }

                // Calculate throughput
                let elapsed_throughput = throughput_timer.elapsed().as_secs_f64();
                let throughput_bps = if elapsed_throughput > 0.0 {
                    throughput_bytes as f64 / elapsed_throughput
                } else {
                    0.0
                };

                // Send BlockWritten event
                let _ = progress_tx.send(ProgressEvent::BlockWritten {
                    session_id: session_id,
                    pass_number: pass_1idx,
                    bytes_written: bytes_written_this_pass,
                    total_bytes,
                    throughput_bps,
                });

                // Reset throughput counter periodically
                if elapsed_throughput >= 1.0 {
                    throughput_timer = Instant::now();
                    throughput_bytes = 0;
                }

                // Periodically save state
                if last_state_save.elapsed().as_secs_f64() >= state_save_interval as f64 {
                    wipe_state.update_progress(
                        pass_1idx,
                        bytes_written_this_pass,
                        total_bytes_written,
                    );
                    let _ = wipe_state.save(&sessions_dir);
                    last_state_save = Instant::now();
                }
            }

            // Sync the device after each pass
            if let Err(e) = device.sync() {
                let msg = format!("Sync warning after pass {pass_1idx}: {e}");
                let _ = progress_tx.send(ProgressEvent::Warning {
                    session_id: session_id,
                    message: msg.clone(),
                });
                warnings.push(msg);
            }

            let pass_duration = pass_start.elapsed().as_secs_f64();
            let throughput_mbps = if pass_duration > 0.0 {
                (total_bytes as f64 / (1024.0 * 1024.0)) / pass_duration
            } else {
                0.0
            };

            let _ = progress_tx.send(ProgressEvent::PassCompleted {
                session_id: session_id,
                pass_number: pass_1idx,
                duration_secs: pass_duration,
                throughput_mbps,
            });

            pass_results.push(PassResult {
                pass_number: pass_1idx,
                pattern_name: pattern_name.clone(),
                bytes_written: total_bytes,
                duration_secs: pass_duration,
                throughput_mbps,
                verified: false,
                verification_passed: None,
            });

            // Save state after each completed pass
            wipe_state.update_progress(pass_1idx, total_bytes, total_bytes_written);
            let _ = wipe_state.save(&sessions_dir);
        }

        // Verification phase — delegate to Verifier trait implementations
        let verification_passed = if self.verify_after {
            // Determine which verifier to use based on the final pass pattern.
            let pattern = self.method.pattern_for_pass(total_passes - 1);
            let pattern_name = pattern.name();

            let passed = if pattern_name.contains("Zero") {
                // Deterministic zero pattern — use the optimised ZeroVerifier.
                let verifier = ZeroVerifier;
                match verifier.verify(device, session_id, progress_tx) {
                    Ok(result) => result,
                    Err(DriveWipeError::VerificationFailed { offset, expected, actual }) => {
                        let msg = format!(
                            "Verification mismatch at offset {offset:#x}: \
                             expected {expected:#04x}, got {actual:#04x}"
                        );
                        warnings.push(msg);
                        false
                    }
                    Err(e) => {
                        let msg = format!("Verification error: {e}");
                        warnings.push(msg);
                        false
                    }
                }
            } else if pattern_name.contains("Random") {
                // Random pattern — byte-level comparison is impossible because
                // `pattern_for_pass()` creates a new AES-CTR seed each time.
                // Instead, verify the device is NOT all zeros (confirming that
                // something was actually written).
                let verify_start = Instant::now();

                let _ = progress_tx.send(ProgressEvent::VerificationStarted {
                    session_id,
                });

                let mut sample_buf = vec![0u8; DEFAULT_BLOCK_SIZE];
                let sample_len = (total_bytes as usize).min(DEFAULT_BLOCK_SIZE);
                let passed = match device.read_at(0, &mut sample_buf[..sample_len]) {
                    Ok(n) => {
                        let all_zero = sample_buf[..n].iter().all(|&b| b == 0);
                        if all_zero {
                            let msg = "Random pattern verification: first block is all \
                                       zeros — expected non-zero data"
                                .to_string();
                            warnings.push(msg);
                            false
                        } else {
                            true
                        }
                    }
                    Err(e) => {
                        let msg = format!("Verification read error at offset 0: {e}");
                        warnings.push(msg);
                        false
                    }
                };

                let warn_msg = "Random pattern verification: confirmed device is non-zero \
                                (byte-level verification not possible for random data)"
                    .to_string();
                if passed {
                    warnings.push(warn_msg);
                }

                let verify_duration = verify_start.elapsed().as_secs_f64();
                let _ = progress_tx.send(ProgressEvent::VerificationCompleted {
                    session_id,
                    passed,
                    duration_secs: verify_duration,
                });

                passed
            } else {
                // Deterministic pattern (OneFill, ConstantFill, RepeatingPattern,
                // etc.) — use PatternVerifier with a fresh copy of the pattern.
                let fresh_pattern = self.method.pattern_for_pass(total_passes - 1);
                let verifier = PatternVerifier::new(fresh_pattern);
                match verifier.verify(device, session_id, progress_tx) {
                    Ok(result) => result,
                    Err(DriveWipeError::VerificationFailed { offset, expected, actual }) => {
                        let msg = format!(
                            "Verification mismatch at offset {offset:#x}: \
                             expected {expected:#04x}, got {actual:#04x}"
                        );
                        warnings.push(msg);
                        false
                    }
                    Err(e) => {
                        let msg = format!("Verification error: {e}");
                        warnings.push(msg);
                        false
                    }
                }
            };

            // Mark the last pass as verified
            if let Some(last_pass) = pass_results.last_mut() {
                last_pass.verified = true;
                last_pass.verification_passed = Some(passed);
            }

            Some(passed)
        } else {
            None
        };

        // Determine outcome
        let outcome = match verification_passed {
            Some(true) => WipeOutcome::Success,
            Some(false) => WipeOutcome::Failed,
            None => {
                if warnings.is_empty() {
                    WipeOutcome::Success
                } else {
                    WipeOutcome::SuccessWithWarnings
                }
            }
        };

        let total_duration = session_start.elapsed().as_secs_f64();
        let avg_throughput = if total_duration > 0.0 {
            (total_bytes_written as f64 / (1024.0 * 1024.0)) / total_duration
        } else {
            0.0
        };

        let _ = progress_tx.send(ProgressEvent::Completed {
            session_id: session_id,
            outcome,
            duration_secs: total_duration,
        });

        // Clean up state file on completion
        let _ = wipe_state.cleanup(&sessions_dir);

        Ok(WipeResult {
            session_id: session_id,
            device_path: self.drive_info.path.clone(),
            device_serial: self.drive_info.serial.clone(),
            device_model: self.drive_info.model.clone(),
            device_capacity: total_bytes,
            method_id: self.method.id().to_string(),
            method_name: self.method.name().to_string(),
            outcome,
            passes: pass_results,
            total_bytes_written,
            total_duration_secs: total_duration,
            average_throughput_mbps: avg_throughput,
            verification_passed,
            started_at,
            completed_at: Utc::now(),
            hostname: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_default(),
            operator: self.config.operator_name.clone(),
            warnings,
            errors: vec![],
        })
    }
}
