use std::path::PathBuf;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;
use drivewipe_core::profile::ProfileDatabase;

/// Run the `profile` subcommand.
pub fn run(config: &DriveWipeConfig, device: &str) -> Result<()> {
    let enumerator = drive::create_enumerator();
    let drive_info = enumerator
        .inspect(&PathBuf::from(device))
        .context("Failed to inspect device")?;

    println!("Drive: {} {}", drive_info.model, drive_info.serial);

    let db =
        ProfileDatabase::load(&config.profiles_dir).context("Failed to load profile database")?;

    let matcher = db.into_matcher();

    if let Some(profile) = matcher.match_drive(&drive_info) {
        println!("\nMatched Profile: {}", profile.name);
        println!("  Manufacturer: {}", profile.manufacturer);
        if let Some(ref controller) = profile.controller_type {
            println!("  Controller: {}", controller);
        }
        println!(
            "  Over-provisioning: {:.0}%",
            profile.over_provisioning_ratio * 100.0
        );
        println!("  Sanitize support: {}", profile.sanitize_support);
        if let Some(ref method) = profile.recommended_method {
            println!("  Recommended method: {}", method);
        }
        if !profile.quirks.is_empty() {
            println!("  Quirks:");
            for quirk in &profile.quirks {
                println!("    - {}", quirk);
            }
        }
        println!("  Performance:");
        if let Some(write) = profile.performance.sequential_write_mbps {
            println!("    Sequential write: {:.0} MiB/s", write);
        }
        if let Some(read) = profile.performance.sequential_read_mbps {
            println!("    Sequential read: {:.0} MiB/s", read);
        }
        if profile.performance.has_slc_cache {
            println!("    SLC cache: yes");
            if let Some(size) = profile.performance.slc_cache_bytes {
                println!("    Cache size: {}", drivewipe_core::format_bytes(size));
            }
        }
    } else {
        println!("\nNo matching profile found for this drive.");
        println!(
            "Using generic defaults based on drive type: {}",
            drive_info.drive_type
        );
    }

    Ok(())
}
