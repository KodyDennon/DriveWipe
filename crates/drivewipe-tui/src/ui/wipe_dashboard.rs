use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use std::io::{self, Write};

use drivewipe_core::types::{WipeOutcome, format_bytes, format_throughput};

use crate::app::App;
use crate::ui::{self, log_viewer};
use crate::widgets::throughput_sparkline;

// Professional security-focused ASCII art
const LOGO: &str = r#"
╔═══════════════════════════════════════════════════════════════════════════════╗
║  ██████╗ ██████╗ ██╗██╗   ██╗███████╗    ██╗    ██╗██╗██████╗ ███████╗       ║
║  ██╔══██╗██╔══██╗██║██║   ██║██╔════╝    ██║    ██║██║██╔══██╗██╔════╝       ║
║  ██║  ██║██████╔╝██║██║   ██║█████╗      ██║ █╗ ██║██║██████╔╝█████╗         ║
║  ██║  ██║██╔══██╗██║╚██╗ ██╔╝██╔══╝      ██║███╗██║██║██╔═══╝ ██╔══╝         ║
║  ██████╔╝██║  ██║██║ ╚████╔╝ ███████╗    ╚███╔███╔╝██║██║     ███████╗       ║
║  ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝     ╚══╝╚══╝ ╚═╝╚═╝     ╚══════╝       ║
║               SECURE DATA SANITIZATION & FORENSIC ERASURE SYSTEM              ║
╚═══════════════════════════════════════════════════════════════════════════════╝
"#;

/// Play terminal bell
fn play_notification_sound() {
    let _ = io::stdout().write_all(b"\x07");
    let _ = io::stdout().flush();
}

/// Draw professional security-focused wipe dashboard
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: Logo -> Drives Grid -> Bottom panels
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Logo
            Constraint::Min(15),    // Drive panels
            Constraint::Length(8),  // System stats + throughput
            Constraint::Min(4),     // Log
            Constraint::Length(1),  // Status
        ])
        .split(area);

    // Draw logo
    draw_logo(frame, main_chunks[0], app);

    // Drive panels
    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    if !sorted_progress.is_empty() {
        let drive_count = sorted_progress.len();

        // Use grid layout if multiple drives
        if drive_count == 1 {
            draw_drive_panel(frame, main_chunks[1], sorted_progress[0]);
        } else {
            // Split into columns for multiple drives
            let cols = if drive_count <= 2 { 1 } else { 2 };
            let rows = drive_count.div_ceil(cols);

            let row_constraints: Vec<Constraint> = (0..rows)
                .map(|_| Constraint::Ratio(1, rows as u32))
                .collect();

            let row_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(row_constraints)
                .split(main_chunks[1]);

            for (idx, progress) in sorted_progress.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;

                if cols > 1 && row < row_chunks.len() {
                    let col_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(row_chunks[row]);
                    draw_drive_panel(frame, col_chunks[col], progress);
                } else if row < row_chunks.len() {
                    draw_drive_panel(frame, row_chunks[row], progress);
                }
            }
        }
    }

    // Bottom section: Stats + Throughput side by side
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[2]);

    draw_system_stats(frame, bottom_chunks[0], &sorted_progress);

    let history_vec: Vec<f64> = app
        .wipe_progress
        .values()
        .find(|p| p.outcome.is_none())
        .map(|p| p.throughput_history.iter().copied().collect())
        .unwrap_or_default();

    throughput_sparkline::draw(
        frame,
        bottom_chunks[1],
        &history_vec,
        app.wipe_progress
            .values()
            .filter(|p| p.outcome.is_none())
            .map(|p| p.throughput_bps)
            .sum(),
    );

    // Log viewer
    log_viewer::draw(frame, main_chunks[3], &app.log_messages, app.log_scroll);

    // Status bar
    ui::status_bar(
        frame,
        main_chunks[4],
        &[
            ("PgUp/PgDn", "Scroll"),
            ("q", "Cancel & Quit"),
            ("?", "Help"),
        ],
    );
}

