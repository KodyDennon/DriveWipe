use std::path::PathBuf;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;
use drivewipe_core::health::snapshot::DriveHealthSnapshot;

/// Run the `health` subcommand.
pub fn run(
    config: &DriveWipeConfig,
    device: &str,
    save: bool,
    compare: Option<&str>,
) -> Result<()> {
    let enumerator = drive::create_enumerator();
    let drive_info = enumerator
        .inspect(&PathBuf::from(device))
        .context("Failed to inspect device")?;

    println!("Drive Health: {} {}", drive_info.model, drive_info.serial);
    println!("  Path: {}", drive_info.path.display());
    println!("  Capacity: {}", drive_info.capacity_display());
    println!(
        "  Type: {} / {}",
        drive_info.drive_type, drive_info.transport
    );

    if let Some(healthy) = drive_info.smart_healthy {
        println!(
            "  SMART Status: {}",
            if healthy { "Healthy" } else { "FAILING" }
        );
    } else {
        println!("  SMART Status: Not available");
    }

    // Create a basic snapshot from available info
    let snapshot = DriveHealthSnapshot {
        timestamp: chrono::Utc::now(),
        device_path: device.to_string(),
        device_serial: drive_info.serial.clone(),
        device_model: drive_info.model.clone(),
        smart_data: None,
        nvme_health: None,
        temperature_celsius: None,
        benchmark: None,
    };

    if save {
        let snapshot_path = config
            .sessions_dir
            .parent()
            .unwrap_or(&config.sessions_dir)
            .join("health")
            .join(format!(
                "{}_{}.json",
                drive_info.serial,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            ));

        snapshot
            .save(&snapshot_path)
            .context("Failed to save health snapshot")?;
        println!("\nSnapshot saved to: {}", snapshot_path.display());
    }

    // Compare with previous snapshot
    if let Some(compare_path) = compare {
        let previous = DriveHealthSnapshot::load(&PathBuf::from(compare_path))
            .context("Failed to load comparison snapshot")?;

        let comparison = drivewipe_core::health::HealthDiff::compare(&previous, &snapshot);
        println!("\nHealth Comparison (vs {}):", compare_path);
        println!("  Verdict: {:?}", comparison.verdict);
        for msg in &comparison.messages {
            println!("  - {}", msg);
        }
    }

    Ok(())
}
