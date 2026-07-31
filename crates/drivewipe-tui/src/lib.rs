//! DriveWipe Terminal User Interface (TUI)
//!
//! A dashboard-centric `ratatui` interface for managing multiple wipes, viewing
//! drive health, and performing forensic scans.
//!
//! Live-environment features (HPA/DCO removal, drive unfreezing) are enabled
//! automatically when running on a supported system.

use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;

pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

/// Run the terminal interface until the user exits.
///
/// The terminal is always restored, including on error — leaving a user in raw
/// mode with no cursor after a crash is worse than the crash.
pub async fn run() -> anyhow::Result<()> {
    // Warn but continue without elevation so the drive list stays browsable;
    // operations that genuinely need privileges fail with their own message.
    if !drivewipe_core::platform::privilege::is_elevated() {
        eprintln!(
            "Warning: {}",
            drivewipe_core::platform::privilege::elevation_hint()
        );
    }

    // Load config before entering raw mode so parse errors stay readable.
    let config =
        drivewipe_core::config::DriveWipeConfig::load().map_err(|e| anyhow::anyhow!("{e}"))?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = async {
        let mut app = app::App::new(config).await?;
        app.run(&mut terminal).await
    }
    .await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}
