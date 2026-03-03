use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::ui;

const MENU_ITEMS: &[(&str, &str, &str)] = &[
    ("1", "Secure Wipe", "Sanitize drives with NIST/IEEE compliant methods"),
    ("2", "Drive Health", "SMART data, NVMe health logs, benchmarks"),
    ("3", "Drive Clone", "Block-level or partition-aware cloning"),
    ("4", "Partition Manager", "View and manage drive partitions"),
    ("5", "Forensic Analysis", "Entropy analysis, signature scanning"),
    ("6", "Settings", "Configure application preferences"),
    ("7", "Quit", "Exit DriveWipe"),
];

/// Draw the main menu hub screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Logo
            Constraint::Min(12),   // Menu
            Constraint::Length(6), // Status panel
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Logo
    draw_logo(frame, chunks[0]);

    // Menu
    let menu_block = Block::default()
        .title(" Main Menu ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = menu_block.inner(chunks[1]);
    frame.render_widget(menu_block, chunks[1]);

    let items: Vec<ListItem> = MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, (key, label, desc))| {
            let is_selected = i == app.main_menu_index;
            let arrow = if is_selected { ">" } else { " " };

            let style = if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::White)
            };

            let desc_style = if is_selected {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {arrow} "), Style::default().fg(Color::Yellow)),
                Span::styled(format!("[{key}] "), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{label:<22}"), style),
                Span::styled(desc.to_string(), desc_style),
            ]))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);

    // Status panel
    let status_block = Block::default()
        .title(" System Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let status_inner = status_block.inner(chunks[2]);
    frame.render_widget(status_block, chunks[2]);

    let lock_status = if app.keyboard_locked { "LOCKED" } else { "Unlocked" };
    let lock_color = if app.keyboard_locked { Color::Red } else { Color::Green };

    let status_lines = vec![
        Line::from(vec![
            Span::styled("  Drives detected: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", app.drives.len()),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw("    "),
            Span::styled("Keyboard: ", Style::default().fg(Color::Gray)),
            Span::styled(lock_status, Style::default().fg(lock_color)),
            Span::raw("    "),
            Span::styled("Notifications: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.config.notifications_enabled { "ON" } else { "OFF" },
                Style::default().fg(if app.config.notifications_enabled { Color::Green } else { Color::DarkGray }),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Sleep prevention: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.config.sleep_prevention_enabled { "ON" } else { "OFF" },
                Style::default().fg(if app.config.sleep_prevention_enabled { Color::Green } else { Color::DarkGray }),
            ),
            Span::raw("    "),
            Span::styled("Auto health check: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.config.auto_health_pre_wipe { "ON" } else { "OFF" },
                Style::default().fg(if app.config.auto_health_pre_wipe { Color::Green } else { Color::DarkGray }),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(status_lines), status_inner);

    // Status bar
    ui::status_bar(
        frame,
        chunks[3],
        &[
            ("1-7", "Select"),
            ("Enter", "Open"),
            ("Ctrl-L", "Lock"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
    );
}

fn draw_logo(frame: &mut Frame, area: Rect) {
    let logo = r#"
 ██████╗ ██████╗ ██╗██╗   ██╗███████╗    ██╗    ██╗██╗██████╗ ███████╗
 ██╔══██╗██╔══██╗██║██║   ██║██╔════╝    ██║    ██║██║██╔══██╗██╔════╝
 ██║  ██║██████╔╝██║██║   ██║█████╗      ██║ █╗ ██║██║██████╔╝█████╗
 ██║  ██║██╔══██╗██║╚██╗ ██╔╝██╔══╝      ██║███╗██║██║██╔═══╝ ██╔══╝
 ██████╔╝██║  ██║██║ ╚████╔╝ ███████╗    ╚███╔███╔╝██║██║     ███████╗
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝     ╚══╝╚══╝ ╚═╝╚═╝     ╚══════╝
          SECURE DATA SANITIZATION & DRIVE MANAGEMENT PLATFORM"#;

    let text = Paragraph::new(logo)
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);

    frame.render_widget(text, area);
}
