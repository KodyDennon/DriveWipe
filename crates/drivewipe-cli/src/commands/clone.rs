use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use drivewipe_core::clone::{CloneConfig, CloneMode, CompressionMode};
use drivewipe_core::config::DriveWipeConfig;
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
        _ => {
            anyhow::bail!("Unknown clone mode: {}. Use 'block' or 'partition'.", mode);
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
        verify: true,
        block_size: 4 * 1024 * 1024,
    };

    println!("Clone operation:");
    println!("  Source: {}", source);
    println!("  Target: {}", target);
    println!("  Mode: {:?}", clone_mode);
    println!("  Compression: {:?}", compression);
    println!("  Encrypt: {}", encrypt);

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded();

    // Open source and target devices
    let mut source_device = drivewipe_core::io::open_device(&PathBuf::from(source), false)
        .context("Failed to open source device")?;
    let mut target_device = drivewipe_core::io::open_device(&PathBuf::from(target), true)
        .context("Failed to open target device")?;

    let result = match clone_mode {
        CloneMode::Block => drivewipe_core::clone::block::clone_block(
            source_device.as_mut(),
            target_device.as_mut(),
            &clone_config,
            &progress_tx,
            cancel_token,
        ).await,
        CloneMode::Partition => drivewipe_core::clone::partition_aware::clone_partition_aware(
            source_device.as_mut(),
            target_device.as_mut(),
            &clone_config,
            &progress_tx,
            cancel_token,
        ).await,
    };

    // Drain progress events
    drop(progress_tx);
    for _event in progress_rx {}

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
