use console::style;

use drivewipe_core::types::{
    AtaSecurityState, DriveInfo, DriveType, Transport, format_bytes,
};

/// Print a summary table of all detected drives.
///
/// Columns: Device, Model, Serial, Capacity, Type, Transport, Boot, Suggested Method
pub fn print_drive_table(drives: &[DriveInfo]) {
    // ── Column widths ───────────────────────────────────────────────────
    let col_dev = 18;
    let col_model = 24;
    let col_serial = 22;
    let col_cap = 12;
    let col_type = 7;
    let col_trans = 9;
    let col_boot = 6;
    let col_method = 22;

    // ── Header ──────────────────────────────────────────────────────────
    println!(
        "  {:<col_dev$} {:<col_model$} {:<col_serial$} {:>col_cap$} {:<col_type$} {:<col_trans$} {:<col_boot$} {}",
        style("DEVICE").bold(),
        style("MODEL").bold(),
        style("SERIAL").bold(),
        style("CAPACITY").bold(),
        style("TYPE").bold(),
        style("TRANSPORT").bold(),
        style("BOOT").bold(),
        style("SUGGESTED METHOD").bold(),
    );

    let total_width = col_dev + col_model + col_serial + col_cap + col_type + col_trans + col_boot + col_method + 7;
    println!("  {}", "-".repeat(total_width));

    // ── Rows ────────────────────────────────────────────────────────────
    for drive in drives {
        let device = drive.path.display().to_string();
        let model = truncate(&drive.model, col_model - 1);
        let serial = truncate(&drive.serial, col_serial - 1);
        let capacity = format_bytes(drive.capacity);
        let drive_type = format!("{}", drive.drive_type);
        let transport = format!("{}", drive.transport);
        let boot = if drive.is_boot_drive { "YES" } else { "no" };
        let suggested = drive.suggested_method();

        // Apply colours.
        let device_styled = if drive.is_boot_drive {
            style(device.clone()).red()
        } else {
            style(device.clone())
        };

        let boot_styled = if drive.is_boot_drive {
            style(boot.to_string()).red().bold()
        } else {
            style(boot.to_string()).dim()
        };

        let transport_styled = match drive.transport {
            Transport::Usb => style(transport.clone()).yellow(),
            Transport::Nvme => style(transport.clone()).cyan(),
            _ => style(transport.clone()),
        };

        let type_styled = match drive.drive_type {
            DriveType::Ssd | DriveType::Nvme => style(drive_type.clone()).cyan(),
            DriveType::Hdd => style(drive_type.clone()).green(),
            _ => style(drive_type.clone()).dim(),
        };

        let health_styled = match drive.smart_healthy {
            Some(true) => style(suggested.to_string()).green(),
            Some(false) => style(suggested.to_string()).red(),
            None => style(suggested.to_string()),
        };

        println!(
            "  {:<col_dev$} {:<col_model$} {:<col_serial$} {:>col_cap$} {:<col_type$} {:<col_trans$} {:<col_boot$} {}",
            device_styled,
            model,
            serial,
            capacity,
            type_styled,
            transport_styled,
            boot_styled,
            health_styled,
        );
    }

    println!();
    println!("  {} drive(s) detected.", drives.len());
}

