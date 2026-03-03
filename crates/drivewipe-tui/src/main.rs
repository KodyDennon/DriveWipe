//! DriveWipe Terminal User Interface (TUI)
//!
//! The `drivewipe-tui` binary provides a modern, interactive terminal interface
//! built on `ratatui`. It offers a dashboard-centric workflow for managing
//! multiple wipes, viewing drive health, and performing forensic scans.
//!
//! Special features like HPA/DCO removal and drive unfreezing are automatically
//! enabled when running in a supported Live environment.

use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;

mod app;
mod event;
mod ui;
mod widgets;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Check privileges — warn but allow the TUI to start so users can at
    // least browse the drive list. Wipe operations will fail gracefully if
    // elevated privileges are truly required.
    if !drivewipe_core::platform::privilege::is_elevated() {
        eprintln!(
            "Warning: {}",
            drivewipe_core::platform::privilege::elevation_hint()
        );
    }

    // Load config before entering raw mode so parse errors are readable.
    let config =
        drivewipe_core::config::DriveWipeConfig::load().map_err(|e| anyhow::anyhow!("{e}"))?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run — always restore the terminal even on error.
    let result = (|| -> anyhow::Result<()> {
        let mut app = app::App::new(config)?;
        app.run(&mut terminal)
    })();

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}
