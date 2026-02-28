use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::ui;

/// Draw the help overlay showing keybindings grouped by screen.
pub fn draw(frame: &mut Frame, _app: &mut App) {
    let area = frame.area();
    let popup_area = ui::centered_rect_percent(70, 85, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Keyboard Shortcuts ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let section = |title: &str| -> Line {
        Line::from(Span::styled(
            format!("  {title}"),
            Style::default().fg(Color::Yellow).bold(),
        ))
    };

    let binding = |key: &str, desc: &str| -> Line {
        Line::from(vec![
            Span::styled(format!("    {key:<16}"), Style::default().fg(Color::Green)),
            Span::styled(desc.to_string(), Style::default().fg(Color::White)),
        ])
    };

    let blank = || -> Line { Line::from("") };

    let lines = vec![
        blank(),
        section("Global"),
        binding("?", "Toggle help overlay"),
        binding("q", "Quit (with confirmation if wiping)"),
        binding("Ctrl-C", "Quit / Cancel active wipes"),
        blank(),
        section("Drive Selection"),
        binding("Up / k", "Move cursor up"),
        binding("Down / j", "Move cursor down"),
        binding("Space", "Toggle drive selection"),
        binding("a", "Select / deselect all drives"),
        binding("Enter", "Proceed to method selection"),
        binding("i", "Show drive info popup"),
        binding("r", "Refresh drive list"),
        binding("Esc", "Close popup / Quit"),
        blank(),
        section("Method Selection"),
        binding("Up / k", "Move cursor up"),
        binding("Down / j", "Move cursor down"),
        binding("Enter", "Select method and proceed"),
        binding("Esc", "Back to drive selection"),
        blank(),
        section("Confirmation"),
        binding("Type YES", "Confirm destructive operation"),
        binding("Esc", "Cancel and go back"),
        binding("Backspace", "Delete last character"),
        blank(),
        section("Wipe Dashboard"),
        binding("PgUp", "Scroll log up"),
        binding("PgDn", "Scroll log down"),
        binding("q", "Cancel wipes and quit"),
        blank(),
        section("Results"),
        binding("n / Enter", "Start a new batch"),
        binding("q", "Quit"),
        blank(),
        Line::from(Span::styled(
            "  Press q, Esc, or ? to close this help screen",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
