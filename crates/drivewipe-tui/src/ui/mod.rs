pub mod clone_progress;
pub mod clone_setup;
pub mod confirm_dialog;
pub mod drive_list;
pub mod forensic_screen;
pub mod health_screen;
pub mod help;
pub mod info_panel;
pub mod log_viewer;
pub mod main_menu;
pub mod method_select;
pub mod partition_screen;
pub mod settings_screen;
pub mod wipe_dashboard;

#[cfg(all(feature = "live", target_os = "linux"))]
pub mod ata_security_screen;
#[cfg(all(feature = "live", target_os = "linux"))]
pub mod hpa_dco_screen;
#[cfg(all(feature = "live", target_os = "linux"))]
pub mod kernel_status_screen;
#[cfg(all(feature = "live", target_os = "linux"))]
pub mod live_dashboard;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, AppScreen};

/// Top-level draw dispatch: renders the active screen and any overlays.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // Draw keyboard lock overlay if active.
    if app.keyboard_locked {
        let seq_len = app.config.keyboard_lock_sequence.chars().count();
        draw_keyboard_lock_with_config(frame, Some(seq_len));
        return;
    }

    match &app.screen.clone() {
        AppScreen::MainMenu => main_menu::draw(frame, app),
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
        AppScreen::DriveHealth | AppScreen::HealthComparison => health_screen::draw(frame, app),
        AppScreen::CloneSetup => clone_setup::draw(frame, app),
        AppScreen::CloneProgress => clone_progress::draw(frame, app),
        AppScreen::PartitionManager => partition_screen::draw(frame, app),
        AppScreen::ForensicAnalysis => forensic_screen::draw(frame, app),
        AppScreen::Settings => settings_screen::draw(frame, app),
        #[cfg(all(feature = "live", target_os = "linux"))]
        AppScreen::LiveDashboard => live_dashboard::draw(frame, app),
        #[cfg(all(feature = "live", target_os = "linux"))]
        AppScreen::HpaDcoManager => hpa_dco_screen::draw(frame, app),
        #[cfg(all(feature = "live", target_os = "linux"))]
        AppScreen::AtaSecurityManager => ata_security_screen::draw(frame, app),
        #[cfg(all(feature = "live", target_os = "linux"))]
        AppScreen::KernelModuleStatus => kernel_status_screen::draw(frame, app),
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
        Line::from("Active operations are in progress."),
        Line::from("Press 'y' to cancel and quit, any other key to continue."),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Render the keyboard lock overlay, optionally including the unlock sequence length.
pub fn draw_keyboard_lock_with_config(frame: &mut Frame, unlock_seq_len: Option<usize>) {
    let area = frame.area();

    // Fill the background
    let bg = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    let popup = centered_rect(55, 7, area);

    let block = Block::default()
        .title(" KEYBOARD LOCKED ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red).bold())
        .title_style(Style::default().fg(Color::Red).bold());

    let hint = match unlock_seq_len {
        Some(n) => format!("Type the {}-character unlock sequence to resume.", n),
        None => "Type the unlock sequence to resume.".to_string(),
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Keyboard input is locked.",
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::from(Span::styled(hint, Style::default().fg(Color::Gray))),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, popup);
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
