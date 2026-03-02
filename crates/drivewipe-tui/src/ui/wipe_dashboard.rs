use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Wrap};
use std::io::{self, Write};

use drivewipe_core::types::{WipeOutcome, format_bytes, format_throughput};

use crate::app::App;
use crate::ui::{self, log_viewer};
use crate::widgets::throughput_sparkline;

// ASCII art logo - cross-platform, no special characters
const LOGO: &str = r#"
 ____       _           __        ___
|  _ \ _ __(_)_   _____/ /  ___  / _ \ ___  ___
| | | | '__| \ \ / / _ \ \ / _ \| | | / __|/ _ \
| |_| | |  | |\ V /  __/ /| (_) | |_| \__ \  __/
|____/|_|  |_| \_/ \___/_/  \___/ \___/|___/\___|
"#;

/// Play a terminal bell sound (cross-platform)
fn play_notification_sound() {
    let _ = io::stdout().write_all(b"\x07");
    let _ = io::stdout().flush();
}

/// Draw the active wipe dashboard showing per-drive progress with modern UI.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Layout: Logo -> Progress Cards -> Stats Panel -> Throughput -> Log -> Status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Logo
            Constraint::Min(10),    // Progress cards (dynamic)
            Constraint::Length(4),  // Stats panel
            Constraint::Length(5),  // Throughput sparkline
            Constraint::Min(6),     // Log viewer
            Constraint::Length(1),  // Status bar
        ])
        .split(area);

    // ── Logo ────────────────────────────────────────────────────────────

    let logo_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan).bold())
        .style(Style::default().bg(Color::Black));

    let logo_text = Paragraph::new(LOGO)
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center)
        .block(logo_block);

    frame.render_widget(logo_text, chunks[0]);

    // ── Progress Cards ──────────────────────────────────────────────────

    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    if !sorted_progress.is_empty() {
        // Each card needs 11 lines
        let card_height = 11u16;
        let cards_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                sorted_progress
                    .iter()
                    .map(|_| Constraint::Length(card_height))
                    .collect::<Vec<_>>(),
            )
            .split(chunks[1]);

        for (idx, progress) in sorted_progress.iter().enumerate() {
            if idx >= cards_layout.len() {
                break;
            }
            draw_modern_progress_card(frame, cards_layout[idx], progress);
        }
    }

    // ── Advanced Stats Panel ────────────────────────────────────────────

    draw_stats_panel(frame, chunks[2], &sorted_progress);

    // ── Throughput Sparkline ────────────────────────────────────────────

    let mut combined_history: Vec<f64> = Vec::new();
    let current_throughput: f64 = app
        .wipe_progress
        .values()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.throughput_bps)
        .sum();

    if let Some(active) = app.wipe_progress.values().find(|p| p.outcome.is_none()) {
        combined_history = active.throughput_history.clone();
    }

    throughput_sparkline::draw(frame, chunks[3], &combined_history, current_throughput);

    // ── Log Viewer ──────────────────────────────────────────────────────

    log_viewer::draw(frame, chunks[4], &app.log_messages, app.log_scroll);

    // ── Status Bar ──────────────────────────────────────────────────────

    ui::status_bar(
        frame,
        chunks[5],
        &[("PgUp/PgDn", "Scroll log"), ("q", "Cancel & Quit")],
    );
}

