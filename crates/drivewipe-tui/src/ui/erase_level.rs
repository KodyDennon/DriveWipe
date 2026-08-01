//! Basic-mode erase selection.
//!
//! Expert mode lists 27 methods by their standards names. This screen asks the
//! question a non-specialist actually has — how thorough, and how long — and
//! names the standard it picked underneath, so the choice stays auditable.

use drivewipe_core::experience::{EraseLevel, estimate};
use drivewipe_core::types::{DriveInfo, format_bytes};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::App;
use crate::ui;

/// Height of a selected card versus an unselected one. The recommended choice
/// shows its full rationale; the others stay one line so the whole set is
/// comparable at a glance.
const CARD_SELECTED_H: u16 = 7;
const CARD_PLAIN_H: u16 = 4;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = ui::centered_column(frame.area(), 82);

    let Some(drive) = current_drive(app) else {
        ui::status_bar(frame, area, &[("Esc", "Back")]);
        return;
    };

    let title = format!(
        " Erase   {}  ·  {}  ·  {} ",
        truncate(&drive.model, 34),
        format_bytes(drive.capacity),
        drive.path.display(),
    );

    let outer = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Cards, then the standard being used, then the key hints.
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(1)];
    for (i, _) in EraseLevel::ALL.iter().enumerate() {
        constraints.push(Constraint::Length(if i == app.erase_level_index {
            CARD_SELECTED_H
        } else {
            CARD_PLAIN_H
        }));
    }
    constraints.push(Constraint::Min(1));
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Length(1));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, level) in EraseLevel::ALL.iter().enumerate() {
        draw_card(
            frame,
            rows[i + 1],
            *level,
            &drive,
            i == app.erase_level_index,
            app.observed_mbps,
        );
    }

    // Always name the standard. Basic mode simplifies the question, never the
    // record of what was done.
    let chosen = EraseLevel::ALL[app.erase_level_index.min(EraseLevel::ALL.len() - 1)];
    let footer = rows[rows.len() - 2];
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "  Standard applied:  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                chosen.standard_name(&drive),
                Style::default().fg(Color::Gray),
            ),
        ])),
        footer,
    );

    ui::status_bar(
        frame,
        rows[rows.len() - 1],
        &[
            ("Up/Down", "Choose"),
            ("Enter", "Continue"),
            ("e", "Expert mode"),
            ("Esc", "Back"),
        ],
    );
}

fn draw_card(
    frame: &mut Frame,
    area: Rect,
    level: EraseLevel,
    drive: &DriveInfo,
    selected: bool,
    observed_mbps: Option<f64>,
) {
    if area.height == 0 {
        return;
    }

    let accent = if selected {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if selected {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(accent));
    let inner = ui::pad_h(block.inner(area));
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Header: marker, title, recommended badge, time estimate — right-aligned
    // so the durations line up down the column and can be compared.
    let time = estimate(level, drive, observed_mbps);
    let mut header: Vec<Span> = vec![
        Span::styled(
            if selected { " > " } else { "   " },
            Style::default().fg(accent).bold(),
        ),
        Span::styled(
            level.title(),
            if selected {
                Style::default().fg(Color::White).bold()
            } else {
                Style::default().fg(Color::Gray)
            },
        ),
    ];
    if level.is_recommended() {
        header.push(Span::raw("   "));
        header.push(Span::styled(
            "RECOMMENDED",
            Style::default().fg(Color::Green).bold(),
        ));
    }

    let used: usize = header.iter().map(|s| s.content.chars().count()).sum();
    let pad = (inner.width as usize)
        .saturating_sub(used + time.chars().count() + 1)
        .max(1);
    header.push(Span::raw(" ".repeat(pad)));
    header.push(Span::styled(
        time,
        if selected {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));

    let mut lines = vec![Line::from(header)];

    let body_style = if selected {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    for l in ui::wrap_indented(level.summary(), inner.width as usize, 3) {
        lines.push(Line::from(Span::styled(l, body_style)));
    }

    // The "when to use this" line only appears on the focused card; showing it
    // for all three turns the screen into a wall of text.
    if selected && inner.height >= 4 {
        lines.push(Line::from(""));
        for l in ui::wrap_indented(level.guidance(), inner.width as usize, 3) {
            lines.push(Line::from(Span::styled(
                l,
                Style::default().fg(Color::Cyan),
            )));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn current_drive(app: &App) -> Option<DriveInfo> {
    app.selected_drive_indices()
        .first()
        .and_then(|&i| app.drives.get(i))
        .cloned()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('~');
        out
    }
}
