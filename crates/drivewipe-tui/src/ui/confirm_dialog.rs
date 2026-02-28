use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::ui;

/// Draw the confirmation dialog as a centered overlay.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Determine how many lines we need based on the number of drives.
    let drive_count = app.drive_methods.len();
    let height = (12 + drive_count * 2).min(area.height as usize) as u16;
    let width = 70u16.min(area.width);
    let popup_area = ui::centered_rect(width, height, area);

    // Clear the background.
    frame.render_widget(Clear, popup_area);

    let border_color = if app.confirm_countdown.is_some() {
        Color::Red
    } else {
        Color::Yellow
    };

    let block = Block::default()
        .title(" CONFIRM DESTRUCTIVE OPERATION ")
        .title_style(Style::default().fg(Color::Red).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Build the content lines.
    let mut lines = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  WARNING: This operation will PERMANENTLY DESTROY all data!",
        Style::default().fg(Color::Red).bold(),
    )));
    lines.push(Line::from(""));

    // Show each drive and method pair.
    for (drive_idx, method_id) in &app.drive_methods {
        if *drive_idx < app.drives.len() {
            let drive = &app.drives[*drive_idx];
            let method_name = app
                .method_registry
                .get(method_id)
                .map(|m| m.name())
                .unwrap_or("Unknown");
            let pass_count = app
                .method_registry
                .get(method_id)
                .map(|m| m.pass_count())
                .unwrap_or(0);

            lines.push(Line::from(vec![
                Span::styled("  Device: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(
                        "{} ({}, {})",
                        drive.path.display(),
                        drive.model,
                        drive.capacity_display()
                    ),
                    Style::default().fg(Color::White).bold(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Method: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(
                        "{method_name} ({pass_count} pass{})",
                        if pass_count == 1 { "" } else { "es" }
                    ),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            lines.push(Line::from(""));
        }
    }

    // Countdown or input prompt.
    if let Some(started) = app.confirm_countdown {
        let elapsed = started.elapsed().as_secs();
        let remaining = 3u64.saturating_sub(elapsed);

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "  Proceeding in {remaining} second{}...",
                if remaining == 1 { "" } else { "s" }
            ),
            Style::default().fg(Color::Red).bold(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Type YES to confirm: ", Style::default().fg(Color::White)),
            Span::styled(&app.confirm_input, Style::default().fg(Color::Green).bold()),
            Span::styled("_", Style::default().fg(Color::White).slow_blink()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