/// Draw the logo with status
fn draw_logo(frame: &mut Frame, area: Rect, app: &App) {
    let completed = app
        .wipe_progress
        .values()
        .filter(|p| p.outcome.is_some())
        .count();
    let total = app.wipe_progress.len();
    let all_done = completed == total && total > 0;

    let color = if all_done { Color::Green } else { Color::Cyan };

    let logo_text = Paragraph::new(LOGO)
        .style(Style::default().fg(color).bold())
        .alignment(Alignment::Center);

    frame.render_widget(logo_text, area);
}

/// Draw comprehensive drive panel with all technical details
fn draw_drive_panel(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    let (border_color, status_icon) = match progress.outcome {
        Some(WipeOutcome::Success) => (Color::Green, "✓"),
        Some(WipeOutcome::SuccessWithWarnings) => (Color::Yellow, "⚠"),
        Some(WipeOutcome::Failed) => (Color::Red, "✗"),
        Some(WipeOutcome::Cancelled) => (Color::Yellow, "⊗"),
        Some(WipeOutcome::Interrupted) => (Color::Yellow, "⊘"),
        None => (Color::Cyan, "●"),
    };

    let title = format!(" {} {} ", status_icon, progress.device);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .border_style(Style::default().fg(border_color).bold());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Header info
            Constraint::Length(1), // Progress bar
            Constraint::Length(3), // Sector map
            Constraint::Min(3),    // Technical details
        ])
        .split(inner);

    // Header: Method, Pass, Pattern info
    draw_header_info(frame, layout[0], progress);

    // Progress bar
    draw_progress_bar(frame, layout[1], progress);

    // Sector map visualization
    draw_sector_map(frame, layout[2], progress);

    // Technical details
    draw_technical_details(frame, layout[3], progress);
}

