use std::sync::{Arc, Mutex};
use std::time::Instant;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use drivewipe_core::progress::ProgressEvent;

/// Manages indicatif progress bars for a wipe session.
///
/// Wraps a `MultiProgress` with an overall progress bar and a per-pass info
/// line. Thread-safe via internal `Arc<Mutex<...>>` so it can be cheaply
/// cloned and shared with a progress-consumer thread.
#[derive(Clone)]
pub struct WipeProgressDisplay {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    multi: MultiProgress,
    overall_bar: ProgressBar,
    pass_bar: ProgressBar,
    verify_bar: Option<ProgressBar>,
    total_bytes: u64,
    total_passes: u32,
    session_start: Instant,
}

impl WipeProgressDisplay {
    /// Create a new progress display for a wipe session.
    ///
    /// * `total_bytes` - Total bytes per pass (device capacity).
    /// * `total_passes` - Number of wipe passes (0 for verification-only).
    pub fn new(total_bytes: u64, total_passes: u32) -> Self {
        let multi = MultiProgress::new();

        // Overall progress bar.
        let overall_style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
             {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})"
        )
        .unwrap()
        .progress_chars("=>-");

        let overall_bar = multi.add(ProgressBar::new(total_bytes));
        overall_bar.set_style(overall_style);

        // Per-pass info bar (shows current pass status as a message).
        let pass_style = ProgressStyle::with_template(
            "  {prefix} {msg}"
        )
        .unwrap();

        let pass_bar = multi.add(ProgressBar::new(0));
        pass_bar.set_style(pass_style);
        if total_passes > 0 {
            pass_bar.set_prefix(format!("Pass 1/{total_passes}:"));
            pass_bar.set_message("starting...");
        } else {
            pass_bar.set_prefix("Verifying:");
            pass_bar.set_message("starting...");
        }

        Self {
            inner: Arc::new(Mutex::new(Inner {
                multi,
                overall_bar,
                pass_bar,
                verify_bar: None,
                total_bytes,
                total_passes,
                session_start: Instant::now(),
            })),
        }
    }

    /// Process a `ProgressEvent` and update the display accordingly.
    pub fn update(&self, event: &ProgressEvent) {
        let mut inner = self.inner.lock().unwrap();

        match event {
            ProgressEvent::SessionStarted {
                total_bytes,
                total_passes,
                method_name,
                ..
            } => {
                inner.total_bytes = *total_bytes;
                inner.total_passes = *total_passes;
                inner.overall_bar.set_length(*total_bytes);
                inner.pass_bar.set_message(format!("method: {method_name}"));
                inner.session_start = Instant::now();
            }

            ProgressEvent::PassStarted {
                pass_number,
                pass_name,
                ..
            } => {
                // Reset the overall bar for the new pass.
                inner.overall_bar.set_position(0);
                inner.overall_bar.set_length(inner.total_bytes);
                inner
                    .pass_bar
                    .set_prefix(format!("Pass {}/{}", pass_number, inner.total_passes));
                inner
                    .pass_bar
                    .set_message(format!("{pass_name} -- writing..."));
            }

            ProgressEvent::BlockWritten {
                bytes_written,
                total_bytes,
                throughput_bps,
                ..
            } => {
                inner.overall_bar.set_position(*bytes_written);

                let pct = if *total_bytes > 0 {
                    (*bytes_written as f64 / *total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                let throughput_str = if *throughput_bps > 0.0 {
                    drivewipe_core::types::format_throughput(*throughput_bps)
                } else {
                    "calculating...".to_string()
                };

                inner.pass_bar.set_message(format!(
                    "{:.1}% @ {}",
                    pct, throughput_str,
                ));
            }

            ProgressEvent::PassCompleted {
                duration_secs,
                throughput_mbps,
                ..
            } => {
                inner.overall_bar.set_position(inner.total_bytes);
                inner.pass_bar.set_message(format!(
                    "completed in {:.1}s ({:.1} MiB/s)",
                    duration_secs, throughput_mbps,
                ));
            }

            ProgressEvent::VerificationStarted { .. } => {
                // Add a verification progress bar.
                let verify_style = ProgressStyle::with_template(
                    "  {prefix} [{bar:40.green/white}] {bytes}/{total_bytes} ({bytes_per_sec})"
                )
                .unwrap()
                .progress_chars("=>-");

                let verify_bar = inner.multi.add(ProgressBar::new(inner.total_bytes));
                verify_bar.set_style(verify_style);
                verify_bar.set_prefix("Verify:");
                inner.verify_bar = Some(verify_bar);

                inner.pass_bar.set_message("verification started...");
            }

            ProgressEvent::VerificationProgress {
                bytes_verified,
                ..
            } => {
                if let Some(ref bar) = inner.verify_bar {
                    bar.set_position(*bytes_verified);
                }
            }

            ProgressEvent::VerificationCompleted {
                passed,
                duration_secs,
                ..
            } => {
                if let Some(ref bar) = inner.verify_bar {
                    bar.finish_and_clear();
                }
                let status = if *passed { "PASSED" } else { "FAILED" };
                inner.pass_bar.set_message(format!(
                    "verification {status} ({:.1}s)",
                    duration_secs,
                ));
            }

            ProgressEvent::FirmwareEraseStarted { method_name, .. } => {
                inner.pass_bar.set_prefix("Firmware Erase:");
                inner
                    .pass_bar
                    .set_message(format!("{method_name} -- in progress..."));
                inner.overall_bar.set_position(0);
            }

            ProgressEvent::FirmwareEraseProgress { percent, .. } => {
                let pos = ((*percent as f64 / 100.0) * inner.total_bytes as f64) as u64;
                inner.overall_bar.set_position(pos);
                inner
                    .pass_bar
                    .set_message(format!("{:.0}% complete", percent));
            }

            ProgressEvent::FirmwareEraseCompleted { duration_secs, .. } => {
                inner.overall_bar.set_position(inner.total_bytes);
                inner.pass_bar.set_message(format!(
                    "firmware erase completed in {:.1}s",
                    duration_secs,
                ));
            }

            ProgressEvent::Warning { message, .. } => {
                inner.pass_bar.set_message(format!("WARNING: {message}"));
            }

            ProgressEvent::Error { message, .. } => {
                inner.pass_bar.set_message(format!("ERROR: {message}"));
            }

            ProgressEvent::Interrupted { reason, .. } => {
                inner
                    .pass_bar
                    .set_message(format!("INTERRUPTED: {reason}"));
            }

            ProgressEvent::Completed {
                outcome,
                duration_secs,
                ..
            } => {
                inner.overall_bar.finish();
                inner.pass_bar.set_message(format!(
                    "{outcome} -- total time: {:.1}s",
                    duration_secs,
                ));
            }
        }
    }

    /// Signal that the display is complete; clear spinners.
    pub fn finish(&self) {
        let inner = self.inner.lock().unwrap();
        inner.overall_bar.finish_and_clear();
        inner.pass_bar.finish_and_clear();
        if let Some(ref bar) = inner.verify_bar {
            bar.finish_and_clear();
        }
    }
}
