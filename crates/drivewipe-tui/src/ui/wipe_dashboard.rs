use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Wrap};

use drivewipe_core::types::{WipeOutcome, format_bytes, format_throughput};

use crate::app::App;
use crate::ui::{self, log_viewer};
use crate::widgets::throughput_sparkline;

/// Draw the active wipe dashboard showing per-drive progress.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Count active wipes for layout calculation.
    let active_count = app.wipe_progress.len().max(1);
    // Each drive gets 4 rows (device line, gauge, ETA line, blank).
    let wipe_section_height = (active_count * 4 + 2) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(wipe_section_height.min(area.height / 2)),
            Constraint::Length(5),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(area);

    let wipe_area = chunks[0];
    let sparkline_area = chunks[1];
    let log_area = chunks[2];
    let status_area = chunks[3];

    // ── Active wipes section ────────────────────────────────────────────

    let completed = app
        .wipe_progress
        .values()
        .filter(|p| p.outcome.is_some())
        .count();
    let total = app.wipe_progress.len();
    let title = format!(" Active Wipes ({completed}/{total} done) ");

    let wipe_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let wipe_inner = wipe_block.inner(wipe_area);
    frame.render_widget(wipe_block, wipe_area);

    // Render each wipe progress entry.
    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    if !sorted_progress.is_empty() {
        let entry_height = 3u16;
        let entries_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                sorted_progress
                    .iter()
                    .map(|_| Constraint::Length(entry_height))
                    .collect::<Vec<_>>(),
            )
            .split(wipe_inner);

        for (idx, progress) in sorted_progress.iter().enumerate() {
            if idx >= entries_layout.len() {
                break;
            }
            draw_wipe_entry(frame, entries_layout[idx], progress);
        }
    }

    // ── Throughput sparkline ────────────────────────────────────────────

    // Combine throughput history from all active wipes.
    let mut combined_history: Vec<f64> = Vec::new();
    let current_throughput: f64 = app
        .wipe_progress
        .values()
        .filter(|p| p.outcome.is_none())
        .map(|p| p.throughput_bps)
        .sum();

    // Use the first active session's history for the sparkline.
    if let Some(active) = app.wipe_progress.values().find(|p| p.outcome.is_none()) {
        combined_history = active.throughput_history.clone();
    }

    throughput_sparkline::draw(frame, sparkline_area, &combined_history, current_throughput);

    // ── Log viewer ──────────────────────────────────────────────────────

    log_viewer::draw(frame, log_area, &app.log_messages, app.log_scroll);

    // ── Status bar ──────────────────────────────────────────────────────

    ui::status_bar(
        frame,
        status_area,
        &[("PgUp/PgDn", "Scroll log"), ("q", "Cancel & Quit")],
    );
}

