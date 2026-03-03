use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Table};

use crate::app::App;
use crate::ui;

/// Draw the clone setup screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(6),  // Config panel
            Constraint::Min(8),    // Drive list
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" Drive Clone Setup ", Style::default().fg(Color::Cyan).bold()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    // Config panel
    draw_config_panel(frame, chunks[1], app);

    // Drive list
    draw_drive_list(frame, chunks[2], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[3],
        &[
            ("s", "Set Source"),
            ("t", "Set Target"),
            ("m", "Toggle Mode"),
            ("Enter", "Start"),
            ("Esc", "Back"),
        ],
    );
}

fn draw_config_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Clone Configuration ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let source_str = match app.clone_source_index {
        Some(i) if i < app.drives.len() => {
            format!("{} ({})", app.drives[i].path.display(), app.drives[i].model)
        }
        _ => "Not selected (press 's')".to_string(),
    };

    let target_str = match app.clone_target_index {
        Some(i) if i < app.drives.len() => {
            format!("{} ({})", app.drives[i].path.display(), app.drives[i].model)
        }
        _ => "Not selected (press 't')".to_string(),
    };

    let source_color = if app.clone_source_index.is_some() {
        Color::Green
    } else {
        Color::DarkGray
    };

    let target_color = if app.clone_target_index.is_some() {
        Color::Green
    } else {
        Color::DarkGray
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Source: ", Style::default().fg(Color::Gray)),
            Span::styled(source_str, Style::default().fg(source_color)),
        ]),
        Line::from(vec![
            Span::styled("  Target: ", Style::default().fg(Color::Gray)),
            Span::styled(target_str, Style::default().fg(target_color)),
        ]),
        Line::from(vec![
            Span::styled("  Mode:   ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.clone_mode.to_uppercase(),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                if app.clone_mode == "block" {
                    " (sector-by-sector copy)"
                } else {
                    " (partition-aware copy)"
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_drive_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new(vec![
        Cell::from("Role"),
        Cell::from("Device"),
        Cell::from("Model"),
        Cell::from("Capacity"),
        Cell::from("Type"),
    ])
    .style(Style::default().fg(Color::Yellow).bold())
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .drives
        .iter()
        .enumerate()
        .map(|(i, drive)| {
            let role = if app.clone_source_index == Some(i) {
                "[SRC]"
            } else if app.clone_target_index == Some(i) {
                "[TGT]"
            } else {
                ""
            };

            let role_color = if app.clone_source_index == Some(i) {
                Color::Blue
            } else if app.clone_target_index == Some(i) {
                Color::Magenta
            } else {
                Color::DarkGray
            };

            Row::new(vec![
                Cell::from(Span::styled(role, Style::default().fg(role_color).bold())),
                Cell::from(drive.path.display().to_string()),
                Cell::from(drive.model.clone()),
                Cell::from(drive.capacity_display()),
                Cell::from(drive.drive_type.to_string()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Min(12),
        Constraint::Min(16),
        Constraint::Length(12),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(" Select Drives ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.table_state);
}
