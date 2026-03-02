use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{ListState, TableState};
use uuid::Uuid;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::session::{CancellationToken, WipeSession};
use drivewipe_core::types::*;
use drivewipe_core::wipe::WipeMethodRegistry;

use crate::event::{self, AppEvent, EventHandler};
use crate::ui;

// ── Screen state ────────────────────────────────────────────────────────────

/// The currently active screen in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AppScreen {
    DriveSelection,
    MethodSelect,
    Confirm,
    Wiping,
    Done,
    Error(String),
    Help,
}

// ── Per-session wipe progress ───────────────────────────────────────────────

/// Tracked progress for a single wipe session displayed in the dashboard.
#[allow(dead_code)]
pub struct WipeProgress {
    pub session_id: Uuid,
    pub device: String,
    pub method: String,
    pub current_pass: u32,
    pub total_passes: u32,
    pub bytes_written: u64,
    pub total_bytes: u64,
    pub throughput_bps: f64,
    pub outcome: Option<WipeOutcome>,
    /// Last 60 throughput samples for the sparkline widget.
    pub throughput_history: Vec<f64>,
    /// Whether we are in verification phase.
    pub verifying: bool,
    pub verify_bytes: u64,
    pub verify_total: u64,
    pub firmware_percent: Option<f32>,
    pub started_at: Instant,
}

impl WipeProgress {
    pub fn new(
        session_id: Uuid,
        device: String,
        method: String,
        total_bytes: u64,
        total_passes: u32,
    ) -> Self {
        Self {
            session_id,
            device,
            method,
            current_pass: 1,
            total_passes,
            bytes_written: 0,
            total_bytes,
            throughput_bps: 0.0,
            outcome: None,
            throughput_history: Vec::with_capacity(60),
            verifying: false,
            verify_bytes: 0,
            verify_total: total_bytes,
            firmware_percent: None,
            started_at: Instant::now(),
        }
    }

    /// Overall fraction complete (0.0..=1.0).
    pub fn fraction(&self) -> f64 {
        if self.verifying {
            if self.verify_total == 0 {
                return 1.0;
            }
            return self.verify_bytes as f64 / self.verify_total as f64;
        }
        if let Some(pct) = self.firmware_percent {
            return pct as f64 / 100.0;
        }
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.bytes_written as f64 / self.total_bytes as f64
    }

    /// Estimated time remaining in seconds.
    pub fn eta_secs(&self) -> Option<f64> {
        if self.throughput_bps <= 0.0 {
            return None;
        }
        let remaining = if self.verifying {
            self.verify_total.saturating_sub(self.verify_bytes) as f64
        } else {
            self.total_bytes.saturating_sub(self.bytes_written) as f64
        };
        Some(remaining / self.throughput_bps)
    }

    /// Push a throughput sample, keeping only the last 60.
    pub fn push_throughput(&mut self, bps: f64) {
        if self.throughput_history.len() >= 60 {
            self.throughput_history.remove(0);
        }
        self.throughput_history.push(bps);
    }
}

// ── Main app state ──────────────────────────────────────────────────────────

pub struct App {
    pub screen: AppScreen,
    pub config: DriveWipeConfig,
    pub drives: Vec<DriveInfo>,
    /// Boolean flags for which drives are selected (parallel to `drives`).
    pub selected_drives: Vec<bool>,
    /// (drive_index, method_id) pairs for drives that have had a method assigned.
    pub drive_methods: Vec<(usize, String)>,
    pub method_registry: WipeMethodRegistry,
    /// Per-session wipe progress, keyed by session UUID.
    pub wipe_progress: HashMap<Uuid, WipeProgress>,
    /// Completed wipe results for the Done screen.
    pub wipe_results: Vec<WipeResult>,
    /// Timestamped log messages.
    pub log_messages: Vec<(DateTime<Utc>, String)>,
    /// Sender side of the progress channel (given to wipe threads).
    pub progress_tx: Option<Sender<ProgressEvent>>,
    /// Receiver side of the progress channel (consumed by the event handler).
    pub progress_rx: Option<Receiver<ProgressEvent>>,
    pub cancel_token: Arc<CancellationToken>,
    /// Table widget state for drive selection.
    pub table_state: TableState,
    /// List widget state for method selection.
    pub method_list_state: ListState,
    /// Whether the application should exit.
    pub should_quit: bool,
    /// Whether the info popup is showing.
    pub show_info_popup: bool,
    /// Confirmation input buffer.
    pub confirm_input: String,
    /// Countdown timer after typing YES (3 seconds).
    pub confirm_countdown: Option<Instant>,
    /// Which drive index is currently being assigned a method (in MethodSelect).
    pub method_assign_index: usize,
    /// Log scroll offset (0 = bottom / most recent).
    pub log_scroll: usize,
    /// Whether we are showing a quit confirmation overlay during a wipe.
    pub quit_confirm: bool,
}

