use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::ui;

/// Draw the wipe method selection screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Layout: selected drives summary (top), method list (middle), status bar (bottom).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let summary_area = chunks[0];
    let method_area = chunks[1];
    let status_area = chunks[2];

    // Summary of selected drives.
    let selected_indices = app.selected_drive_indices();
    let summary_text = if selected_indices.len() == 1 {
        let drive = &app.drives[selected_indices[0]];
        format!(
            "  {} - {} ({}) [{}]",
            drive.path.display(),
            drive.model,
            drive.capacity_display(),
            drive.drive_type,
        )
    } else {
        format!(
            "  {} drives selected ({})",
            selected_indices.len(),
            selected_indices
                .iter()
                .map(|&i| app.drives[i].path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let summary = Paragraph::new(summary_text).block(
        Block::default()
            .title(" Selected Drives ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(summary, summary_area);

    // Determine the suggested method for the first selected drive.
    let suggested_method = selected_indices
        .first()
        .map(|&i| app.drives[i].suggested_method())
        .unwrap_or("zero");

    // Determine if selected drives are SSDs (for warning).
    let has_ssd = selected_indices.iter().any(|&i| {
        let dt = app.drives[i].drive_type;
        dt == drivewipe_core::types::DriveType::Ssd
            || dt == drivewipe_core::types::DriveType::Nvme
    });

    // Build the list of methods.
    let methods = app.method_registry.list();
    let items: Vec<ListItem> = methods
        .iter()
        .map(|m| {
            let is_suggested = m.id() == suggested_method;
            let is_firmware = m.is_firmware();

            let mut spans = vec![
                Span::styled(
                    format!("{:<20}", m.id()),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<30}", m.name()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:>2} pass{}", m.pass_count(), if m.pass_count() == 1 { " " } else { "es" }),
                    Style::default().fg(Color::Yellow),
                ),
            ];

            if is_suggested {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[Recommended]",
                    Style::default().fg(Color::Green).bold(),
                ));
            }

            if is_firmware {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[Firmware]",
                    Style::default().fg(Color::Magenta).bold(),
                ));
            }

            // Warn about software wipe on SSD.
            if has_ssd && !is_firmware && m.pass_count() > 0 {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[SSD Warning]",
                    Style::default().fg(Color::Red),
                ));
            }

            let line = Line::from(spans);
            let desc_line = Line::from(vec![
                Span::raw("  "),
                Span::styled(m.description(), Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(vec![line, desc_line])
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select Wipe Method ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, method_area, &mut app.method_list_state);

    // Status bar.
    ui::status_bar(
        frame,
        status_area,
        &[
            ("Enter", "Select"),
            ("Esc", "Back"),
            ("q", "Quit"),
            ("?", "Help"),
        ],
    );
}
