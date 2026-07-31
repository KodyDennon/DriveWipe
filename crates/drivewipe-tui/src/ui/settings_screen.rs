use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use drivewipe_core::config::DriveWipeConfig;

use crate::app::App;
use crate::ui;

/// How a setting is displayed and whether it can be changed from the TUI.
pub enum SettingKind {
    Toggle {
        get: fn(&DriveWipeConfig) -> bool,
        toggle: fn(&mut DriveWipeConfig),
    },
    ReadOnly {
        display: fn(&DriveWipeConfig) -> String,
    },
}

pub struct SettingItem {
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SettingKind,
}

/// Single source of truth for the settings list: rendering and key handling
/// both read from here, so adding a row cannot desynchronise them.
pub const SETTINGS: &[SettingItem] = &[
    SettingItem {
        label: "Auto Verify",
        description: "Verify after wipe. Methods whose standard mandates verification \
                      (DoD, NIST 800-88, HMG, AFSSI/AR/NAVSO) always verify regardless",
        kind: SettingKind::Toggle {
            get: |c| c.auto_verify,
            toggle: |c| c.auto_verify = !c.auto_verify,
        },
    },
    SettingItem {
        label: "Verify Every Pass",
        description: "Read the full surface back after every pass, not just the last. \
                      Produces per-pass evidence at roughly double the wipe time",
        kind: SettingKind::Toggle {
            get: |c| c.verify_each_pass,
            toggle: |c| c.verify_each_pass = !c.verify_each_pass,
        },
    },
    SettingItem {
        label: "Remove Hidden Areas",
        description: "Clear HPA/DCO before wiping so hidden sectors are covered. \
                      Disabling this leaves any hidden data on the drive",
        kind: SettingKind::Toggle {
            get: |c| c.remove_hidden_areas,
            toggle: |c| c.remove_hidden_areas = !c.remove_hidden_areas,
        },
    },
    SettingItem {
        label: "Auto JSON Reports",
        description: "Automatically generate JSON reports after wipe operations",
        kind: SettingKind::Toggle {
            get: |c| c.auto_report_json,
            toggle: |c| c.auto_report_json = !c.auto_report_json,
        },
    },
    SettingItem {
        label: "Desktop Notifications",
        description: "Send desktop notifications when operations complete",
        kind: SettingKind::Toggle {
            get: |c| c.notifications_enabled,
            toggle: |c| c.notifications_enabled = !c.notifications_enabled,
        },
    },
    SettingItem {
        label: "Sleep Prevention",
        description: "Prevent system sleep during active operations",
        kind: SettingKind::Toggle {
            get: |c| c.sleep_prevention_enabled,
            toggle: |c| c.sleep_prevention_enabled = !c.sleep_prevention_enabled,
        },
    },
    SettingItem {
        label: "Auto Health Check",
        description: "Run health check before wipe operations",
        kind: SettingKind::Toggle {
            get: |c| c.auto_health_pre_wipe,
            toggle: |c| c.auto_health_pre_wipe = !c.auto_health_pre_wipe,
        },
    },
    SettingItem {
        label: "Default Method",
        description: "Wipe method preselected for newly added drives",
        kind: SettingKind::ReadOnly {
            display: |c| c.default_method.clone(),
        },
    },
    SettingItem {
        label: "Profiles Directory",
        description: "Directory for drive profile TOML files",
        kind: SettingKind::ReadOnly {
            display: |c| c.profiles_dir.display().to_string(),
        },
    },
    SettingItem {
        label: "Audit Directory",
        description: "Directory for audit log (JSONL) files",
        kind: SettingKind::ReadOnly {
            display: |c| c.audit_dir.display().to_string(),
        },
    },
    SettingItem {
        label: "Performance History",
        description: "Directory for performance history data",
        kind: SettingKind::ReadOnly {
            display: |c| c.performance_history_dir.display().to_string(),
        },
    },
    SettingItem {
        label: "Keyboard Lock Sequence",
        description: "Key sequence to unlock keyboard lock mode",
        kind: SettingKind::ReadOnly {
            display: |c| c.keyboard_lock_sequence.clone(),
        },
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

            let (value, value_color) = match &setting.kind {
                SettingKind::Toggle { get, .. } => {
                    if get(&app.config) {
                        ("[ON] ".to_string(), Color::Green)
                    } else {
                        ("[OFF]".to_string(), Color::Red)
                    }
                }
                SettingKind::ReadOnly { display } => (display(&app.config), Color::Cyan),
            };

            let label_style = if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {arrow} "), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:<25}", setting.label), label_style),
                Span::styled(value, Style::default().fg(value_color).bold()),
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
                match setting.kind {
                    SettingKind::Toggle { .. } => "  Press Enter or Space to toggle",
                    SettingKind::ReadOnly { .. } => "  Edit in configuration file",
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
