use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use drivewipe_core::types::format_bytes;

use crate::app::App;
use crate::ui;

/// Draw the drive info popup as a centered overlay.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let selected = match app.table_state.selected() {
        Some(i) if i < app.drives.len() => i,
        _ => return,
    };

    let drive = &app.drives[selected];

    let popup_area = ui::centered_rect_percent(60, 75, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" Drive Info: {} ", drive.path.display()))
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Build all field values as owned Strings to avoid temporary borrow issues.
    let device_path = drive.path.display().to_string();
    let model = drive.model.clone();
    let serial = if drive.serial.is_empty() {
        "N/A".to_string()
    } else {
        drive.serial.clone()
    };
    let firmware = if drive.firmware_rev.is_empty() {
        "N/A".to_string()
    } else {
        drive.firmware_rev.clone()
    };
    let capacity_display = drive.capacity_display();
    let capacity_bytes = format!("{}", drive.capacity);
    let block_size = format!("{} bytes", drive.block_size);
    let physical_block = drive
        .physical_block_size
        .map(|pbs| format!("{pbs} bytes"));
    let drive_type = drive.drive_type.to_string();
    let transport = drive.transport.to_string();
    let is_boot = if drive.is_boot_drive { "Yes" } else { "No" };
    let is_removable = if drive.is_removable { "Yes" } else { "No" };
    let ata_security = drive.ata_security.to_string();
    let supports_trim = if drive.supports_trim { "Yes" } else { "No" };
    let is_sed = if drive.is_sed { "Yes" } else { "No" };
    let smart_healthy = match drive.smart_healthy {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "N/A",
    };
    let hpa_display = if drive.hidden_areas.hpa_enabled {
        format!(
            "Yes ({})",
            drive
                .hidden_areas
                .hpa_size
                .map(|s| format_bytes(s))
                .unwrap_or_else(|| "unknown size".into())
        )
    } else {
        "No".to_string()
    };
    let dco_display = if drive.hidden_areas.dco_enabled {
        format!(
            "Yes ({})",
            drive
                .hidden_areas
                .dco_size
                .map(|s| format_bytes(s))
                .unwrap_or_else(|| "unknown size".into())
        )
    } else {
        "No".to_string()
    };
    let partition_table = drive
        .partition_table
        .as_deref()
        .unwrap_or("None")
        .to_string();
    let partition_count = drive.partition_count.to_string();
    let firmware_erase = if drive.firmware_erase_likely_supported() {
        "Likely supported"
    } else {
        "Not supported"
    };
    let suggested_method = drive.suggested_method();

    let mut lines = Vec::new();

    let add_field = |lines: &mut Vec<Line>, label: &str, value: &str| {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {label:<22} "),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(value.to_string(), Style::default().fg(Color::White)),
        ]));
    };

    lines.push(Line::from(""));

    add_field(&mut lines, "Device Path:", &device_path);
    add_field(&mut lines, "Model:", &model);
    add_field(&mut lines, "Serial:", &serial);
    add_field(&mut lines, "Firmware:", &firmware);

    lines.push(Line::from(""));

    add_field(&mut lines, "Capacity:", &capacity_display);
    add_field(&mut lines, "Capacity (bytes):", &capacity_bytes);
    add_field(&mut lines, "Block Size:", &block_size);
    if let Some(ref pbs) = physical_block {
        add_field(&mut lines, "Physical Block Size:", pbs);
    }

    lines.push(Line::from(""));

    add_field(&mut lines, "Type:", &drive_type);
    add_field(&mut lines, "Transport:", &transport);
    add_field(&mut lines, "Boot Drive:", is_boot);
    add_field(&mut lines, "Removable:", is_removable);

    lines.push(Line::from(""));

    add_field(&mut lines, "ATA Security:", &ata_security);
    add_field(&mut lines, "TRIM Support:", supports_trim);
    add_field(&mut lines, "Self-Encrypting:", is_sed);
    add_field(&mut lines, "SMART Healthy:", smart_healthy);

    lines.push(Line::from(""));

    add_field(&mut lines, "HPA Enabled:", &hpa_display);
    add_field(&mut lines, "DCO Enabled:", &dco_display);

    lines.push(Line::from(""));

    add_field(&mut lines, "Partition Table:", &partition_table);
    add_field(&mut lines, "Partitions:", &partition_count);

    lines.push(Line::from(""));

    add_field(&mut lines, "Firmware Erase:", firmware_erase);
    add_field(&mut lines, "Suggested Method:", suggested_method);

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
