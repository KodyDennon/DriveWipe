//! Live dashboard screen — system overview when running in live mode.
//!
//! Shows kernel/module version, CPU/RAM, all drives with HPA/DCO indicators,
//! network info, and hardware temperatures.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, Wrap};

use crate::app::App;
use crate::ui;

/// Draw the live dashboard screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Length(8), // System info
            Constraint::Min(10),   // Drive table
            Constraint::Length(5), // Network / status
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title bar with DRIVEWIPE LIVE branding
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "  DRIVEWIPE LIVE  ",
            Style::default().fg(Color::Black).bg(Color::Yellow).bold(),
        ),
        Span::raw("  "),
        Span::styled(
            "System Dashboard",
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(title, chunks[0]);

    // System info panel
    draw_system_info(frame, chunks[1], app);

    // Drive table with HPA/DCO indicators
    draw_drive_table(frame, chunks[2], app);

    // Network / status info
    draw_network_info(frame, chunks[3], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[4],
        &[
            ("q", "Back"),
            ("1", "HPA/DCO"),
            ("2", "ATA Security"),
            ("3", "Kernel Module"),
        ],
    );
}

fn draw_system_info(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" System Information ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Kernel and module info from live status
    for line in &app.live_status_lines {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(Color::Cyan),
        )));
    }

    // Drive count summary
    let sata_count = app
        .drives
        .iter()
        .filter(|d| d.transport == drivewipe_core::types::Transport::Sata)
        .count();
    let nvme_count = app
        .drives
        .iter()
        .filter(|d| d.transport == drivewipe_core::types::Transport::Nvme)
        .count();
    let usb_count = app
        .drives
        .iter()
        .filter(|d| d.transport == drivewipe_core::types::Transport::Usb)
        .count();

    lines.push(Line::from(vec![
        Span::styled("  Drives: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} SATA", sata_count),
            Style::default().fg(Color::White),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} NVMe", nvme_count),
            Style::default().fg(Color::White),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} USB", usb_count),
            Style::default().fg(Color::White),
        ),
    ]));

    // HPA/DCO summary
    let hpa_count = app
        .drives
        .iter()
        .filter(|d| d.hidden_areas.hpa_enabled)
        .count();
    let dco_count = app
        .drives
        .iter()
        .filter(|d| d.hidden_areas.dco_enabled)
        .count();
    let frozen_count = app
        .drives
        .iter()
        .filter(|d| d.ata_security == drivewipe_core::types::AtaSecurityState::Frozen)
        .count();

    lines.push(Line::from(vec![
        Span::styled("  Hidden Areas: ", Style::default().fg(Color::Gray)),
        if hpa_count > 0 {
            Span::styled(
                format!("{} HPA", hpa_count),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("0 HPA", Style::default().fg(Color::Green))
        },
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        if dco_count > 0 {
            Span::styled(
                format!("{} DCO", dco_count),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("0 DCO", Style::default().fg(Color::Green))
        },
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        if frozen_count > 0 {
            Span::styled(
                format!("{} Frozen", frozen_count),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("0 Frozen", Style::default().fg(Color::Green))
        },
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_drive_table(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Detected Drives ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let header = Row::new(vec![
        "Device",
        "Model",
        "Capacity",
        "Type",
        "Transport",
        "HPA",
        "DCO",
        "Security",
    ])
    .style(Style::default().fg(Color::Yellow).bold())
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .drives
        .iter()
        .map(|d| {
            let hpa_cell = if d.hidden_areas.hpa_enabled {
                Span::styled("YES", Style::default().fg(Color::Red).bold())
            } else {
                Span::styled("no", Style::default().fg(Color::DarkGray))
            };
            let dco_cell = if d.hidden_areas.dco_enabled {
                Span::styled("YES", Style::default().fg(Color::Red).bold())
            } else {
                Span::styled("no", Style::default().fg(Color::DarkGray))
            };
            let sec_style = match d.ata_security {
                drivewipe_core::types::AtaSecurityState::Frozen => {
                    Style::default().fg(Color::Red).bold()
                }
                drivewipe_core::types::AtaSecurityState::Locked => {
                    Style::default().fg(Color::Red).bold()
                }
                drivewipe_core::types::AtaSecurityState::NotSupported => {
                    Style::default().fg(Color::DarkGray)
                }
                _ => Style::default().fg(Color::Green),
            };

            Row::new(vec![
                d.path.display().to_string(),
                d.model.clone(),
                d.capacity_display(),
                d.drive_type.to_string(),
                d.transport.to_string(),
                hpa_cell.to_string(),
                dco_cell.to_string(),
                d.ata_security.to_string(),
            ])
            .style(sec_style)
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}

fn draw_network_info(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Network ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![];
    let pxe = app.live_status_lines.iter().any(|l| l.contains("PXE"));
    lines.push(Line::from(vec![
        Span::styled("  Boot method: ", Style::default().fg(Color::Gray)),
        if pxe {
            Span::styled("PXE Network Boot", Style::default().fg(Color::Cyan).bold())
        } else {
            Span::styled("Local (USB/HDD)", Style::default().fg(Color::White))
        },
    ]));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
