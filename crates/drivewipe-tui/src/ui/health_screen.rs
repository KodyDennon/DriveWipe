use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Table};

use crate::app::App;
use crate::ui;

/// Draw the drive health screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(10),   // Drive list or health detail
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" Drive Health ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(
            "- Select a drive to view health data",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    // Main area: split into drive list and detail panel
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Drive list
    draw_drive_list(frame, main_chunks[0], app);

    // Health detail
    draw_health_detail(frame, main_chunks[1], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[2],
        &[
            ("Up/Down", "Navigate"),
            ("Enter", "View Health"),
            ("r", "Refresh"),
            ("Esc", "Back"),
        ],
    );
}

fn draw_drive_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new(vec![
        Cell::from("Device"),
        Cell::from("Model"),
        Cell::from("Health"),
    ])
    .style(Style::default().fg(Color::Yellow).bold())
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .drives
        .iter()
        .map(|drive| {
            let path = drive.path.display().to_string();
            let model = if drive.model.len() > 20 {
                format!("{}...", &drive.model[..17])
            } else {
                drive.model.clone()
            };

            let (health_str, health_color) = match drive.smart_healthy {
                Some(true) => ("OK", Color::Green),
                Some(false) => ("FAIL", Color::Red),
                None => ("N/A", Color::DarkGray),
            };

            Row::new(vec![
                Cell::from(path),
                Cell::from(model),
                Cell::from(Span::styled(health_str, Style::default().fg(health_color))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(12),
        Constraint::Min(16),
        Constraint::Length(6),
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

fn draw_health_detail(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Health Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.health_display_lines.is_empty() {
        let placeholder = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Select a drive and press Enter to view health data",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(placeholder, inner);
        return;
    }

    let lines: Vec<Line> = app
        .health_display_lines
        .iter()
        .map(|line| {
            if line.starts_with("SMART Status: HEALTHY") {
                Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Green).bold()))
            } else if line.starts_with("SMART Status: UNHEALTHY") {
                Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Red).bold()))
            } else if line.starts_with("Drive:") || line.starts_with("Serial:") {
                Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Cyan)))
            } else if line.starts_with("Temperature:") {
                Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Yellow)))
            } else {
                Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Gray)))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