/// Draw header information
fn draw_header_info(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    let method_info = format!("METHOD: {}", progress.method.to_uppercase());
    let pass_info = format!("PASS: {}/{}", progress.current_pass, progress.total_passes);
    let phase = if progress.verifying {
        "VERIFICATION"
    } else {
        "DATA SANITIZATION"
    };

    let bytes_written = if progress.verifying {
        progress.verify_bytes
    } else {
        progress.bytes_written
    };

    let total_bytes = if progress.verifying {
        progress.verify_total
    } else {
        progress.total_bytes
    };

    let completion = (bytes_written as f64 / total_bytes.max(1) as f64 * 100.0).min(100.0);

    let lines = vec![
        Line::from(vec![
            Span::styled("╔══", Style::default().fg(Color::DarkGray)),
            Span::styled(" OPERATION ", Style::default().fg(Color::Cyan).bold()),
            Span::styled("══╗ ", Style::default().fg(Color::DarkGray)),
            Span::styled(phase, Style::default().fg(Color::Yellow).bold()),
        ]),
        Line::from(vec![
            Span::styled("║ ", Style::default().fg(Color::DarkGray)),
            Span::styled(&method_info, Style::default().fg(Color::Blue)),
            Span::raw("  "),
            Span::styled(&pass_info, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::styled("║ COMPLETION: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}%", completion),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "({} / {})",
                    format_bytes(bytes_written),
                    format_bytes(total_bytes)
                ),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("║ THROUGHPUT: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_throughput(progress.throughput_bps),
                Style::default().fg(Color::Green).bold(),
            ),
            Span::raw("  "),
            Span::styled(
                format!("IOPS: {:.0}", progress.iops),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(Span::styled(
            "╚═══════════════",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// Draw progress bar
fn draw_progress_bar(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    let fraction = progress.fraction().clamp(0.0, 1.0);
    let color = match progress.outcome {
        Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings) => Color::Green,
        Some(WipeOutcome::Failed) => Color::Red,
        Some(WipeOutcome::Cancelled | WipeOutcome::Interrupted) => Color::Yellow,
        None => Color::Cyan,
    };

    let label = if let Some(eta) = progress.eta_secs() {
        format!(" ETA: {} ", ui::format_eta(eta))
    } else {
        " Processing... ".to_string()
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .ratio(fraction)
        .label(Span::styled(
            label,
            Style::default().fg(Color::White).bold(),
        ));

    frame.render_widget(gauge, area);
}

/// Draw sector map showing which sectors are being written
fn draw_sector_map(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    if area.width < 10 {
        return;
    }

    let total_sectors = progress.total_bytes / 512;
    let current_sector = progress.current_sector;
    let fraction = current_sector as f64 / total_sectors.max(1) as f64;

    // Create sector map
    let map_width = (area.width.saturating_sub(20)) as usize;
    let filled_chars = (map_width as f64 * fraction) as usize;

    let mut sector_line = String::from("SECTORS [");
    for i in 0..map_width {
        if i < filled_chars {
            sector_line.push('█');
        } else if i == filled_chars && fraction < 1.0 {
            sector_line.push('▓');
        } else {
            sector_line.push('░');
        }
    }
    sector_line.push(']');

    let lines = vec![
        Line::from(vec![
            Span::styled("Current Sector: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:>15}", format_number(current_sector)),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(" / ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_number(total_sectors),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(Span::styled(sector_line, Style::default().fg(Color::Blue))),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// Draw technical details panel
fn draw_technical_details(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    let elapsed = progress.started_at.elapsed().as_secs_f64();
    let ops = progress.write_operations;
    let avg_throughput = if elapsed > 0.0 && progress.bytes_written > 0 {
        progress.bytes_written as f64 / elapsed
    } else {
        0.0
    };

    let security_level = match progress.total_passes {
        1 => ("LOW", Color::Yellow),
        3 => ("MEDIUM", Color::Blue),
        7 => ("HIGH", Color::Magenta),
        35 => ("EXTREME", Color::Red),
        _ => ("CUSTOM", Color::Cyan),
    };

    let items = vec![
        ListItem::new(format!("├─ Security Level: {}", security_level.0))
            .style(Style::default().fg(security_level.1)),
        ListItem::new(format!("├─ Write Operations: {}", format_number(ops)))
            .style(Style::default().fg(Color::Gray)),
        ListItem::new(format!(
            "├─ Average Throughput: {}",
            format_throughput(avg_throughput)
        ))
        .style(Style::default().fg(Color::Gray)),
        ListItem::new(format!("├─ Elapsed Time: {}", ui::format_eta(elapsed)))
            .style(Style::default().fg(Color::Gray)),
        ListItem::new(format!(
            "└─ Status: {}",
            progress
                .outcome
                .map(|o| o.to_string())
                .unwrap_or_else(|| "IN PROGRESS".to_string())
        ))
        .style(Style::default().fg(Color::Cyan)),
    ];

    let list = List::new(items);
    frame.render_widget(list, area);
}

/// Draw system-wide statistics
fn draw_system_stats(frame: &mut Frame, area: Rect, progress_list: &[&crate::app::WipeProgress]) {
    let block = Block::default()
        .title(" ⚡ SYSTEM STATISTICS ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow).bold());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let active = progress_list.iter().filter(|p| p.outcome.is_none()).count();
    let completed = progress_list
        .iter()
        .filter(|p| {
            matches!(
                p.outcome,
                Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings)
            )
        })
        .count();
    let failed = progress_list
        .iter()
        .filter(|p| {
            matches!(
                p.outcome,
                Some(WipeOutcome::Failed | WipeOutcome::Cancelled | WipeOutcome::Interrupted)
            )
        })
        .count();

    let total_throughput: f64 = progress_list
        .iter()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.throughput_bps)
        .sum();

    let total_iops: f64 = progress_list
        .iter()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.iops)
        .sum();

    let total_written: u64 = progress_list.iter().map(|p| p.bytes_written).sum();

    let lines = vec![
        Line::from(vec![
            Span::styled("ACTIVE OPERATIONS: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", active),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw("  "),
            Span::styled("COMPLETED: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", completed),
                Style::default().fg(Color::Green).bold(),
            ),
            Span::raw("  "),
            Span::styled("FAILED: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", failed),
                Style::default().fg(Color::Red).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("AGGREGATE THROUGHPUT: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_throughput(total_throughput),
                Style::default().fg(Color::Green).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("AGGREGATE IOPS: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.0}", total_iops),
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("TOTAL DATA WRITTEN: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_bytes(total_written),
                Style::default().fg(Color::Blue).bold(),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Draw completed screen
pub fn draw_completed(frame: &mut Frame, app: &mut App) {
    static mut NOTIFIED: bool = false;
    unsafe {
        if !NOTIFIED {
            play_notification_sound();
            NOTIFIED = true;
        }
    }

    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Logo
            Constraint::Min(10),    // Results
            Constraint::Length(8),  // Summary
            Constraint::Length(1),  // Status
        ])
        .split(area);

    draw_logo(frame, chunks[0], app);

    // Results
    let block = Block::default()
        .title(" ═══ OPERATION COMPLETE ═══ ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .border_style(Style::default().fg(Color::Green).bold());

    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    let mut lines = Vec::new();
    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    for progress in &sorted_progress {
        let (icon, color) = match progress.outcome {
            Some(WipeOutcome::Success) => ("✓", Color::Green),
            Some(WipeOutcome::SuccessWithWarnings) => ("⚠", Color::Yellow),
            Some(WipeOutcome::Failed) => ("✗", Color::Red),
            Some(WipeOutcome::Cancelled) => ("⊗", Color::Yellow),
            Some(WipeOutcome::Interrupted) => ("⊘", Color::Yellow),
            None => ("?", Color::Gray),
        };

        let elapsed = progress.started_at.elapsed().as_secs_f64();
        let avg_throughput = if elapsed > 0.0 {
            format_throughput(progress.bytes_written as f64 / elapsed)
        } else {
            "N/A".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(color).bold()),
            Span::styled(
                format!("{:<20}", progress.device),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                format!("{:<20}", progress.method),
                Style::default().fg(Color::Blue),
            ),
            Span::styled(
                format!("Passes: {}", progress.total_passes),
                Style::default().fg(Color::Magenta),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                format!("Data: {}  ", format_bytes(progress.bytes_written)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("Avg: {}  ", avg_throughput),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("Time: {}", ui::format_eta(elapsed)),
                Style::default().fg(Color::Gray),
            ),
        ]));
        lines.push(Line::from(""));
    }

    frame.render_widget(Paragraph::new(lines), inner);

    // Summary
    let total = sorted_progress.len();
    let succeeded = sorted_progress
        .iter()
        .filter(|p| {
            matches!(
                p.outcome,
                Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings)
            )
        })
        .count();
    let failed = total - succeeded;

    let summary_block = Block::default()
        .title(" SUMMARY ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if failed == 0 {
            Color::Green
        } else {
            Color::Yellow
        }));

    let summary_inner = summary_block.inner(chunks[2]);
    frame.render_widget(summary_block, chunks[2]);

    let summary_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  Total Drives: {} ", total),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                format!(" Success: {} ", succeeded),
                Style::default().fg(Color::Green).bold(),
            ),
            Span::styled(
                format!(" Failed: {}", failed),
                Style::default()
                    .fg(if failed > 0 { Color::Red } else { Color::Gray })
                    .bold(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if failed == 0 {
                "  ✓ ALL OPERATIONS COMPLETED SUCCESSFULLY - DATA SANITIZED"
            } else {
                "  ⚠ SOME OPERATIONS FAILED - REVIEW REQUIRED"
            },
            Style::default()
                .fg(if failed == 0 {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .bold(),
        )),
        Line::from(""),
    ];

    frame.render_widget(Paragraph::new(summary_lines), summary_inner);

    ui::status_bar(frame, chunks[3], &[("n/Enter", "New"), ("q", "Quit")]);
}