impl App {
    pub fn new(config: DriveWipeConfig) -> Result<Self> {
        let method_registry = WipeMethodRegistry::new();

        // Create the progress channel pair.
        let (ptx, prx) = event::progress_channel();

        // Set up the global cancellation token.
        let cancel_token = Arc::new(CancellationToken::new());

        // Install Ctrl-C handler that signals cancellation.
        {
            let ct = cancel_token.clone();
            ctrlc::set_handler(move || {
                ct.cancel();
            })
            .ok(); // ignore error if already set
        }

        let mut app = Self {
            screen: AppScreen::DriveSelection,
            config,
            drives: Vec::new(),
            selected_drives: Vec::new(),
            drive_methods: Vec::new(),
            method_registry,
            wipe_progress: HashMap::new(),
            wipe_results: Vec::new(),
            log_messages: Vec::new(),
            progress_tx: Some(ptx),
            progress_rx: Some(prx),
            cancel_token,
            table_state: TableState::default(),
            method_list_state: ListState::default(),
            should_quit: false,
            show_info_popup: false,
            confirm_input: String::new(),
            confirm_countdown: None,
            method_assign_index: 0,
            log_scroll: 0,
            quit_confirm: false,
        };

        // Populate the drive list. Errors are logged into the TUI rather
        // than propagated — this lets users see a helpful message on-screen
        // instead of crashing with a raw OS error.
        app.refresh_drives();
        Ok(app)
    }

    /// Refresh the list of available drives from the system.
    pub fn refresh_drives(&mut self) {
        let enumerator = drive::create_enumerator();
        match enumerator.enumerate() {
            Ok(drives) => {
                let count = drives.len();
                self.drives = drives;
                self.selected_drives = vec![false; count];
                self.table_state = TableState::default();
                if count > 0 {
                    self.table_state.select(Some(0));
                }
                self.log_push(format!("Found {count} drive(s)"));
            }
            Err(e) => {
                self.log_push(format!("Failed to enumerate drives: {e}"));
                self.drives.clear();
                self.selected_drives.clear();
            }
        }
    }

    /// Push a timestamped message to the log buffer.
    pub fn log_push(&mut self, msg: String) {
        self.log_messages.push((Utc::now(), msg));
        // Keep log from growing unbounded.
        if self.log_messages.len() > 5000 {
            self.log_messages.drain(0..1000);
        }
        // Reset scroll to bottom when new messages arrive.
        self.log_scroll = 0;
    }

    /// Get the list of selected drive indices.
    pub fn selected_drive_indices(&self) -> Vec<usize> {
        self.selected_drives
            .iter()
            .enumerate()
            .filter_map(|(i, &sel)| if sel { Some(i) } else { None })
            .collect()
    }

    /// Get the number of selected drives.
    pub fn selected_count(&self) -> usize {
        self.selected_drives.iter().filter(|&&s| s).count()
    }

    /// Currently selected row in the drive table.
    #[allow(dead_code)]
    pub fn selected_table_row(&self) -> Option<usize> {
        self.table_state.selected()
    }

