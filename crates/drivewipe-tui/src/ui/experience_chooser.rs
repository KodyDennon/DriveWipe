//! First-run choice between the guided and full interfaces.
//!
//! Shown once, then remembered. Someone who booted a USB stick to erase a
//! laptop should not have to scroll a list of standards documents to discover
//! which one they want, and someone sanitising a fleet should not have to fight
//! a wizard.

use drivewipe_core::experience::Experience;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::App;
use crate::ui;

const CHOICES: [Experience; 2] = [Experience::Basic, Experience::Expert];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = ui::centered_column(frame.area(), 78);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // heading
            Constraint::Length(6), // basic
            Constraint::Length(6), // expert
            Constraint::Min(0),    // spacer
            Constraint::Length(1), // hints
        ])
        .split(area);

    let heading = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  DriveWipe",
            Style::default().fg(Color::Cyan).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  How would you like to use it?",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "  You can change this later in Settings.",
            Style::default().fg(Color::DarkGray),
        )),
    ]);
    frame.render_widget(heading, rows[0]);

    for (i, choice) in CHOICES.iter().enumerate() {
        draw_choice(frame, rows[i + 1], *choice, i == app.experience_index);
    }

    ui::status_bar(frame, rows[4], &[("Up/Down", "Choose"), ("Enter", "Start")]);
}

fn draw_choice(frame: &mut Frame, area: Rect, choice: Experience, selected: bool) {
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

    let mut lines = vec![Line::from(vec![
        Span::styled(
            if selected { " > " } else { "   " },
            Style::default().fg(accent).bold(),
        ),
        Span::styled(
            choice.label(),
            if selected {
                Style::default().fg(Color::White).bold()
            } else {
                Style::default().fg(Color::Gray)
            },
        ),
        Span::raw("   "),
        Span::styled(
            match choice {
                Experience::Basic => "recommended if you are not sure",
                Experience::Expert => "for compliance and fleet work",
            },
            Style::default().fg(if selected {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ),
    ])];

    lines.push(Line::from(""));
    let body = Style::default().fg(if selected {
        Color::Gray
    } else {
        Color::DarkGray
    });
    for l in ui::wrap_indented(choice.description(), inner.width as usize, 3) {
        lines.push(Line::from(Span::styled(l, body)));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// The choice under the cursor.
pub fn choice_at(index: usize) -> Experience {
    CHOICES[index.min(CHOICES.len() - 1)]
}

/// How many choices there are, for cursor bounds.
pub fn choice_count() -> usize {
    CHOICES.len()
}
