use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::App;
use crate::ui;

/// Draw the clone progress screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(5),  // Source/Target info
            Constraint::Length(3),  // Progress bar
            Constraint::Min(6),    // Stats
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" Clone in Progress ", Style::default().fg(Color::Yellow).bold()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(title, chunks[0]);

    // Source/Target info
    let source_str = match app.clone_source_index {
        Some(i) if i < app.drives.len() => app.drives[i].path.display().to_string(),
        _ => "Unknown".to_string(),
    };
    let target_str = match app.clone_target_index {
        Some(i) if i < app.drives.len() => app.drives[i].path.display().to_string(),
        _ => "Unknown".to_string(),
    };

    let info_block = Block::default()
        .title(" Operation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    let info_inner = info_block.inner(chunks[1]);
    frame.render_widget(info_block, chunks[1]);

    let info_lines = vec![
        Line::from(vec![
            Span::styled("  Source: ", Style::default().fg(Color::Gray)),
            Span::styled(&source_str, Style::default().fg(Color::Blue).bold()),
            Span::styled("  -->  Target: ", Style::default().fg(Color::Gray)),
            Span::styled(&target_str, Style::default().fg(Color::Magenta).bold()),
        ]),
        Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.clone_mode.to_uppercase(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(info_lines), info_inner);

    // Progress bar
    let fraction = app.clone_progress_fraction.clamp(0.0, 1.0);
    let label = format!(" {:.1}% ", fraction * 100.0);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Progress ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .ratio(fraction)
        .label(Span::styled(
            label,
            Style::default().fg(Color::White).bold(),
        ));
    frame.render_widget(gauge, chunks[2]);

    // Stats
    let stats_block = Block::default()
        .title(" Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let stats_inner = stats_block.inner(chunks[3]);
    frame.render_widget(stats_block, chunks[3]);

    let throughput = if app.clone_throughput.is_empty() {
        "Calculating..."
    } else {
        &app.clone_throughput
    };

    let stats_lines = vec![
        Line::from(vec![
            Span::styled("  Throughput: ", Style::default().fg(Color::Gray)),
            Span::styled(throughput, Style::default().fg(Color::Green).bold()),
        ]),
    ];
    frame.render_widget(Paragraph::new(stats_lines), stats_inner);

    // Status bar
    ui::status_bar(frame, chunks[4], &[("q", "Cancel & Quit")]);
}