/// Draw a modern progress card with hybrid display: bar + sector info + visual disk
fn draw_modern_progress_card(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    if area.height < 11 {
        return;
    }

    let phase = if progress.verifying {
        "VERIFYING"
    } else if progress.firmware_percent.is_some() {
        "FIRMWARE ERASE"
    } else {
        "WIPING"
    };

    let (border_color, status_text, status_color) = match progress.outcome {
        Some(WipeOutcome::Success) => (Color::Green, "✓ COMPLETE", Color::Green),
        Some(WipeOutcome::SuccessWithWarnings) => (Color::Yellow, "✓ DONE (WARNINGS)", Color::Yellow),
        Some(WipeOutcome::Failed) => (Color::Red, "✗ FAILED", Color::Red),
        Some(WipeOutcome::Cancelled) => (Color::Yellow, "⚠ CANCELLED", Color::Yellow),
        Some(WipeOutcome::Interrupted) => (Color::Yellow, "⚠ INTERRUPTED", Color::Yellow),
        None => (Color::Cyan, phase, Color::Cyan),
    };

    let title = format!(" {} ", progress.device);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).bold())
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let card_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header: Method + Status
            Constraint::Length(1), // Live byte counter
            Constraint::Length(1), // Gauge
            Constraint::Length(1), // Sector info
            Constraint::Length(4), // Visual disk representation
            Constraint::Length(1), // ETA / Stats
        ])
        .split(inner);

    // ── Header: Method + Pass + Status ──────────────────────────────────

    let header_spans = vec![
        Span::styled(
            format!("{} ", progress.method),
            Style::default().fg(Color::Blue).bold(),
        ),
        Span::styled(
            format!("│ Pass {}/{} ", progress.current_pass, progress.total_passes),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("│ "),
        Span::styled(status_text, Style::default().fg(status_color).bold()),
    ];

    frame.render_widget(
        Paragraph::new(Line::from(header_spans)),
        card_layout[0],
    );

    // ── Live Byte Counter (Animated) ────────────────────────────────────

    let bytes_display = if progress.verifying {
        format_bytes(progress.verify_bytes)
    } else {
        format_bytes(progress.bytes_written)
    };

    let total_display = if progress.verifying {
        format_bytes(progress.verify_total)
    } else {
        format_bytes(progress.total_bytes)
    };

    let byte_counter_spans = vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            bytes_display,
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(" / ", Style::default().fg(Color::DarkGray)),
        Span::styled(total_display, Style::default().fg(Color::Gray)),
        Span::styled(
            format!("  ({:.2}%)", progress.fraction() * 100.0),
            Style::default().fg(Color::Blue),
        ),
    ];

    frame.render_widget(
        Paragraph::new(Line::from(byte_counter_spans)),
        card_layout[1],
    );

    // ── Progress Gauge ──────────────────────────────────────────────────

    let fraction = progress.fraction().clamp(0.0, 1.0);
    let gauge_color = match progress.outcome {
        Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings) => Color::Green,
        Some(WipeOutcome::Failed) => Color::Red,
        Some(WipeOutcome::Cancelled | WipeOutcome::Interrupted) => Color::Yellow,
        None => Color::Blue,
    };

    let throughput_display = format_throughput(progress.throughput_bps);
    let gauge_label = format!(" {} ", throughput_display);

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .ratio(fraction)
        .label(Span::styled(
            gauge_label,
            Style::default().fg(Color::White).bold(),
        ));

    frame.render_widget(gauge, card_layout[2]);

    // ── Sector Information ──────────────────────────────────────────────

    let sector_info = format!(
        "  Sector: {}  │  IOPS: {:.0}",
        format_number(progress.current_sector),
        progress.iops,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            sector_info,
            Style::default().fg(Color::DarkGray),
        )),
        card_layout[3],
    );

    // ── Visual Disk Representation ──────────────────────────────────────

    draw_visual_disk(frame, card_layout[4], fraction, gauge_color);

    // ── ETA / Duration ──────────────────────────────────────────────────

    let eta_text = if progress.outcome.is_some() {
        let elapsed = progress.started_at.elapsed().as_secs();
        format!("  Duration: {}", ui::format_eta(elapsed as f64))
    } else {
        match progress.eta_secs() {
            Some(eta) => format!("  ETA: {} remaining", ui::format_eta(eta)),
            None => "  Calculating ETA...".to_string(),
        }
    };

    frame.render_widget(
        Paragraph::new(Span::styled(
            eta_text,
            Style::default().fg(Color::DarkGray),
        )),
        card_layout[5],
    );
}

/// Draw a visual representation of the disk being wiped
fn draw_visual_disk(frame: &mut Frame, area: Rect, fraction: f64, color: Color) {
    if area.height < 4 || area.width < 4 {
        return;
    }

    // Create a visual "disk" using blocks
    let blocks_per_row = (area.width.saturating_sub(4)) as usize;
    let total_blocks = blocks_per_row * 3;  // 3 rows of blocks
    let filled_blocks = (total_blocks as f64 * fraction) as usize;

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "  ┌─ Disk ─┐",
        Style::default().fg(Color::DarkGray),
    )));

    for row in 0..3 {
        let start = row * blocks_per_row;
        let end = ((row + 1) * blocks_per_row).min(total_blocks);

        let mut spans = vec![Span::styled("  │", Style::default().fg(Color::DarkGray))];

        for block_idx in start..end {
            let block_char = if block_idx < filled_blocks {
                "█"
            } else {
                "░"
            };

            let block_color = if block_idx < filled_blocks {
                color
            } else {
                Color::DarkGray
            };

            spans.push(Span::styled(
                block_char,
                Style::default().fg(block_color),
            ));
        }

        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::styled(
        "  └────────┘",
        Style::default().fg(Color::DarkGray),
    )));

    let visual_disk = Paragraph::new(lines);
    frame.render_widget(visual_disk, area);
}

