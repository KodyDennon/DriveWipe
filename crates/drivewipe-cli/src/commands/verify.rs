use std::path::Path;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result, bail};
use crossbeam_channel;
use uuid::Uuid;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive::create_enumerator;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::session::CancellationToken;
use drivewipe_core::types::format_bytes;
use drivewipe_core::verify::Verifier;
use drivewipe_core::verify::pattern_verify::PatternVerifier;
use drivewipe_core::verify::zero_verify::ZeroVerifier;
use drivewipe_core::wipe::patterns::OneFill;

use crate::progress::WipeProgressDisplay;

/// Execute `drivewipe verify`.
pub async fn run(
    _config: &DriveWipeConfig,
    _cancel_token: &Arc<CancellationToken>,
    device: &str,
    pattern: &str,
) -> Result<()> {
    // ── Inspect the device ──────────────────────────────────────────────
    let enumerator = create_enumerator();
    let device_path = Path::new(device);
    let drive_info = enumerator
        .inspect(device_path)
        .await
        .with_context(|| format!("Failed to inspect device {device}"))?;

    println!(
        "{} Verifying {} ({}, {})...",
        console::style("==>").green().bold(),
        drive_info.path.display(),
        drive_info.model,
        format_bytes(drive_info.capacity),
    );
    println!("  Expected pattern: {pattern}");
    println!();

    // ── Open the device ─────────────────────────────────────────────────
    #[cfg(target_os = "linux")]
    let mut device_io = drivewipe_core::io::linux::LinuxDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    #[cfg(target_os = "macos")]
    let mut device_io = drivewipe_core::io::macos::MacosDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    #[cfg(target_os = "windows")]
    let mut device_io = drivewipe_core::io::windows::WindowsDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    // ── Build the verifier ──────────────────────────────────────────────
    let verifier: Box<dyn Verifier> = match pattern {
        "zero" => Box::new(ZeroVerifier),
        "one" => Box::new(PatternVerifier::new(Box::new(OneFill))),
        "random" => {
            bail!(
                "Cannot verify random pattern -- there is no way to reproduce \
                 the original random stream without the session state. \
                 Use `drivewipe resume --list` to check session data."
            );
        }
        other => {
            bail!("Unknown verification pattern: {other}. Supported: zero, one");
        }
    };

    // ── Progress display ────────────────────────────────────────────────
    let progress_display = WipeProgressDisplay::new(drive_info.capacity, 0);

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressEvent>();

    let display_handle = {
        let pd = progress_display.clone();
        tokio::task::spawn_blocking(move || {
            while let Ok(event) = progress_rx.recv() {
                pd.update(&event);
            }
        })
    };

    // ── Run verification ────────────────────────────────────────────────
    let session_id = Uuid::new_v4();
    let result = verifier.verify(&mut device_io, session_id, &progress_tx).await;

    // Drop sender so the display thread terminates.
    drop(progress_tx);
    let _ = display_handle.await;
    progress_display.finish();

    println!();

    match result {
        Ok(true) => {
            println!(
                "{} Verification PASSED -- entire device matches expected pattern ({pattern}).",
                console::style("PASS").green().bold(),
            );
            Ok(())
        }
        Ok(false) => {
            println!(
                "{} Verification FAILED -- device contents do not match expected pattern.",
                console::style("FAIL").red().bold(),
            );
            bail!("Verification failed");
        }
        Err(e) => {
            println!(
                "{} Verification FAILED with error: {e}",
                console::style("FAIL").red().bold(),
            );
            Err(e).context("Verification error")
        }
    }
}
