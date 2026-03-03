//! Suspend/resume cycle to unfreeze ATA drives.
//!
//! When a system boots, the BIOS typically sends SECURITY FREEZE LOCK to all
//! ATA drives, putting them in the "Frozen" state. This prevents any security
//! commands (including Secure Erase) from being issued.
//!
//! The standard workaround is to trigger a system suspend (S3 sleep) and
//! immediately resume. During resume, the drives reset to an unfrozen state
//! but the BIOS freeze-lock sequence is not re-executed.
//!
//! This module writes `"mem"` to `/sys/power/state` to initiate the cycle.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use drivewipe_core::error::{DriveWipeError, Result};
use log;

/// Check if any SATA drives are currently in the frozen state.
/// Reads ATA security from IDENTIFY DEVICE for each sd* device.
pub fn any_drives_frozen() -> bool {
    let Ok(entries) = fs::read_dir("/sys/block") else {
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("sd") {
            continue;
        }
        // Skip partitions.
        if name_str.len() > 3 && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let dev_path = format!("/dev/{name_str}");
        if let Ok(info) = crate::ata_security::query_ata_security(&dev_path) {
            if info.frozen {
                log::info!("Drive {} is frozen", dev_path);
                return true;
            }
        }
    }

    false
}

/// Perform a suspend/resume cycle to unfreeze all ATA drives.
///
/// This writes `"mem"` to `/sys/power/state`, which triggers an ACPI S3 sleep.
/// The system will suspend and immediately resume (on most hardware).
///
/// # Safety
///
/// - Requires root privileges.
/// - The system will briefly suspend. All running processes are paused.
/// - On some hardware, resume may fail (requiring manual power button press).
/// - USB devices may need re-enumeration after resume.
pub fn unfreeze_drives() -> Result<()> {
    let power_state = Path::new("/sys/power/state");
    if !power_state.exists() {
        return Err(DriveWipeError::LiveEnvironmentRequired(
            "Cannot unfreeze: /sys/power/state not available".to_string(),
        ));
    }

    // Verify suspend is supported.
    let supported = fs::read_to_string(power_state).unwrap_or_default();
    if !supported.contains("mem") {
        return Err(DriveWipeError::LiveEnvironmentRequired(
            "System does not support S3 suspend (required for drive unfreeze)".to_string(),
        ));
    }

    log::info!("Initiating suspend/resume cycle to unfreeze drives...");

    // Sync filesystems before suspend.
    unsafe {
        libc::sync();
    }

    // Brief delay to let sync complete.
    thread::sleep(Duration::from_millis(500));

    // Trigger suspend.
    fs::write(power_state, "mem").map_err(|e| {
        DriveWipeError::LiveEnvironmentRequired(format!(
            "Failed to write to /sys/power/state: {e} (are you root?)"
        ))
    })?;

    // If we get here, the system has resumed.
    log::info!("System resumed from suspend");

    // Wait for devices to re-enumerate.
    thread::sleep(Duration::from_secs(2));

    // Trigger udev to re-discover devices.
    let _ = std::process::Command::new("udevadm")
        .args(["trigger"])
        .status();
    let _ = std::process::Command::new("udevadm")
        .args(["settle", "--timeout=10"])
        .status();

    // Verify drives are unfrozen.
    thread::sleep(Duration::from_secs(1));
    if any_drives_frozen() {
        log::warn!("Some drives are still frozen after suspend/resume cycle");
        return Err(DriveWipeError::AtaSecurityFrozen);
    }

    log::info!("All drives unfrozen successfully");
    Ok(())
}

/// Auto-detect frozen drives and unfreeze if any found.
/// Returns the number of drives that were frozen.
pub fn auto_unfreeze() -> Result<u32> {
    let frozen_count = count_frozen_drives();
    if frozen_count == 0 {
        log::info!("No frozen drives detected, skipping unfreeze");
        return Ok(0);
    }

    log::info!(
        "{} frozen drive(s) detected, initiating unfreeze cycle",
        frozen_count
    );
    unfreeze_drives()?;
    Ok(frozen_count)
}

/// Count how many SATA drives are currently frozen.
fn count_frozen_drives() -> u32 {
    let Ok(entries) = fs::read_dir("/sys/block") else {
        return 0;
    };

    let mut count = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("sd") {
            continue;
        }
        if name_str.len() > 3 && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let dev_path = format!("/dev/{name_str}");
        if let Ok(info) = crate::ata_security::query_ata_security(&dev_path) {
            if info.frozen {
                count += 1;
            }
        }
    }
    count
}