/// Draw advanced stats panel showing aggregate statistics
fn draw_stats_panel(
    frame: &mut Frame,
    area: Rect,
    sorted_progress: &[&crate::app::WipeProgress],
) {
    let block = Block::default()
        .title(" Advanced Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let active_wipes = sorted_progress.iter().filter(|p| p.outcome.is_none()).count();
    let total_throughput: f64 = sorted_progress
        .iter()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.throughput_bps)
        .sum();

    let total_iops: f64 = sorted_progress
        .iter()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.iops)
        .sum();

    let stats_text = vec![
        Line::from(vec![
            Span::styled("  Active Wipes: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", active_wipes),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled("    Total Throughput: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_throughput(total_throughput),
                Style::default().fg(Color::Blue).bold(),
            ),
            Span::styled("    Total IOPS: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.0}", total_iops),
                Style::default().fg(Color::Green).bold(),
            ),
        ]),
    ];

    let stats = Paragraph::new(stats_text);
    frame.render_widget(stats, inner);
}

/// Format a number with thousand separators
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

/// Draw the completed wipe results screen with modern styling
pub fn draw_completed(frame: &mut Frame, app: &mut App) {
    // Play notification sound when done
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
            Constraint::Length(7),  // Logo
            Constraint::Min(8),     // Results
            Constraint::Length(6),  // Summary
            Constraint::Length(1),  // Status
        ])
        .split(area);

    // ── Logo ────────────────────────────────────────────────────────────

    let logo_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green).bold())
        .style(Style::default().bg(Color::Black));

    let logo_text = Paragraph::new(LOGO)
        .style(Style::default().fg(Color::Green).bold())
        .alignment(Alignment::Center)
        .block(logo_block);

    frame.render_widget(logo_text, chunks[0]);

    // ── Results Section ─────────────────────────────────────────────────

    let block = Block::default()
        .title(" Wipe Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan).bold())
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    let mut lines = Vec::new();

    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    for progress in &sorted_progress {
        let (icon, color) = match progress.outcome {
            Some(WipeOutcome::Success) => ("✓", Color::Green),
            Some(WipeOutcome::SuccessWithWarnings) => ("✓", Color::Yellow),
            Some(WipeOutcome::Failed) => ("✗", Color::Red),
            Some(WipeOutcome::Cancelled) => ("⚠", Color::Yellow),
            Some(WipeOutcome::Interrupted) => ("⚠", Color::Yellow),
            None => ("?", Color::Gray),
        };

        let outcome_str = progress
            .outcome
            .map(|o| o.to_string())
            .unwrap_or_else(|| "In Progress".to_string());

        let elapsed = progress.started_at.elapsed().as_secs_f64();
        let avg_throughput = if elapsed > 0.0 && progress.bytes_written > 0 {
            format_throughput(progress.bytes_written as f64 / elapsed)
        } else {
            "N/A".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {icon} "), Style::default().fg(color).bold()),
            Span::styled(
                format!("{:<20}", progress.device),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                format!("{:<24}", progress.method),
                Style::default().fg(Color::Blue),
            ),
            Span::styled(format!("{:<20}", outcome_str), Style::default().fg(color)),
            Span::styled(avg_throughput, Style::default().fg(Color::Gray)),
        ]));

        lines.push(Line::from(vec![
            Span::raw("         "),
            Span::styled(
                format!(
                    "Written: {}  │  Duration: {}  │  {} passes",
                    format_bytes(progress.bytes_written),
                    ui::format_eta(elapsed),
                    progress.total_passes,
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        lines.push(Line::from(""));
    }

    let results = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(results, inner);

    // ── Summary ─────────────────────────────────────────────────────────

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
    let failed = sorted_progress
        .iter()
        .filter(|p| {
            matches!(
                p.outcome,
                Some(WipeOutcome::Failed | WipeOutcome::Cancelled | WipeOutcome::Interrupted)
            )
        })
        .count();

    let summary_color = if failed == 0 {
        Color::Green
    } else if succeeded > 0 {
        Color::Yellow
    } else {
        Color::Red
    };

    let summary_block = Block::default()
        .title(" Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(summary_color).bold())
        .style(Style::default().bg(Color::Black));

    let summary_inner = summary_block.inner(chunks[2]);
    frame.render_widget(summary_block, chunks[2]);

    let summary_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  Total: {}  │  ", total),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                format!("Succeeded: {}  │  ", succeeded),
                Style::default().fg(Color::Green).bold(),
            ),
            Span::styled(
                format!("Failed/Cancelled: {}", failed),
                Style::default()
                    .fg(if failed > 0 { Color::Red } else { Color::Gray })
                    .bold(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if failed == 0 {
                "  All operations completed successfully!"
            } else if succeeded > 0 {
                "  Some operations completed with warnings or failures."
            } else {
                "  All operations failed."
            },
            Style::default().fg(summary_color),
        )),
    ];

    let summary = Paragraph::new(summary_lines);
    frame.render_widget(summary, summary_inner);

    // ── Status Bar ──────────────────────────────────────────────────────

    ui::status_bar(
        frame,
        chunks[3],
        &[("n/Enter", "New Batch"), ("q", "Quit")],
    );
}
