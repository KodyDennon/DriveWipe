use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::ui;

struct SettingItem {
    label: &'static str,
    description: &'static str,
    is_bool: bool,
}

const SETTINGS: &[SettingItem] = &[
    SettingItem {
        label: "Auto JSON Reports",
        description: "Automatically generate JSON reports after wipe operations",
        is_bool: true,
    },
    SettingItem {
        label: "Desktop Notifications",
        description: "Send desktop notifications when operations complete",
        is_bool: true,
    },
    SettingItem {
        label: "Sleep Prevention",
        description: "Prevent system sleep during active operations",
        is_bool: true,
    },
    SettingItem {
        label: "Auto Health Check",
        description: "Run health check before wipe operations",
        is_bool: true,
    },
    SettingItem {
        label: "Profiles Directory",
        description: "Directory for drive profile TOML files",
        is_bool: false,
    },
    SettingItem {
        label: "Audit Directory",
        description: "Directory for audit log (JSONL) files",
        is_bool: false,
    },
    SettingItem {
        label: "Performance History",
        description: "Directory for performance history data",
        is_bool: false,
    },
    SettingItem {
        label: "Keyboard Lock Sequence",
        description: "Key sequence to unlock keyboard lock mode",
        is_bool: false,
    },
];

/// Draw the settings screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(12),   // Settings list
            Constraint::Length(5), // Detail panel
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![Span::styled(
        " Settings ",
        Style::default().fg(Color::Cyan).bold(),
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    // Settings list
    let settings_block = Block::default()
        .title(" Configuration ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = settings_block.inner(chunks[1]);
    frame.render_widget(settings_block, chunks[1]);

    let items: Vec<ListItem> = SETTINGS
        .iter()
        .enumerate()
        .map(|(i, setting)| {
            let is_selected = i == app.settings_index;
            let arrow = if is_selected { ">" } else { " " };

            let value = if setting.is_bool {
                let enabled = match i {
                    0 => app.config.auto_report_json,
                    1 => app.config.notifications_enabled,
                    2 => app.config.sleep_prevention_enabled,
                    3 => app.config.auto_health_pre_wipe,
                    _ => false,
                };
                if enabled { "[ON] " } else { "[OFF]" }
            } else {
                match i {
                    4 => "...",
                    5 => "...",
                    6 => "...",
                    7 => &app.config.keyboard_lock_sequence,
                    _ => "",
                }
            };

            let value_color = if setting.is_bool {
                let enabled = match i {
                    0 => app.config.auto_report_json,
                    1 => app.config.notifications_enabled,
                    2 => app.config.sleep_prevention_enabled,
                    3 => app.config.auto_health_pre_wipe,
                    _ => false,
                };
                if enabled { Color::Green } else { Color::Red }
            } else {
                Color::Cyan
            };

            let label_style = if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {arrow} "), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:<25}", setting.label), label_style),
                Span::styled(value.to_string(), Style::default().fg(value_color).bold()),
            ]))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);

    // Detail panel
    let detail_block = Block::default()
        .title(" Description ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let detail_inner = detail_block.inner(chunks[2]);
    frame.render_widget(detail_block, chunks[2]);

    if app.settings_index < SETTINGS.len() {
        let setting = &SETTINGS[app.settings_index];
        let detail_lines = vec![
            Line::from(Span::styled(
                format!("  {}", setting.description),
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                if setting.is_bool {
                    "  Press Enter or Space to toggle"
                } else {
                    "  Edit in configuration file"
                },
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(detail_lines), detail_inner);
    }

    // Status bar
    ui::status_bar(
        frame,
        chunks[3],
        &[
            ("Up/Down", "Navigate"),
            ("Enter/Space", "Toggle"),
            ("Esc", "Back"),
        ],
    );
}