/// Print detailed information about a single drive.
pub fn print_drive_info(drive: &DriveInfo) {
    println!("{}", style("=== Drive Information ===").bold());
    println!();

    // ── Identity ────────────────────────────────────────────────────────
    println!("  {:<20} {}", style("Device Path:").bold(), drive.path.display());
    println!("  {:<20} {}", style("Model:").bold(), drive.model);
    println!("  {:<20} {}", style("Serial:").bold(), drive.serial);
    println!("  {:<20} {}", style("Firmware Rev:").bold(), drive.firmware_rev);
    println!();

    // ── Capacity ────────────────────────────────────────────────────────
    println!("  {:<20} {} ({} bytes)", style("Capacity:").bold(), format_bytes(drive.capacity), drive.capacity);
    println!("  {:<20} {} bytes", style("Block Size:").bold(), drive.block_size);
    if let Some(pbs) = drive.physical_block_size {
        println!("  {:<20} {} bytes", style("Phys Block Size:").bold(), pbs);
    }
    println!();

    // ── Type / Transport ────────────────────────────────────────────────
    println!("  {:<20} {}", style("Drive Type:").bold(), drive.drive_type);
    println!("  {:<20} {}", style("Transport:").bold(), drive.transport);
    println!(
        "  {:<20} {}",
        style("Removable:").bold(),
        if drive.is_removable { "Yes" } else { "No" }
    );
    println!();

    // ── Boot / System ───────────────────────────────────────────────────
    let boot_display = if drive.is_boot_drive {
        style("YES -- this is the boot/system drive".to_string()).red().bold()
    } else {
        style("No".to_string()).green()
    };
    println!("  {:<20} {}", style("Boot Drive:").bold(), boot_display);
    println!();

    // ── ATA Security ────────────────────────────────────────────────────
    let ata_styled = match drive.ata_security {
        AtaSecurityState::Frozen => style(format!("{}", drive.ata_security)).yellow().bold(),
        AtaSecurityState::Locked => style(format!("{}", drive.ata_security)).red().bold(),
        AtaSecurityState::NotSupported => style(format!("{}", drive.ata_security)).dim(),
        _ => style(format!("{}", drive.ata_security)),
    };
    println!("  {:<20} {}", style("ATA Security:").bold(), ata_styled);
    println!();

    // ── Hidden Areas ────────────────────────────────────────────────────
    println!("  {:<20} {}", style("HPA Enabled:").bold(), drive.hidden_areas.hpa_enabled);
    if let Some(hpa_size) = drive.hidden_areas.hpa_size {
        println!("  {:<20} {}", style("HPA Size:").bold(), format_bytes(hpa_size));
    }
    println!("  {:<20} {}", style("DCO Enabled:").bold(), drive.hidden_areas.dco_enabled);
    if let Some(dco_size) = drive.hidden_areas.dco_size {
        println!("  {:<20} {}", style("DCO Size:").bold(), format_bytes(dco_size));
    }
    println!();

    // ── Features ────────────────────────────────────────────────────────
    println!("  {:<20} {}", style("TRIM/UNMAP:").bold(), if drive.supports_trim { "Supported" } else { "Not supported" });
    println!("  {:<20} {}", style("SED (SED):").bold(), if drive.is_sed { "Yes" } else { "No" });
    println!();

    // ── SMART ───────────────────────────────────────────────────────────
    let smart_display = match drive.smart_healthy {
        Some(true) => style("Healthy".to_string()).green().bold(),
        Some(false) => style("UNHEALTHY".to_string()).red().bold(),
        None => style("Not available".to_string()).dim(),
    };
    println!("  {:<20} {}", style("SMART Health:").bold(), smart_display);
    println!();

    // ── Partitions ──────────────────────────────────────────────────────
    if let Some(ref pt) = drive.partition_table {
        println!("  {:<20} {}", style("Partition Table:").bold(), pt.to_uppercase());
    } else {
        println!("  {:<20} {}", style("Partition Table:").bold(), "None / Unknown");
    }
    println!("  {:<20} {}", style("Partition Count:").bold(), drive.partition_count);
    println!();

    // ── Firmware erase support ──────────────────────────────────────────
    println!(
        "  {:<20} {}",
        style("FW Erase:").bold(),
        if drive.firmware_erase_likely_supported() {
            style("Likely supported".to_string()).green()
        } else {
            style("Not likely supported".to_string()).dim()
        }
    );
    println!(
        "  {:<20} {}",
        style("Suggested Method:").bold(),
        style(drive.suggested_method()).cyan().bold()
    );
}

/// Truncate a string to `max_len` display characters, adding "..." if needed.
///
/// Uses `char_indices` to avoid panicking on multi-byte UTF-8 boundaries.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let end = s
            .char_indices()
            .nth(max_len - 3)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    } else {
        let end = s
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        s[..end].to_string()
    }
}
