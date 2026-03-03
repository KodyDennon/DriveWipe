//! HPA/DCO detection and removal screen.
//!
//! Shows all drives with their current vs native capacity, HPA/DCO status,
//! and provides actions: Detect, Remove HPA, Restore DCO, Freeze DCO.

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
            Constraint::Min(12),   // Drive table
            Constraint::Length(8), // Actions panel
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
            "HPA / DCO Manager",
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(title, chunks[0]);

    // Drive table
    draw_hpa_dco_table(frame, chunks[1], app);

    // Actions panel
    draw_actions(frame, chunks[2], app);

    // Status bar
    ui::status_bar(
        frame,
        chunks[3],
        &[
            ("j/k", "Select Drive"),
            ("d", "Detect"),
            ("r", "Remove HPA"),
            ("R", "Restore DCO"),
            ("q", "Back"),
        ],
    );
}

fn draw_hpa_dco_table(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Drive Hidden Areas ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let header = Row::new(vec![
        "  ",
        "Device",
        "Model",
        "Visible Capacity",
        "HPA",
        "HPA Size",
        "DCO",
        "DCO Size",
    ])
    .style(Style::default().fg(Color::Cyan).bold())
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .drives
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let selected = i == app.live_drive_index;
            let arrow = if selected { "> " } else { "  " };

            let hpa_text = if d.hidden_areas.hpa_enabled {
                "YES"
            } else {
                "no"
            };
            let _hpa_style = if d.hidden_areas.hpa_enabled {
                Style::default().fg(Color::Red).bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let hpa_size = d
                .hidden_areas
                .hpa_size
                .map(drivewipe_core::types::format_bytes)
                .unwrap_or_else(|| "-".to_string());

            let dco_text = if d.hidden_areas.dco_enabled {
                "YES"
            } else {
                "no"
            };
            let _dco_style = if d.hidden_areas.dco_enabled {
                Style::default().fg(Color::Red).bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let dco_size = d
                .hidden_areas
                .dco_size
                .map(drivewipe_core::types::format_bytes)
                .unwrap_or_else(|| "-".to_string());

            let row_style = if selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };

            Row::new(vec![
                arrow.to_string(),
                d.path.display().to_string(),
                d.model.clone(),
                d.capacity_display(),
                hpa_text.to_string(),
                hpa_size,
                dco_text.to_string(),
                dco_size,
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(12),
        Constraint::Min(16),
        Constraint::Length(14),
        Constraint::Length(5),
        Constraint::Length(12),
        Constraint::Length(5),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}

fn draw_actions(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Actions ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled(" [d] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Detect Hidden Areas", Style::default().fg(Color::Cyan)),
            Span::styled(
                "  — Scan selected drive for HPA/DCO",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" [r] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Remove HPA", Style::default().fg(Color::Yellow)),
            Span::styled(
                "  — Set max LBA to native max (DESTRUCTIVE)",
                Style::default().fg(Color::Red),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" [R] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Restore DCO", Style::default().fg(Color::Yellow)),
            Span::styled(
                "  — Restore factory configuration (DESTRUCTIVE)",
                Style::default().fg(Color::Red),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" [F] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Freeze DCO", Style::default().fg(Color::White)),
            Span::styled(
                "  — Lock DCO configuration until power cycle",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
    ];

    // Show confirmation warning if an action is pending
    if let Some(ref action) = app.live_confirm_action {
        let warning = ListItem::new(Line::from(vec![
            Span::styled(" WARNING: ", Style::default().fg(Color::Red).bold()),
            Span::styled(
                format!("Press 'y' to confirm {action}, any other key to cancel"),
                Style::default().fg(Color::Yellow),
            ),
        ]));
        let mut all_items = vec![warning];
        all_items.extend(items);
        frame.render_widget(List::new(all_items), inner);
    } else {
        frame.render_widget(List::new(items), inner);
    }
}
