use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::ui;

const MENU_ITEMS: &[(&str, &str, &str)] = &[
    (
        "1",
        "Secure Wipe",
        "Sanitize drives with NIST/IEEE compliant methods",
    ),
    (
        "2",
        "Drive Health",
        "SMART data, NVMe health logs, benchmarks",
    ),
    ("3", "Drive Clone", "Block-level or partition-aware cloning"),
    ("4", "Partition Manager", "View and manage drive partitions"),
    (
        "5",
        "Forensic Analysis",
        "Entropy analysis, signature scanning",
    ),
    ("6", "Settings", "Configure application preferences"),
];

#[cfg(all(feature = "live", target_os = "linux"))]
const LIVE_MENU_ITEMS: &[(&str, &str, &str)] = &[
    (
        "7",
        "Live Dashboard",
        "System overview, drives, HPA/DCO summary",
    ),
    ("8", "HPA/DCO Manager", "Detect and remove hidden areas"),
    (
        "9",
        "ATA Security",
        "Security state, freeze/unfreeze drives",
    ),
    ("0", "Kernel Module", "Module status and capabilities"),
];

/// Draw the main menu hub screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Logo
            Constraint::Min(12),    // Menu
            Constraint::Length(6),  // Status panel
            Constraint::Length(1),  // Status bar
        ])
        .split(area);

    // Logo
    draw_logo(frame, chunks[0], app);

    // Menu
    let is_live = is_live_mode(app);

    let menu_color = if is_live { Color::Yellow } else { Color::Cyan };
    let menu_title = if is_live {
        " DRIVEWIPE LIVE — Main Menu "
    } else {
        " Main Menu "
    };

    let menu_block = Block::default()
        .title(menu_title)
        .title_style(Style::default().fg(menu_color).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(menu_color));

    let inner = menu_block.inner(chunks[1]);
    frame.render_widget(menu_block, chunks[1]);

    let mut all_items: Vec<(&str, &str, &str)> = MENU_ITEMS.to_vec();

    #[cfg(all(feature = "live", target_os = "linux"))]
    if app.live_mode {
        // In live mode, replace "Quit" position with live items, then add Quit at end
        all_items.extend_from_slice(LIVE_MENU_ITEMS);
    }

    // Always add Quit as last item
    let quit_key = if is_live { "Q" } else { "7" };
    all_items.push((quit_key, "Quit", "Exit DriveWipe"));

    let items: Vec<ListItem> = all_items
        .iter()
        .enumerate()
        .map(|(i, (key, label, desc))| {
            let is_selected = i == app.main_menu_index;
            let arrow = if is_selected { ">" } else { " " };

            let style = if is_selected {
                Style::default().fg(menu_color).bold()
            } else {
                Style::default().fg(Color::White)
            };

            let desc_style = if is_selected {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Live-specific items get amber accent
            #[cfg(all(feature = "live", target_os = "linux"))]
            let is_live_item = app.live_mode && i >= MENU_ITEMS.len() && i < all_items.len() - 1;
            #[cfg(not(all(feature = "live", target_os = "linux")))]
            let is_live_item = false;

            let key_style = if is_live_item {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {arrow} "), Style::default().fg(Color::Yellow)),
                Span::styled(format!("[{key}] "), key_style),
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

    let lock_status = if app.keyboard_locked {
        "LOCKED"
    } else {
        "Unlocked"
    };
    let lock_color = if app.keyboard_locked {
        Color::Red
    } else {
        Color::Green
    };

    let mut status_lines = vec![
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
                if app.config.notifications_enabled {
                    "ON"
                } else {
                    "OFF"
                },
                Style::default().fg(if app.config.notifications_enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Sleep prevention: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.config.sleep_prevention_enabled {
                    "ON"
                } else {
                    "OFF"
                },
                Style::default().fg(if app.config.sleep_prevention_enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw("    "),
            Span::styled("Auto health check: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.config.auto_health_pre_wipe {
                    "ON"
                } else {
                    "OFF"
                },
                Style::default().fg(if app.config.auto_health_pre_wipe {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
    ];

    // Add live mode indicator to status
    if is_live {
        status_lines.push(Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(Color::Gray)),
            Span::styled(
                "LIVE ENVIRONMENT",
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(
                "  — Kernel-level operations available",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(status_lines), status_inner);

    // Status bar
    let hint_range = if is_live { "1-0" } else { "1-7" };
    ui::status_bar(
        frame,
        chunks[3],
        &[
            (hint_range, "Select"),
            ("Enter", "Open"),
            ("Ctrl-L", "Lock"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
    );
}

fn draw_logo(frame: &mut Frame, area: Rect, app: &App) {
    let is_live = is_live_mode(app);

    let logo = if is_live {
        r#"
 ██████╗ ██████╗ ██╗██╗   ██╗███████╗    ██╗    ██╗██╗██████╗ ███████╗
 ██╔══██╗██╔══██╗██║██║   ██║██╔════╝    ██║    ██║██║██╔══██╗██╔════╝
 ██║  ██║██████╔╝██║██║   ██║█████╗      ██║ █╗ ██║██║██████╔╝█████╗
 ██║  ██║██╔══██╗██║╚██╗ ██╔╝██╔══╝      ██║███╗██║██║██╔═══╝ ██╔══╝
 ██████╔╝██║  ██║██║ ╚████╔╝ ███████╗    ╚███╔███╔╝██║██║     ███████╗
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝     ╚══╝╚══╝ ╚═╝╚═╝     ╚══════╝
                ██╗     ██╗██╗   ██╗███████╗
                ██║     ██║██║   ██║██╔════╝
                ██║     ██║██║   ██║█████╗
                ██║     ██║╚██╗ ██╔╝██╔══╝
                ███████╗██║ ╚████╔╝ ███████╗
                ╚══════╝╚═╝  ╚═══╝  ╚══════╝"#
    } else {
        r#"
 ██████╗ ██████╗ ██╗██╗   ██╗███████╗    ██╗    ██╗██╗██████╗ ███████╗
 ██╔══██╗██╔══██╗██║██║   ██║██╔════╝    ██║    ██║██║██╔══██╗██╔════╝
 ██║  ██║██████╔╝██║██║   ██║█████╗      ██║ █╗ ██║██║██████╔╝█████╗
 ██║  ██║██╔══██╗██║╚██╗ ██╔╝██╔══╝      ██║███╗██║██║██╔═══╝ ██╔══╝
 ██████╔╝██║  ██║██║ ╚████╔╝ ███████╗    ╚███╔███╔╝██║██║     ███████╗
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝     ╚══╝╚══╝ ╚═╝╚═╝     ╚══════╝
          SECURE DATA SANITIZATION & DRIVE MANAGEMENT PLATFORM"#
    };

    let color = if is_live { Color::Yellow } else { Color::Cyan };
    let text = Paragraph::new(logo)
        .style(Style::default().fg(color).bold())
        .alignment(Alignment::Center);

    frame.render_widget(text, area);
}

/// Check if live mode is active.
fn is_live_mode(_app: &App) -> bool {
    #[cfg(all(feature = "live", target_os = "linux"))]
    {
        _app.live_mode
    }
    #[cfg(not(all(feature = "live", target_os = "linux")))]
    {
        false
    }
}