/// Draw a single wipe progress entry: header line + gauge + ETA line.
fn draw_wipe_entry(frame: &mut Frame, area: Rect, progress: &crate::app::WipeProgress) {
    if area.height < 3 {
        return;
    }

    let entry_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // gauge
            Constraint::Length(1), // ETA line
        ])
        .split(area);

    // ── Header line ─────────────────────────────────────────────────────

    let phase = if progress.verifying {
        "Verifying"
    } else if progress.firmware_percent.is_some() {
        "Firmware Erase"
    } else {
        "Wiping"
    };

    let (status_color, status_text) = match progress.outcome {
        Some(WipeOutcome::Success) => (Color::Green, "DONE"),
        Some(WipeOutcome::SuccessWithWarnings) => (Color::Yellow, "DONE (warnings)"),
        Some(WipeOutcome::Failed) => (Color::Red, "FAILED"),
        Some(WipeOutcome::Cancelled) => (Color::Yellow, "CANCELLED"),
        Some(WipeOutcome::Interrupted) => (Color::Yellow, "INTERRUPTED"),
        None => (Color::Cyan, phase),
    };

    let header_spans = vec![
        Span::styled(
            format!(" {} ", progress.device),
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format!(" {} ", progress.method),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(
            format!("Pass {}/{}", progress.current_pass, progress.total_passes),
            Style::default().fg(Color::Gray),
        ),
        Span::raw("  "),
        Span::styled(status_text, Style::default().fg(status_color).bold()),
    ];

    let header = Paragraph::new(Line::from(header_spans));
    frame.render_widget(header, entry_chunks[0]);

    // ── Progress gauge ──────────────────────────────────────────────────

    let fraction = progress.fraction().clamp(0.0, 1.0);
    let gauge_color = match progress.outcome {
        Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings) => Color::Green,
        Some(WipeOutcome::Failed) => Color::Red,
        Some(WipeOutcome::Cancelled | WipeOutcome::Interrupted) => Color::Yellow,
        None => Color::Blue,
    };

    let written_display = if progress.verifying {
        format!(
            "{}/{}",
            format_bytes(progress.verify_bytes),
            format_bytes(progress.verify_total)
        )
    } else if let Some(pct) = progress.firmware_percent {
        format!("{pct:.0}%")
    } else {
        format!(
            "{}/{}",
            format_bytes(progress.bytes_written),
            format_bytes(progress.total_bytes)
        )
    };

    let throughput_display = format_throughput(progress.throughput_bps);

    let label = format!(" {written_display}  {throughput_display} ");

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .ratio(fraction)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    frame.render_widget(gauge, entry_chunks[1]);

    // ── ETA line ────────────────────────────────────────────────────────

    let eta_text = if progress.outcome.is_some() {
        let elapsed = progress.started_at.elapsed().as_secs();
        format!("   Elapsed: {}", ui::format_eta(elapsed as f64))
    } else {
        match progress.eta_secs() {
            Some(eta) => format!(
                "   ETA: {}  ({:.1}% complete)",
                ui::format_eta(eta),
                fraction * 100.0
            ),
            None => format!("   {:.1}% complete", fraction * 100.0),
        }
    };

    let eta = Paragraph::new(Span::styled(eta_text, Style::default().fg(Color::DarkGray)));
    frame.render_widget(eta, entry_chunks[2]);
}

/// Draw the completed wipe results screen.
pub fn draw_completed(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(area);

    let results_area = chunks[0];
    let summary_area = chunks[1];
    let status_area = chunks[2];

    // ── Results section ─────────────────────────────────────────────────

    let block = Block::default()
        .title(" Wipe Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(results_area);
    frame.render_widget(block, results_area);

    let mut lines = Vec::new();

    let mut sorted_progress: Vec<_> = app.wipe_progress.values().collect();
    sorted_progress.sort_by(|a, b| a.device.cmp(&b.device));

    for progress in &sorted_progress {
        let (icon, color) = match progress.outcome {
            Some(WipeOutcome::Success) => ("OK", Color::Green),
            Some(WipeOutcome::SuccessWithWarnings) => ("OK", Color::Yellow),
            Some(WipeOutcome::Failed) => ("FAIL", Color::Red),
            Some(WipeOutcome::Cancelled) => ("CANCEL", Color::Yellow),
            Some(WipeOutcome::Interrupted) => ("INTERRUPT", Color::Yellow),
            None => ("???", Color::Gray),
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
            Span::styled(format!("  [{icon}] "), Style::default().fg(color).bold()),
            Span::styled(
                format!("{:<16}", progress.device),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                format!("{:<24}", progress.method),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(format!("{:<20}", outcome_str), Style::default().fg(color)),
            Span::styled(avg_throughput, Style::default().fg(Color::Gray)),
        ]));

        lines.push(Line::from(vec![
            Span::raw("         "),
            Span::styled(
                format!(
                    "Written: {}  Duration: {}",
                    format_bytes(progress.bytes_written),
                    ui::format_eta(elapsed),
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
        .border_style(Style::default().fg(summary_color));

    let summary_inner = summary_block.inner(summary_area);
    frame.render_widget(summary_block, summary_area);

    let summary_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  Total: {total}  "),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("Succeeded: {succeeded}  "),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!("Failed/Cancelled: {failed}"),
                Style::default().fg(if failed > 0 { Color::Red } else { Color::Gray }),
            ),
        ]),
        Line::from(""),
    ];

    let summary = Paragraph::new(summary_lines);
    frame.render_widget(summary, summary_inner);

    // ── Status bar ──────────────────────────────────────────────────────

    ui::status_bar(
        frame,
        status_area,
        &[("n/Enter", "New Batch"), ("q", "Quit")],
    );
}
