pub mod confirm_dialog;
pub mod drive_list;
pub mod help;
pub mod info_panel;
pub mod log_viewer;
pub mod method_select;
pub mod wipe_dashboard;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, AppScreen};

/// Top-level draw dispatch: renders the active screen and any overlays.
pub fn draw(frame: &mut Frame, app: &mut App) {
    match &app.screen.clone() {
        AppScreen::DriveSelection => {
            drive_list::draw(frame, app);
            if app.show_info_popup {
                info_panel::draw(frame, app);
            }
        }
        AppScreen::MethodSelect => method_select::draw(frame, app),
        AppScreen::Confirm => confirm_dialog::draw(frame, app),
        AppScreen::Wiping => wipe_dashboard::draw(frame, app),
        AppScreen::Done => wipe_dashboard::draw_completed(frame, app),
        AppScreen::Error(msg) => draw_error(frame, &msg.clone()),
        AppScreen::Help => help::draw(frame, app),
    }

    // Draw quit confirmation overlay if active.
    if app.quit_confirm {
        draw_quit_confirm(frame);
    }
}

/// Render a full-screen error message.
fn draw_error(frame: &mut Frame, msg: &str) {
    let area = frame.area();
    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title_style(Style::default().fg(Color::Red).bold());

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(msg, Style::default().fg(Color::Red))),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter or q to return",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Render a centered quit confirmation overlay.
fn draw_quit_confirm(frame: &mut Frame) {
    let area = centered_rect(40, 7, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Quit? ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title_style(Style::default().fg(Color::Yellow).bold());

    let text = vec![
        Line::from(""),
        Line::from("Active wipes are in progress."),
        Line::from("Press 'y' to cancel and quit, any other key to continue."),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Create a centered `Rect` within `area` with the given width (columns) and
/// height (rows). Width and height are clamped to the available space.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Create a centered rect using percentage of the area.
pub fn centered_rect_percent(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let w = (area.width as u32 * percent_x as u32 / 100) as u16;
    let h = (area.height as u32 * percent_y as u32 / 100) as u16;
    centered_rect(w, h, area)
}

/// Render a status bar at the bottom of the given area with key hints.
pub fn status_bar(frame: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    let spans: Vec<Span> = hints
        .iter()
        .enumerate()
        .flat_map(|(i, (key, desc))| {
            let mut parts = vec![
                Span::styled(
                    format!(" {key} "),
                    Style::default().bg(Color::DarkGray).fg(Color::White),
                ),
                Span::styled(format!(" {desc} "), Style::default().fg(Color::Gray)),
            ];
            if i < hints.len() - 1 {
                parts.push(Span::raw(" "));
            }
            parts
        })
        .collect();

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}

/// Format a duration in seconds to H:MM:SS or MM:SS.
pub fn format_eta(secs: f64) -> String {
    if secs < 0.0 || !secs.is_finite() {
        return "--:--".to_string();
    }
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}
