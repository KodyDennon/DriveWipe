use std::path::Path;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result, bail};
use crossbeam_channel;
use uuid::Uuid;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive::create_enumerator;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::resume::WipeState;
use drivewipe_core::session::{CancellationToken, WipeSession};
use drivewipe_core::types::*;
use drivewipe_core::wipe::WipeMethodRegistry;

use crate::progress::WipeProgressDisplay;

/// Execute `drivewipe resume`.
pub fn run(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    list: bool,
    session_id_str: Option<&str>,
    auto: bool,
) -> Result<()> {
    let sessions_dir = config.sessions_dir();

    if list {
        return list_incomplete(sessions_dir);
    }

    if let Some(sid) = session_id_str {
        return resume_session(config, cancel_token, sessions_dir, sid);
    }

    if auto {
        return auto_resume(config, cancel_token, sessions_dir);
    }

    // Default: show incomplete sessions if no flag given.
    list_incomplete(sessions_dir)
}

/// List all incomplete wipe sessions found in the sessions directory.
fn list_incomplete(sessions_dir: &Path) -> Result<()> {
    let states = WipeState::find_incomplete(sessions_dir)
        .context("Failed to scan for incomplete sessions")?;

    if states.is_empty() {
        println!("No incomplete sessions found.");
        return Ok(());
    }

    println!("{}", console::style("=== Incomplete Sessions ===").bold());
    println!(
        "  {:<38} {:<18} {:<12} {:<10} {}",
        console::style("SESSION ID").bold(),
        console::style("DEVICE").bold(),
        console::style("METHOD").bold(),
        console::style("PROGRESS").bold(),
        console::style("LAST UPDATED").bold(),
    );
    println!("  {}", "-".repeat(100));

    for state in &states {
        let pct = if state.device_capacity > 0 {
            (state.total_bytes_written as f64 / state.device_capacity as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "  {:<38} {:<18} {:<12} {:>5.1}%     {}",
            state.session_id,
            state.device_path.display(),
            state.method_id,
            pct,
            state.last_updated.format("%Y-%m-%d %H:%M"),
        );
    }

    println!();
    println!("  {} session(s) found.", states.len());
    println!(
        "  Resume with: drivewipe resume --session <SESSION_ID>"
    );

    Ok(())
}

/// Resume a specific session by its UUID string.
fn resume_session(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    sessions_dir: &Path,
    session_id_str: &str,
) -> Result<()> {
    let session_id: Uuid = session_id_str
        .parse()
        .with_context(|| format!("Invalid session ID: {session_id_str}"))?;

    let state_path = WipeState::state_path(sessions_dir, session_id);
    if !state_path.exists() {
        bail!(
            "No state file found for session {session_id}. \
             Use `drivewipe resume --list` to see available sessions."
        );
    }

    let state = WipeState::load(&state_path)
        .with_context(|| format!("Failed to load session state from {}", state_path.display()))?;

    println!(
        "{} Resuming session {} (pass {}/{}, device {})",
        console::style("==>").green().bold(),
        state.session_id,
        state.current_pass,
        state.total_passes,
        state.device_path.display(),
    );

    execute_resumed_session(config, cancel_token, state)
}

/// Auto-resume: scan for incomplete sessions, match by device serial, and
/// offer to resume each one.
fn auto_resume(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    sessions_dir: &Path,
) -> Result<()> {
    let states = WipeState::find_incomplete(sessions_dir)
        .context("Failed to scan for incomplete sessions")?;

    if states.is_empty() {
        println!("No incomplete sessions found for auto-resume.");
        return Ok(());
    }

    let enumerator = create_enumerator();
    let current_drives = enumerator.enumerate().unwrap_or_default();

    let mut matched = Vec::new();
    for state in &states {
        // Try to match the saved device serial with a currently attached drive.
        let drive_present = current_drives
            .iter()
            .any(|d| d.serial == state.device_serial);

        if drive_present {
            matched.push(state.clone());
        } else {
            println!(
                "  {} Session {} -- device serial {} not currently attached, skipping.",
                console::style("skip:").dim(),
                state.session_id,
                state.device_serial,
            );
        }
    }

    if matched.is_empty() {
        println!("No matching devices found for auto-resume.");
        return Ok(());
    }

    println!(
        "{} Found {} resumable session(s) with matching devices.\n",
        console::style("==>").green().bold(),
        matched.len(),
    );

    for state in matched {
        if cancel_token.is_cancelled() {
            println!("Auto-resume cancelled by user.");
            return Ok(());
        }

        let pct = if state.device_capacity > 0 {
            (state.total_bytes_written as f64 / state.device_capacity as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "  Resuming session {} ({}, pass {}/{}, {:.1}% complete)...",
            state.session_id,
            state.device_path.display(),
            state.current_pass,
            state.total_passes,
            pct,
        );

        match execute_resumed_session(config, cancel_token, state) {
            Ok(()) => {
                println!(
                    "  {} Session completed successfully.\n",
                    console::style("ok:").green().bold(),
                );
            }
            Err(e) => {
                eprintln!(
                    "  {} Resume failed: {e}\n",
                    console::style("error:").red().bold(),
                );
            }
        }
    }

    Ok(())
}

/// Execute a wipe session from a loaded `WipeState`.
fn execute_resumed_session(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    state: WipeState,
) -> Result<()> {
    let device_path = &state.device_path;

    // Re-inspect the device.
    let enumerator = create_enumerator();
    let drive_info = enumerator
        .inspect(device_path)
        .with_context(|| format!("Failed to inspect device {}", device_path.display()))?;

    // Validate serial matches.
    if drive_info.serial != state.device_serial {
        bail!(
            "Device serial mismatch: expected {}, found {}. \
             The drive may have been swapped.",
            state.device_serial,
            drive_info.serial,
        );
    }

    // Look up the wipe method.
    let method_id = &state.method_id;
    let registry = WipeMethodRegistry::new();
    if registry.get(method_id).is_none() {
        bail!("Unknown wipe method from saved state: {method_id}");
    }

    // Open the device.
    #[cfg(target_os = "linux")]
    let mut device_io =
        drivewipe_core::io::linux::LinuxDeviceIo::open(device_path)
            .with_context(|| format!("Failed to open device {}", device_path.display()))?;

    #[cfg(target_os = "macos")]
    let mut device_io =
        drivewipe_core::io::macos::MacosDeviceIo::open(device_path)
            .with_context(|| format!("Failed to open device {}", device_path.display()))?;

    #[cfg(target_os = "windows")]
    let mut device_io =
        drivewipe_core::io::windows::WindowsDeviceIo::open(device_path)
            .with_context(|| format!("Failed to open device {}", device_path.display()))?;

    // Build the session using the MethodProxy from wipe command.
    let mut session_config = config.clone();
    session_config.auto_verify = state.verify_after;

    let session = {
        let method_box = super::wipe::find_and_clone_method_by_id(method_id)
            .ok_or_else(|| anyhow::anyhow!("Method {method_id} not found in registry"))?;
        WipeSession::new(drive_info.clone(), method_box, session_config)
    };

    // Progress display.
    let progress_display = WipeProgressDisplay::new(
        drive_info.capacity,
        state.total_passes,
    );

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressEvent>();

    let display_handle = {
        let pd = progress_display.clone();
        thread::spawn(move || {
            while let Ok(event) = progress_rx.recv() {
                pd.update(&event);
            }
        })
    };

    // Execute.
    let result = session.execute(
        &mut device_io,
        &progress_tx,
        cancel_token,
        Some(state),
    );

    drop(progress_tx);
    let _ = display_handle.join();
    progress_display.finish();

    let wipe_result = result.context("Resumed wipe operation failed")?;

    println!();
    println!(
        "  Outcome: {}",
        match wipe_result.outcome {
            WipeOutcome::Success => console::style("Success".to_string()).green().bold(),
            WipeOutcome::SuccessWithWarnings => {
                console::style("Success (with warnings)".to_string()).yellow().bold()
            }
            _ => console::style(format!("{}", wipe_result.outcome)).red().bold(),
        }
    );

    // Save JSON report.
    let report_dir = config.sessions_dir();
    std::fs::create_dir_all(report_dir)
        .context("Failed to create report directory")?;

    let json_path = report_dir.join(format!("{}.json", wipe_result.session_id));
    let generator = drivewipe_core::report::json::JsonReportGenerator;
    let report_bytes = drivewipe_core::report::ReportGenerator::generate(&generator, &wipe_result)
        .context("Failed to generate JSON report")?;
    std::fs::write(&json_path, &report_bytes)
        .with_context(|| format!("Failed to write report to {}", json_path.display()))?;

    println!(
        "  {} JSON report saved to {}",
        console::style("report:").blue().bold(),
        json_path.display(),
    );

    match wipe_result.outcome {
        WipeOutcome::Success | WipeOutcome::SuccessWithWarnings => Ok(()),
        _ => bail!("Resumed wipe ended with outcome: {}", wipe_result.outcome),
    }
}
