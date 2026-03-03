//! Kernel module status display.
//!
//! Shows the DriveWipe kernel module version, capabilities bitmask,
//! and recent dmesg errors related to drivewipe.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::ui;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(12), // Module info
            Constraint::Min(8),     // Capabilities
            Constraint::Length(1),  // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "  DRIVEWIPE LIVE  ",
            Style::default().fg(Color::Black).bg(Color::Yellow).bold(),
        ),
        Span::raw("  "),
        Span::styled(
            "Kernel Module Status",
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(title, chunks[0]);

    // Module info
    draw_module_info(frame, chunks[1], app);

    // Capabilities
    draw_capabilities(frame, chunks[2]);

    // Status bar
    ui::status_bar(frame, chunks[3], &[("q", "Back"), ("r", "Refresh")]);
}

fn draw_module_info(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Module Information ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let module_loaded = app
        .live_status_lines
        .iter()
        .any(|l| l.contains("Kernel module: loaded"));

    let mut lines = vec![];

    if module_loaded {
        lines.push(Line::from(vec![
            Span::styled("  Status: ", Style::default().fg(Color::Gray)),
            Span::styled("LOADED", Style::default().fg(Color::Green).bold()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Device: ", Style::default().fg(Color::Gray)),
            Span::styled("/dev/drivewipe", Style::default().fg(Color::Cyan)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Status: ", Style::default().fg(Color::Gray)),
            Span::styled("NOT LOADED", Style::default().fg(Color::Red).bold()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "The kernel module provides direct ATA/NVMe passthrough, ",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "HPA/DCO manipulation, and DMA I/O. Operations will fall ",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "back to SG_IO when the module is unavailable.",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_capabilities(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Capabilities ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let capabilities = [
        (
            "ATA Passthrough",
            "DW_CAP_ATA",
            "Direct ATA command execution via libata",
        ),
        (
            "NVMe Passthrough",
            "DW_CAP_NVME",
            "Direct NVMe admin command execution",
        ),
        (
            "HPA Support",
            "DW_CAP_HPA",
            "READ NATIVE MAX / SET MAX ADDRESS",
        ),
        (
            "DCO Support",
            "DW_CAP_DCO",
            "DEVICE CONFIGURATION IDENTIFY/RESTORE/FREEZE",
        ),
        ("DMA I/O", "DW_CAP_DMA", "Zero-copy DMA-coherent buffer I/O"),
        (
            "ATA Security",
            "DW_CAP_ATA_SECURITY",
            "Security state query from IDENTIFY DEVICE",
        ),
    ];

    let items: Vec<ListItem> = capabilities
        .iter()
        .map(|(name, flag, desc)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {name:<20}"), Style::default().fg(Color::White)),
                Span::styled(format!("{flag:<22}"), Style::default().fg(Color::DarkGray)),
                Span::styled(desc.to_string(), Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}
