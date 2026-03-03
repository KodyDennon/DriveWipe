//! ATA security state and unfreeze screen.
//!
//! Shows SATA drives with their security state (frozen, locked, enabled, etc).
//! Frozen drives are highlighted with an Unfreeze action. Displays estimated
//! erase times from IDENTIFY DEVICE.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table};

use crate::app::App;
use crate::ui;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(10),   // Drive table
            Constraint::Length(8), // Actions + info
            Constraint::Length(1), // Status bar
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
            "ATA Security Manager",
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(title, chunks[0]);

    // Drive security table
    draw_security_table(frame, chunks[1], app);

    // Actions and info
    draw_security_actions(frame, chunks[2], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[3],
        &[("j/k", "Select"), ("u", "Unfreeze All"), ("q", "Back")],
    );
}

fn draw_security_table(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" ATA Security State ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let header = Row::new(vec![
        "  ",
        "Device",
        "Model",
        "Transport",
        "Security State",
        "Erase Time",
    ])
    .style(Style::default().fg(Color::Cyan).bold())
    .bottom_margin(1);

    // Only show SATA drives (NVMe doesn't use ATA security)
    let sata_drives: Vec<(usize, &drivewipe_core::types::DriveInfo)> = app
        .drives
        .iter()
        .enumerate()
        .filter(|(_, d)| d.transport == drivewipe_core::types::Transport::Sata)
        .collect();

    let rows: Vec<Row> = sata_drives
        .iter()
        .enumerate()
        .map(|(display_idx, (_, d))| {
            let selected = display_idx == app.live_drive_index;
            let arrow = if selected { "> " } else { "  " };

            let (sec_text, _sec_style) = match d.ata_security {
                drivewipe_core::types::AtaSecurityState::Frozen => {
                    ("FROZEN", Style::default().fg(Color::Red).bold())
                }
                drivewipe_core::types::AtaSecurityState::Locked => {
                    ("LOCKED", Style::default().fg(Color::Red).bold())
                }
                drivewipe_core::types::AtaSecurityState::Enabled => {
                    ("Enabled", Style::default().fg(Color::Yellow))
                }
                drivewipe_core::types::AtaSecurityState::Disabled => {
                    ("Disabled", Style::default().fg(Color::Green))
                }
                drivewipe_core::types::AtaSecurityState::CountExpired => {
                    ("EXPIRED", Style::default().fg(Color::Red))
                }
                drivewipe_core::types::AtaSecurityState::NotSupported => {
                    ("N/A", Style::default().fg(Color::DarkGray))
                }
            };

            let erase_time =
                if d.ata_security != drivewipe_core::types::AtaSecurityState::NotSupported {
                    "See IDENTIFY".to_string()
                } else {
                    "-".to_string()
                };

            let row_style = if selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };

            Row::new(vec![
                arrow.to_string(),
                d.path.display().to_string(),
                d.model.clone(),
                d.transport.to_string(),
                sec_text.to_string(),
                erase_time,
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(12),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}

fn draw_security_actions(frame: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .title(" Actions ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled(" [u] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Unfreeze All Drives", Style::default().fg(Color::Cyan)),
            Span::styled(
                "  — Suspend/resume cycle to reset frozen state",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![
            Span::styled(" INFO: ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Frozen drives reject security commands. The BIOS freezes drives during ",
                Style::default().fg(Color::Gray),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("       ", Style::default()),
            Span::styled(
                "boot. A suspend/resume cycle unfreezes them without the BIOS re-locking.",
                Style::default().fg(Color::Gray),
            ),
        ])),
    ];

    frame.render_widget(List::new(items), inner);
}
