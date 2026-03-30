use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};

use drivewipe_core::clone::{CloneConfig, CloneMode, CompressionMode};
use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::session::CancellationToken;

/// Run the `clone` subcommand.
pub async fn run(
    _config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    source: &str,
    target: &str,
    mode: &str,
    compress: bool,
    encrypt: bool,
) -> Result<()> {
    let clone_mode = match mode {
        "block" => CloneMode::Block,
        "partition" => CloneMode::Partition,
        "image" => CloneMode::Image,
        _ => {
            anyhow::bail!(
                "Unknown clone mode: {}. Use 'block', 'partition', or 'image'.",
                mode
            );
        }
    };

    let compression = if compress {
        CompressionMode::Zstd
    } else {
        CompressionMode::None
    };

    let clone_config = CloneConfig {
        source: PathBuf::from(source),
        target: PathBuf::from(target),
        mode: clone_mode,
        compression,
        encrypt,
        password: None,
        verify: true,
        block_size: 4 * 1024 * 1024,
        bandwidth_limit_bps: None,
    };

    println!("Clone operation:");
    println!("  Source: {}", source);
    println!("  Target: {}", target);
    println!("  Mode: {:?}", clone_mode);
    println!("  Compression: {:?}", compression);
    println!("  Encrypt: {}", encrypt);

    // Confirmation prompt: cloning overwrites the target.
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "This will overwrite all data on the target ({target}). Continue?"
        ))
        .default(false)
        .interact()?;

    if !confirmed {
        println!("Clone aborted by user.");
        return Ok(());
    }

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded();

    // Spawn a thread to consume progress events and display a progress bar.
    let progress_handle = std::thread::spawn(move || {
        let bar = ProgressBar::new(0);
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                 {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})",
            )
            .unwrap()
            .progress_chars("=>-"),
        );

        for event in progress_rx {
            match event {
                ProgressEvent::CloneStarted { total_bytes, .. } => {
                    bar.set_length(total_bytes);
                    bar.set_position(0);
                }
                ProgressEvent::CloneProgress {
                    bytes_copied,
                    total_bytes,
                    ..
                } => {
                    bar.set_length(total_bytes);
                    bar.set_position(bytes_copied);
                }
                ProgressEvent::CloneCompleted { .. } => {
                    bar.finish_with_message("done");
                }
                ProgressEvent::Error { message, .. } => {
                    bar.abandon_with_message(format!("ERROR: {message}"));
                }
                _ => {}
            }
        }

        bar.finish_and_clear();
    });

    let result = match clone_mode {
        CloneMode::Block => {
            let mut source_device = drivewipe_core::io::open_device(&PathBuf::from(source), false)
                .context("Failed to open source device")?;
            let mut target_device = drivewipe_core::io::open_device(&PathBuf::from(target), true)
                .context("Failed to open target device")?;
            drivewipe_core::clone::block::clone_block(
                source_device.as_mut(),
                target_device.as_mut(),
                &clone_config,
                &progress_tx,
                cancel_token,
            )
            .await
        }
        CloneMode::Partition => {
            let mut source_device = drivewipe_core::io::open_device(&PathBuf::from(source), false)
                .context("Failed to open source device")?;
            let mut target_device = drivewipe_core::io::open_device(&PathBuf::from(target), true)
                .context("Failed to open target device")?;
            drivewipe_core::clone::partition_aware::clone_partition_aware(
                source_device.as_mut(),
                target_device.as_mut(),
                &clone_config,
                &progress_tx,
                cancel_token,
            )
            .await
        }
        CloneMode::Image => {
            // Determine if we are creating or restoring
            let source_path = PathBuf::from(source);
            let target_path = PathBuf::from(target);

            if target_path.extension().and_then(|s| s.to_str()) == Some("dwc")
                || (!target_path.exists() || target_path.is_file())
            {
                // Clone to image
                let mut source_device = drivewipe_core::io::open_device(&source_path, false)
                    .context("Failed to open source device")?;
                drivewipe_core::clone::ops::clone_device_to_image(
                    source_device.as_mut(),
                    &target_path,
                    &clone_config,
                    &progress_tx,
                    cancel_token,
                )
                .await
            } else {
                // Restore from image
                let mut target_device = drivewipe_core::io::open_device(&target_path, true)
                    .context("Failed to open target device")?;
                drivewipe_core::clone::ops::restore_image_to_device(
                    &source_path,
                    target_device.as_mut(),
                    &clone_config,
                    &progress_tx,
                    cancel_token,
                )
                .await
            }
        }
    };

    // Drop the sender so the progress thread's recv loop terminates.
    drop(progress_tx);
    let _ = progress_handle.join();

    match result {
        Ok(result) => {
            println!("\nClone completed successfully!");
            println!(
                "  Bytes copied: {}",
                drivewipe_core::format_bytes(result.bytes_copied)
            );
            println!("  Duration: {:.1}s", result.duration_secs);
            println!("  Throughput: {:.1} MiB/s", result.throughput_mbps);
            if let Some(passed) = result.verification_passed {
                println!(
                    "  Verification: {}",
                    if passed { "PASSED" } else { "FAILED" }
                );
            }
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Clone failed: {}", e);
        }
    }
}
