use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

mod app;
mod event;
mod ui;
mod widgets;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Check privileges (warn but don't block)
    if !drivewipe_core::platform::privilege::is_elevated() {
        eprintln!(
            "Warning: {}",
            drivewipe_core::platform::privilege::elevation_hint()
        );
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let config = drivewipe_core::config::DriveWipeConfig::load()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut app = app::App::new(config)?;
    let result = app.run(&mut terminal);

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
