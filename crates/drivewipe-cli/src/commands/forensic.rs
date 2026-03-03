use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;
use drivewipe_core::forensic::{ForensicConfig, ForensicResult, ForensicSession};
use drivewipe_core::session::CancellationToken;

/// Run the `forensic scan` subcommand.
pub fn scan(
    _config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    device: &str,
) -> Result<()> {
    let result = execute_scan(cancel_token, device)?;
    print_results(&result);
    Ok(())
}

/// Execute a forensic scan and return the result.
fn execute_scan(
    cancel_token: &Arc<CancellationToken>,
    device: &str,
) -> Result<ForensicResult> {
    let enumerator = drive::create_enumerator();
    let drive_info = enumerator
        .inspect(&PathBuf::from(device))
        .context("Failed to inspect device")?;

    println!("Forensic scan: {} {}", drive_info.model, drive_info.serial);

    let forensic_config = ForensicConfig::default();
    let session = ForensicSession::new(forensic_config);

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded();

    let mut device_io = drivewipe_core::io::open_device(&PathBuf::from(device), false)
        .context("Failed to open device")?;

    let result = session.execute(
        device_io.as_mut(),
        device,
        &drive_info.serial,
        &progress_tx,
        cancel_token,
    ).context("Forensic scan failed")?;

    // Drain progress
    drop(progress_tx);
    for _event in progress_rx {}

    Ok(result)
}

/// Print forensic results to stdout.
fn print_results(result: &ForensicResult) {
    println!("\nForensic Analysis Results:");
    println!("  Duration: {:.1}s", result.duration_secs);

    if let Some(ref entropy) = result.entropy_stats {
        println!("\n  Entropy Analysis:");
        println!("    Average: {:.2} bits/byte", entropy.average_entropy);
        println!("    Range: {:.2} - {:.2}", entropy.min_entropy, entropy.max_entropy);
        println!("    High entropy sectors: {:.1}%", entropy.high_entropy_pct);
        println!("    Zero sectors: {:.1}%", entropy.zero_pct);
    }

    if !result.signature_hits.is_empty() {
        println!("\n  File Signatures Found: {}", result.signature_hits.len());
        for hit in result.signature_hits.iter().take(20) {
            println!("    {} at offset {:#x}", hit.file_type, hit.offset);
        }
        if result.signature_hits.len() > 20 {
            println!("    ... and {} more", result.signature_hits.len() - 20);
        }
    } else {
        println!("\n  File Signatures: None found");
    }

    if let Some(ref sampling) = result.sampling_result {
        println!("\n  Statistical Sampling:");
        println!("    Sectors sampled: {}", sampling.sectors_sampled);
        println!("    Zero: {:.1}%", sampling.zero_pct);
        println!("    High entropy: {:.1}%", sampling.high_entropy_pct);
        println!("    Data remnants: {:.1}%", sampling.data_remnant_pct);
        println!("    Confidence: {:.1}%", sampling.confidence * 100.0);
    }
}

/// Run the `forensic report` subcommand.
pub fn report(
    _config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    device: &str,
    output: Option<&str>,
) -> Result<()> {
    let result = execute_scan(cancel_token, device)?;
    print_results(&result);

    // Generate and save a formal forensic report
    let report = drivewipe_core::forensic::report::ForensicReport::generate(
        result,
        None,
        None,
    );

    let report_json = report.to_json()
        .context("Failed to serialize forensic report")?;

    let output_path = match output {
        Some(path) => PathBuf::from(path),
        None => {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("forensic_report_{}.json", timestamp))
        }
    };

    std::fs::write(&output_path, &report_json)
        .with_context(|| format!("Failed to write report to {}", output_path.display()))?;

    println!("\nForensic report saved to: {}", output_path.display());

    Ok(())
}
