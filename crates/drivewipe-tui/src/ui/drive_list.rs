use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Row, Table};

use crate::app::App;
use crate::ui;

/// Draw the drive selection table.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Layout: main table area + status bar at bottom.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    let table_area = chunks[0];
    let status_area = chunks[1];

    // Build the header row.
    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from("Device"),
        Cell::from("Model"),
        Cell::from("Serial"),
        Cell::from("Capacity"),
        Cell::from("Type"),
        Cell::from("Transport"),
        Cell::from("Info"),
    ])
    .style(Style::default().fg(Color::Yellow).bold())
    .bottom_margin(1);

    // Build data rows.
    let rows: Vec<Row> = app
        .drives
        .iter()
        .enumerate()
        .map(|(i, drive)| {
            let selected = if i < app.selected_drives.len() && app.selected_drives[i] {
                "[X]"
            } else {
                "[ ]"
            };

            let path = drive.path.display().to_string();
            let model = drive.model.clone();
            let serial = if drive.serial.is_empty() {
                "N/A".to_string()
            } else {
                drive.serial.clone()
            };
            let capacity = drive.capacity_display();
            let dtype = drive.drive_type.to_string();
            let transport = drive.transport.to_string();

            let mut info_parts = Vec::new();
            if drive.is_boot_drive {
                info_parts.push("BOOT");
            }
            if drive.is_removable {
                info_parts.push("USB");
            }
            if drive.is_sed {
                info_parts.push("SED");
            }
            if drive.smart_healthy == Some(false) {
                info_parts.push("UNHEALTHY");
            }
            let info = info_parts.join(", ");

            let style = if drive.is_boot_drive {
                Style::default().fg(Color::DarkGray)
            } else if i < app.selected_drives.len() && app.selected_drives[i] {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(selected),
                Cell::from(path),
                Cell::from(model),
                Cell::from(serial),
                Cell::from(capacity),
                Cell::from(dtype),
                Cell::from(transport),
                Cell::from(info),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Min(12),
        Constraint::Min(16),
        Constraint::Min(14),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Min(8),
    ];

    let selected_count = app.selected_count();
    let title = format!(
        " DriveWipe - Select Drives ({} selected, {} total) ",
        selected_count,
        app.drives.len()
    );

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, table_area, &mut app.table_state);

    // Status bar.
    let hints = if app.drives.is_empty() {
        vec![("r", "Refresh"), ("q", "Quit"), ("?", "Help")]
    } else {
        vec![
            ("Space", "Toggle"),
            ("a", "All"),
            ("Enter", "Continue"),
            ("i", "Info"),
            ("r", "Refresh"),
            ("q", "Quit"),
            ("?", "Help"),
        ]
    };
    ui::status_bar(frame, status_area, &hints);
}