    // ── Main event loop ─────────────────────────────────────────────────

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        // Take the progress receiver out of self for the event handler.
        let prx = self.progress_rx.take();
        let event_handler = EventHandler::new(Duration::from_millis(250), prx);

        while !self.should_quit {
            // Draw.
            terminal.draw(|frame| {
                ui::draw(frame, self);
            })?;

            // Wait for an event.
            match event_handler.next() {
                Ok(AppEvent::Key(key)) => self.handle_key(key),
                Ok(AppEvent::Progress(evt)) => self.handle_progress(evt),
                Ok(AppEvent::Tick) => self.handle_tick(),
                Ok(AppEvent::Resize(_, _)) => {
                    // Terminal will auto-resize on next draw.
                }
                Err(_) => {
                    // Channel closed, exit.
                    self.should_quit = true;
                }
            }

            // Check if cancellation was requested externally (Ctrl-C handler).
            if self.cancel_token.is_cancelled() && self.screen == AppScreen::Wiping {
                self.log_push("Cancellation requested...".into());
            }
        }

        Ok(())
    }

    // ── Key handling dispatch ────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) {
        // Global quit confirmation overlay.
        if self.quit_confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if self.screen == AppScreen::Wiping {
                        self.cancel_token.cancel();
                        self.log_push("Cancelling active wipes...".into());
                    }
                    self.should_quit = true;
                }
                _ => {
                    self.quit_confirm = false;
                }
            }
            return;
        }

        // Ctrl-C always triggers quit or cancel.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.screen == AppScreen::Wiping {
                self.quit_confirm = true;
            } else {
                self.should_quit = true;
            }
            return;
        }

        // '?' key shows help from any screen except Confirm input.
        if key.code == KeyCode::Char('?') && self.screen != AppScreen::Confirm {
            if self.screen == AppScreen::Help {
                self.screen = AppScreen::DriveSelection; // toggle off
            } else {
                self.screen = AppScreen::Help;
            }
            return;
        }

        match &self.screen.clone() {
            AppScreen::DriveSelection => self.handle_drive_selection_key(key),
            AppScreen::MethodSelect => self.handle_method_select_key(key),
            AppScreen::Confirm => self.handle_confirm_key(key),
            AppScreen::Wiping => self.handle_wiping_key(key),
            AppScreen::Done => self.handle_done_key(key),
            AppScreen::Error(_) => self.handle_error_key(key),
            AppScreen::Help => self.handle_help_key(key),
        }
    }

    // ── Drive selection keys ────────────────────────────────────────────

    fn handle_drive_selection_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = match self.table_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.table_state.select(Some(i));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.drives.len().saturating_sub(1);
                let i = match self.table_state.selected() {
                    Some(i) if i < max => i + 1,
                    Some(i) => i,
                    None => 0,
                };
                self.table_state.select(Some(i));
            }
            KeyCode::Char(' ') => {
                // Toggle selection of current drive.
                if let Some(i) = self.table_state.selected() {
                    if i < self.selected_drives.len() {
                        // Prevent selecting boot drives.
                        if self.drives[i].is_boot_drive {
                            let msg = format!(
                                "Cannot select boot drive: {}",
                                self.drives[i].path.display()
                            );
                            self.log_push(msg);
                        } else {
                            self.selected_drives[i] = !self.selected_drives[i];
                        }
                    }
                }
            }
            KeyCode::Char('a') => {
                // Select/deselect all non-boot drives.
                let any_selected = self.selected_drives.iter().any(|&s| s);
                for (i, sel) in self.selected_drives.iter_mut().enumerate() {
                    if !self.drives[i].is_boot_drive {
                        *sel = !any_selected;
                    }
                }
            }
            KeyCode::Enter => {
                if self.selected_count() > 0 {
                    // Move to method selection for the first selected drive.
                    self.drive_methods.clear();
                    let indices = self.selected_drive_indices();
                    self.method_assign_index = 0;
                    self.method_list_state = ListState::default();

                    // Pre-select the suggested method.
                    if let Some(&drive_idx) = indices.first() {
                        let suggested = self.drives[drive_idx].suggested_method();
                        let methods = self.method_registry.list();
                        for (mi, m) in methods.iter().enumerate() {
                            if m.id() == suggested {
                                self.method_list_state.select(Some(mi));
                                break;
                            }
                        }
                    }

                    if self.method_list_state.selected().is_none() {
                        self.method_list_state.select(Some(0));
                    }

                    self.screen = AppScreen::MethodSelect;
                } else {
                    self.log_push("No drives selected. Press Space to select.".into());
                }
            }
            KeyCode::Char('i') => {
                // Toggle info popup for selected drive.
                if self.table_state.selected().is_some() {
                    self.show_info_popup = !self.show_info_popup;
                }
            }
            KeyCode::Char('r') => {
                self.refresh_drives();
            }
            KeyCode::Esc => {
                if self.show_info_popup {
                    self.show_info_popup = false;
                } else {
                    self.should_quit = true;
                }
            }
            _ => {}
        }
    }

    // ── Method selection keys ───────────────────────────────────────────

    fn handle_method_select_key(&mut self, key: KeyEvent) {
        let method_count = self.method_registry.list().len();

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.screen = AppScreen::DriveSelection;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = match self.method_list_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                self.method_list_state.select(Some(i));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = method_count.saturating_sub(1);
                let i = match self.method_list_state.selected() {
                    Some(i) if i < max => i + 1,
                    Some(i) => i,
                    None => 0,
                };
                self.method_list_state.select(Some(i));
            }
            KeyCode::Enter => {
                if let Some(mi) = self.method_list_state.selected() {
                    let methods = self.method_registry.list();
                    if mi < methods.len() {
                        let method_id = methods[mi].id().to_string();
                        let is_firmware = methods[mi].is_firmware();
                        let indices = self.selected_drive_indices();

                        // Assign method to all selected drives.
                        self.drive_methods.clear();
                        for &di in &indices {
                            self.drive_methods.push((di, method_id.clone()));
                        }

                        // Collect SSD warning messages first to avoid borrow conflict.
                        let warnings: Vec<String> = indices
                            .iter()
                            .filter_map(|&di| {
                                let drive = &self.drives[di];
                                if (drive.drive_type == DriveType::Ssd
                                    || drive.drive_type == DriveType::Nvme)
                                    && !is_firmware
                                {
                                    Some(format!(
                                        "Warning: Software overwrite on SSD {} may not sanitize all data due to wear leveling",
                                        drive.path.display()
                                    ))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        for msg in warnings {
                            self.log_push(msg);
                        }

                        self.confirm_input.clear();
                        self.confirm_countdown = None;
                        self.screen = AppScreen::Confirm;
                    }
                }
            }
            _ => {}
        }
    }

    // ── Confirmation keys ───────────────────────────────────────────────

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        // If countdown is active, only Escape can cancel.
        if self.confirm_countdown.is_some() {
            if key.code == KeyCode::Esc {
                self.confirm_countdown = None;
                self.confirm_input.clear();
                self.screen = AppScreen::MethodSelect;
            }
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.confirm_input.clear();
                self.screen = AppScreen::MethodSelect;
            }
            KeyCode::Backspace => {
                self.confirm_input.pop();
            }
            KeyCode::Char(c) => {
                self.confirm_input.push(c);
                // Check if the user typed "YES".
                if self.confirm_input.trim() == "YES" {
                    self.confirm_countdown = Some(Instant::now());
                }
            }
            KeyCode::Enter => {
                if self.confirm_input.trim() == "YES" {
                    self.confirm_countdown = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    // ── Wiping keys ─────────────────────────────────────────────────────

    fn handle_wiping_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.quit_confirm = true;
            }
            KeyCode::PageUp => {
                let max_scroll = self.log_messages.len().saturating_sub(1);
                self.log_scroll = (self.log_scroll + 10).min(max_scroll);
            }
            KeyCode::PageDown => {
                self.log_scroll = self.log_scroll.saturating_sub(10);
            }
            _ => {}
        }
    }

    // ── Done keys ───────────────────────────────────────────────────────

    fn handle_done_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('n') | KeyCode::Enter => {
                // Start a new batch.
                self.wipe_progress.clear();
                self.wipe_results.clear();
                self.drive_methods.clear();
                self.confirm_input.clear();
                self.confirm_countdown = None;

                // Reset the existing cancellation token instead of creating a
                // new one.  The Ctrl-C handler installed in App::new() already
                // holds an Arc to this token's inner AtomicBool, so
                // reinstalling the handler (which can only be set once) is
                // both unnecessary and would silently fail.
                self.cancel_token.reset();

                // Re-create progress channel.
                let (ptx, prx) = event::progress_channel();
                self.progress_tx = Some(ptx);
                self.progress_rx = Some(prx);

                self.refresh_drives();
                self.screen = AppScreen::DriveSelection;
            }
            _ => {}
        }
    }

    // ── Error keys ──────────────────────────────────────────────────────

    fn handle_error_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                self.screen = AppScreen::DriveSelection;
            }
            _ => {}
        }
    }

    // ── Help keys ───────────────────────────────────────────────────────

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
                self.screen = AppScreen::DriveSelection;
            }
            _ => {}
        }
    }

    // ── Progress event handling ─────────────────────────────────────────

    fn handle_progress(&mut self, event: ProgressEvent) {
        let sid = event.session_id();

        match event {
            ProgressEvent::SessionStarted {
                session_id,
                device_path,
                method_name,
                total_bytes,
                total_passes,
                ..
            } => {
                let msg = format!("Wipe started on {device_path} ({method_name})");
                self.log_push(msg);
                let progress = WipeProgress::new(
                    session_id,
                    device_path,
                    method_name,
                    total_bytes,
                    total_passes,
                );
                self.wipe_progress.insert(session_id, progress);
            }
            ProgressEvent::PassStarted {
                pass_number,
                pass_name,
                ..
            } => {
                // Extract device name before mutable borrow.
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.current_pass = pass_number;
                }
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Pass {pass_number} ({pass_name}) started on {device}",
                    ));
                }
            }
            ProgressEvent::BlockWritten {
                bytes_written,
                throughput_bps,
                pass_number,
                ..
            } => {
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.bytes_written = bytes_written;
                    p.throughput_bps = throughput_bps;
                    p.current_pass = pass_number;
                    p.push_throughput(throughput_bps);
                }
            }
            ProgressEvent::PassCompleted {
                pass_number,
                duration_secs,
                throughput_mbps,
                ..
            } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Pass {pass_number} completed on {device} ({duration_secs:.1}s, {throughput_mbps:.1} MiB/s)",
                    ));
                }
            }
            ProgressEvent::VerificationStarted { .. } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.verifying = true;
                    p.verify_bytes = 0;
                }
                if let Some(device) = device_name {
                    self.log_push(format!("Verification started on {device}"));
                }
            }
            ProgressEvent::VerificationProgress {
                bytes_verified,
                total_bytes,
                ..
            } => {
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.verify_bytes = bytes_verified;
                    p.verify_total = total_bytes;
                }
            }
            ProgressEvent::VerificationCompleted {
                passed,
                duration_secs,
                ..
            } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.verifying = false;
                }
                let status = if passed { "PASSED" } else { "FAILED" };
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Verification {status} on {device} ({duration_secs:.1}s)",
                    ));
                }
            }
            ProgressEvent::FirmwareEraseStarted { method_name, .. } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.firmware_percent = Some(0.0);
                }
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Firmware erase ({method_name}) started on {device}",
                    ));
                }
            }
            ProgressEvent::FirmwareEraseProgress { percent, .. } => {
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.firmware_percent = Some(percent);
                }
            }
            ProgressEvent::FirmwareEraseCompleted { duration_secs, .. } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.firmware_percent = None;
                }
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Firmware erase completed on {device} ({duration_secs:.1}s)",
                    ));
                }
            }
            ProgressEvent::Warning { message, .. } => {
                self.log_push(format!("WARNING: {message}"));
            }
            ProgressEvent::Error { message, .. } => {
                self.log_push(format!("ERROR: {message}"));
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.outcome = Some(WipeOutcome::Failed);
                }
            }
            ProgressEvent::Interrupted {
                reason,
                bytes_written,
                ..
            } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.outcome = Some(WipeOutcome::Interrupted);
                }
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Wipe interrupted on {device}: {reason} ({} written)",
                        format_bytes(bytes_written)
                    ));
                }
            }
            ProgressEvent::Completed {
                outcome,
                duration_secs,
                ..
            } => {
                let device_name = self.wipe_progress.get(&sid).map(|p| p.device.clone());
                if let Some(p) = self.wipe_progress.get_mut(&sid) {
                    p.outcome = Some(outcome);
                }
                if let Some(device) = device_name {
                    self.log_push(format!(
                        "Wipe completed on {device}: {outcome} ({duration_secs:.1}s)",
                    ));
                }
                // Check if all sessions are done.
                self.check_all_done();
            }
        }
    }

    /// Check if all wipe sessions have completed and transition to Done.
    fn check_all_done(&mut self) {
        if self.wipe_progress.is_empty() {
            return;
        }
        let all_done = self.wipe_progress.values().all(|p| p.outcome.is_some());
        if all_done {
            self.screen = AppScreen::Done;
        }
    }

    // ── Tick handling ───────────────────────────────────────────────────

    fn handle_tick(&mut self) {
        // Check the confirmation countdown.
        if let Some(started) = self.confirm_countdown {
            if started.elapsed() >= Duration::from_secs(3) {
                self.confirm_countdown = None;
                self.start_wipes();
            }
        }
    }

    // ── Start wipe operations ───────────────────────────────────────────

    fn start_wipes(&mut self) {
        self.screen = AppScreen::Wiping;
        self.wipe_progress.clear();
        self.wipe_results.clear();

        let pairs = self.drive_methods.clone();

        for (drive_idx, method_id) in pairs {
            if drive_idx >= self.drives.len() {
                continue;
            }

            let drive_info = self.drives[drive_idx].clone();

            // Validate the device still exists before spawning a wipe thread.
            // On Windows, device paths like \\.\PhysicalDrive0 don't support .exists()
            // so we skip this check and let the device open operation fail if needed.
            #[cfg(not(target_os = "windows"))]
            if !drive_info.path.exists() {
                self.log_push(format!(
                    "Skipping {}: device no longer available (disconnected?)",
                    drive_info.path.display()
                ));
                continue;
            }
            let config = self.config.clone();
            let auto_report_json = config.auto_report_json;
            let sessions_dir = config.sessions_dir().clone();
            let cancel_token = self.cancel_token.clone();

            // Verify the method exists before spawning.
            if self.method_registry.get(&method_id).is_none() {
                self.log_push(format!("Unknown method: {method_id}"));
                continue;
            }

            let method = method_id.clone();

            let progress_tx = match &self.progress_tx {
                Some(tx) => tx.clone(),
                None => {
                    self.log_push("No progress channel available".into());
                    continue;
                }
            };

            // Spawn a wipe thread.
            let device_display = drive_info.path.display().to_string();
            self.log_push(format!(
                "Starting wipe on {device_display} with method {method}"
            ));

            std::thread::spawn(move || {
                // Build a fresh registry that contains ALL methods (software +
                // firmware). We then find our method by id and consume the
                // registry to extract the owned Box<dyn WipeMethod>.
                let registry = WipeMethodRegistry::new();
                if registry.get(&method).is_none() {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Method not found for session: {method}"),
                    });
                    return;
                }

                // Re-create owned method instances. Try software first, then firmware.
                let all_software = drivewipe_core::wipe::software::all_software_methods();
                let boxed_method: Box<dyn drivewipe_core::wipe::WipeMethod> = match all_software
                    .into_iter()
                    .find(|m| m.id() == method)
                {
                    Some(m) => m,
                    None => {
                        // Firmware method — create fresh instances and wrap in adapter
                        let fw_instances: Vec<
                            Box<dyn drivewipe_core::wipe::firmware::FirmwareWipe>,
                        > = vec![
                            Box::new(drivewipe_core::wipe::firmware::ata::AtaSecureErase),
                            Box::new(drivewipe_core::wipe::firmware::ata::AtaEnhancedSecureErase),
                            Box::new(drivewipe_core::wipe::firmware::nvme::NvmeFormatUserData),
                            Box::new(drivewipe_core::wipe::firmware::nvme::NvmeFormatCrypto),
                            Box::new(drivewipe_core::wipe::firmware::nvme::NvmeSanitizeBlock),
                            Box::new(drivewipe_core::wipe::firmware::nvme::NvmeSanitizeCrypto),
                            Box::new(drivewipe_core::wipe::firmware::nvme::NvmeSanitizeOverwrite),
                            Box::new(drivewipe_core::wipe::crypto_erase::TcgOpalCryptoErase),
                        ];
                        match fw_instances.into_iter().find(|fw| fw.id() == method) {
                            Some(fw) => {
                                Box::new(drivewipe_core::wipe::FirmwareMethodAdapter::new(fw))
                            }
                            None => {
                                let _ = progress_tx.send(ProgressEvent::Error {
                                    session_id: Uuid::new_v4(),
                                    message: format!("Method not found for session: {method}"),
                                });
                                return;
                            }
                        }
                    }
                };

                let session = WipeSession::new(drive_info.clone(), boxed_method, config);

                // Open the device for raw I/O.
                #[cfg(target_os = "linux")]
                let device_result = {
                    use drivewipe_core::io::linux::LinuxDeviceIo;
                    LinuxDeviceIo::open(&drive_info.path)
                };
                #[cfg(target_os = "macos")]
                let device_result = {
                    use drivewipe_core::io::macos::MacosDeviceIo;
                    MacosDeviceIo::open(&drive_info.path)
                };
                #[cfg(target_os = "windows")]
                let device_result = {
                    use drivewipe_core::io::windows::WindowsDeviceIo;
                    WindowsDeviceIo::open(&drive_info.path)
                };

                let mut device = match device_result {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = progress_tx.send(ProgressEvent::Error {
                            session_id: session.session_id,
                            message: format!("Failed to open {}: {e}", drive_info.path.display()),
                        });
                        let _ = progress_tx.send(ProgressEvent::Completed {
                            session_id: session.session_id,
                            outcome: WipeOutcome::Failed,
                            duration_secs: 0.0,
                        });
                        return;
                    }
                };

                // Use the global cancellation token directly — no need for a
                // watcher thread that polls and leaks.

                match session.execute(&mut device, &progress_tx, &cancel_token, None) {
                    Ok(result) => {
                        // Completion event already sent by the session.
                        // Auto-generate JSON report in the thread while we
                        // have access to the WipeResult.
                        if auto_report_json {
                            let report_dir = sessions_dir;
                            if let Err(e) = std::fs::create_dir_all(&report_dir) {
                                log::warn!("Failed to create report directory: {e}");
                            } else {
                                let json_path =
                                    report_dir.join(format!("{}.json", result.session_id));
                                let generator = drivewipe_core::report::json::JsonReportGenerator;
                                match drivewipe_core::report::ReportGenerator::generate(
                                    &generator, &result,
                                ) {
                                    Ok(bytes) => {
                                        if let Err(e) = std::fs::write(&json_path, &bytes) {
                                            log::warn!("Failed to write JSON report: {e}");
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to generate JSON report: {e}");
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = progress_tx.send(ProgressEvent::Error {
                            session_id: session.session_id,
                            message: format!("Wipe failed: {e}"),
                        });
                        let _ = progress_tx.send(ProgressEvent::Completed {
                            session_id: session.session_id,
                            outcome: WipeOutcome::Failed,
                            duration_secs: 0.0,
                        });
                    }
                }
            });
        }
    }
}
