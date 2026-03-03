use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Table};

use crate::app::App;
use crate::ui;

/// Draw the partition manager screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(10),   // Main area
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " Partition Manager ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(
            "- Select a drive to view partitions",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    // Main area: drive list + partition details
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Drive list
    draw_drive_list(frame, main_chunks[0], app);

    // Partition detail
    draw_partition_detail(frame, main_chunks[1], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[2],
        &[
            ("Up/Down", "Navigate"),
            ("Enter", "View Partitions"),
            ("r", "Refresh"),
            ("Esc", "Back"),
        ],
    );
}

fn draw_drive_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new(vec![
        Cell::from("Device"),
        Cell::from("Model"),
        Cell::from("Capacity"),
    ])
    .style(Style::default().fg(Color::Yellow).bold())
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .drives
        .iter()
        .map(|drive| {
            Row::new(vec![
                Cell::from(drive.path.display().to_string()),
                Cell::from(drive.model.clone()),
                Cell::from(drive.capacity_display()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(12),
        Constraint::Min(16),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(" Drives ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_partition_detail(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Partitions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.partition_lines.is_empty() {
        let placeholder = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Select a drive and press Enter to view partitions",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(placeholder, inner);
        return;
    }

    let lines: Vec<Line> = app
        .partition_lines
        .iter()
        .map(|line| {
            if line.starts_with("Partition table") {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::Cyan).bold(),
                ))
            } else if line.contains("elevated privileges") || line.contains("CLI") {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::Yellow),
                ))
            } else if line.starts_with("  drivewipe") {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::Green),
                ))
            } else {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::Gray),
                ))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
