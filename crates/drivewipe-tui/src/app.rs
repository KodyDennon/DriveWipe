use std::collections::{HashMap, VecDeque};
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
    MainMenu,
    DriveSelection,
    MethodSelect,
    Confirm,
    Wiping,
    Done,
    Error(String),
    Help,
    DriveHealth,
    HealthComparison,
    CloneSetup,
    CloneProgress,
    PartitionManager,
    ForensicAnalysis,
    Settings,
    #[cfg(all(feature = "live", target_os = "linux"))]
    LiveDashboard,
    #[cfg(all(feature = "live", target_os = "linux"))]
    HpaDcoManager,
    #[cfg(all(feature = "live", target_os = "linux"))]
    AtaSecurityManager,
    #[cfg(all(feature = "live", target_os = "linux"))]
    KernelModuleStatus,
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
    pub throughput_history: VecDeque<f64>,
    /// Whether we are in verification phase.
    pub verifying: bool,
    pub verify_bytes: u64,
    pub verify_total: u64,
    pub firmware_percent: Option<f32>,
    pub started_at: Instant,
    /// Advanced stats for modern UI
    pub current_sector: u64,
    pub last_update: Instant,
    pub write_operations: u64,
    pub iops: f64,
}

impl WipeProgress {
    pub fn new(
        session_id: Uuid,
        device: String,
        method: String,
        total_bytes: u64,
        total_passes: u32,
    ) -> Self {
        let now = Instant::now();
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
            throughput_history: VecDeque::with_capacity(60),
            verifying: false,
            verify_bytes: 0,
            verify_total: total_bytes,
            firmware_percent: None,
            started_at: now,
            current_sector: 0,
            last_update: now,
            write_operations: 0,
            iops: 0.0,
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
            self.throughput_history.pop_front();
        }
        self.throughput_history.push_back(bps);
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
    /// Index of the selected drive for health/forensic/partition screens.
    pub focused_drive_index: Option<usize>,
    /// Main menu cursor position.
    pub main_menu_index: usize,
    /// Clone source drive index.
    pub clone_source_index: Option<usize>,
    /// Clone target drive index.
    pub clone_target_index: Option<usize>,
    /// Clone mode: "block" or "partition".
    pub clone_mode: String,
    /// Whether keyboard lock is active.
    pub keyboard_locked: bool,
    /// Keyboard lock unlock sequence buffer.
    pub keyboard_lock_buffer: Vec<char>,
    /// Settings screen cursor position.
    pub settings_index: usize,
    /// Forensic scan progress percent (0..100).
    pub forensic_progress_pct: f32,
    /// Forensic scan result summary lines.
    pub forensic_result_lines: Vec<String>,
    /// Health data lines for display.
    pub health_display_lines: Vec<String>,
    /// Clone progress fraction (0.0..1.0).
    pub clone_progress_fraction: f64,
    /// Clone throughput string.
    pub clone_throughput: String,
    /// Partition display lines.
    pub partition_lines: Vec<String>,

    // ── Live mode state ─────────────────────────────────────────────────
    /// Whether live mode is active.
    #[cfg(all(feature = "live", target_os = "linux"))]
    pub live_mode: bool,
    /// Live mode status lines for the dashboard.
    #[cfg(all(feature = "live", target_os = "linux"))]
    pub live_status_lines: Vec<String>,
    /// Live mode selected drive index (for HPA/DCO/ATA security screens).
    #[cfg(all(feature = "live", target_os = "linux"))]
    pub live_drive_index: usize,
    /// Live mode action confirmation state.
    #[cfg(all(feature = "live", target_os = "linux"))]
    pub live_confirm_action: Option<String>,
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
            screen: AppScreen::MainMenu,
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
            focused_drive_index: None,
            main_menu_index: 0,
            clone_source_index: None,
            clone_target_index: None,
            clone_mode: "block".to_string(),
            keyboard_locked: false,
            keyboard_lock_buffer: Vec::new(),
            settings_index: 0,
            forensic_progress_pct: 0.0,
            forensic_result_lines: Vec::new(),
            health_display_lines: Vec::new(),
            clone_progress_fraction: 0.0,
            clone_throughput: String::new(),
            partition_lines: Vec::new(),
            #[cfg(all(feature = "live", target_os = "linux"))]
            live_mode: false,
            #[cfg(all(feature = "live", target_os = "linux"))]
            live_status_lines: Vec::new(),
            #[cfg(all(feature = "live", target_os = "linux"))]
            live_drive_index: 0,
            #[cfg(all(feature = "live", target_os = "linux"))]
            live_confirm_action: None,
        };

        // Detect live environment if compiled with live feature on Linux.
        #[cfg(all(feature = "live", target_os = "linux"))]
        {
            let detection = drivewipe_live::detect::detect_live_environment();
            app.live_mode = detection.is_live;
            if app.live_mode {
                app.live_status_lines.push("DRIVEWIPE LIVE".to_string());
                if detection.kernel_module_present {
                    app.live_status_lines
                        .push("Kernel module: loaded".to_string());
                }
                if detection.pxe_booted {
                    app.live_status_lines.push("Boot: PXE network".to_string());
                }
            }
        }

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
            #[allow(unused_mut)]
            Ok(mut drives) => {
                // When running in live mode on Linux, probe each SATA drive for hidden
                // areas and ATA security state using drivewipe-live.
                #[cfg(all(feature = "live", target_os = "linux"))]
                if self.live_mode {
                    for drive in &mut drives {
                        if drive.transport == drivewipe_core::types::Transport::Sata {
                            let dev = drive.path.display().to_string();

                            if let Ok(hpa) = drivewipe_live::hpa::detect_hpa(&dev) {
                                drive.hidden_areas.hpa_enabled = hpa.hpa_present;
                                if hpa.hpa_present {
                                    drive.hidden_areas.hpa_size = Some(hpa.hpa_sectors * 512);
                                    drive.hidden_areas.hpa_native_max_lba =
                                        Some(hpa.native_max_lba);
                                    drive.hidden_areas.hpa_current_max_lba =
                                        Some(hpa.current_max_lba);
                                }
                            }

                            if let Ok(dco) = drivewipe_live::dco::detect_dco(&dev) {
                                drive.hidden_areas.dco_enabled = dco.dco_present;
                                if dco.dco_present {
                                    drive.hidden_areas.dco_size = Some(dco.dco_hidden_bytes);
                                    drive.hidden_areas.dco_factory_max_lba =
                                        Some(dco.factory_max_lba);
                                    drive.hidden_areas.dco_features_restricted =
                                        dco.restricted_features;
                                }
                            }

                            if let Ok(sec) = drivewipe_live::ata_security::query_ata_security(&dev)
                            {
                                drive.ata_security = sec.to_core_state();
                            }
                        }
                    }
                }

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
        // Keyboard lock: when locked, only buffer keys for unlock sequence.
        if self.keyboard_locked {
            if let KeyCode::Char(c) = key.code {
                self.keyboard_lock_buffer.push(c);
                let unlock_seq: Vec<char> = self.config.keyboard_lock_sequence.chars().collect();
                if self.keyboard_lock_buffer.len() > unlock_seq.len() {
                    let start = self.keyboard_lock_buffer.len() - unlock_seq.len();
                    if self.keyboard_lock_buffer[start..] == unlock_seq[..] {
                        self.keyboard_locked = false;
                        self.keyboard_lock_buffer.clear();
                        self.log_push("Keyboard unlocked".into());
                    }
                }
                // Trim buffer to prevent unbounded growth.
                if self.keyboard_lock_buffer.len() > 100 {
                    self.keyboard_lock_buffer.drain(0..50);
                }
            }
            return;
        }

        // Global quit confirmation overlay.
        if self.quit_confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if self.screen == AppScreen::Wiping || self.screen == AppScreen::CloneProgress {
                        self.cancel_token.cancel();
                        self.log_push("Cancelling active operations...".into());
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
            if self.screen == AppScreen::Wiping || self.screen == AppScreen::CloneProgress {
                self.quit_confirm = true;
            } else {
                self.should_quit = true;
            }
            return;
        }

        // Ctrl-L toggles keyboard lock from any screen.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.keyboard_locked = true;
            self.keyboard_lock_buffer.clear();
            self.log_push("Keyboard locked (type unlock sequence to unlock)".into());
            return;
        }

        // '?' key shows help from any screen except Confirm input.
        if key.code == KeyCode::Char('?') && self.screen != AppScreen::Confirm {
            if self.screen == AppScreen::Help {
                self.screen = AppScreen::MainMenu;
            } else {
                self.screen = AppScreen::Help;
            }
            return;
        }

        match &self.screen.clone() {
            AppScreen::MainMenu => self.handle_main_menu_key(key),
            AppScreen::DriveSelection => self.handle_drive_selection_key(key),
            AppScreen::MethodSelect => self.handle_method_select_key(key),
            AppScreen::Confirm => self.handle_confirm_key(key),
            AppScreen::Wiping => self.handle_wiping_key(key),
            AppScreen::Done => self.handle_done_key(key),
            AppScreen::Error(_) => self.handle_error_key(key),
            AppScreen::Help => self.handle_help_key(key),
            AppScreen::DriveHealth => self.handle_health_key(key),
            AppScreen::HealthComparison => self.handle_health_key(key),
            AppScreen::CloneSetup => self.handle_clone_setup_key(key),
            AppScreen::CloneProgress => self.handle_wiping_key(key),
            AppScreen::PartitionManager => self.handle_partition_key(key),
            AppScreen::ForensicAnalysis => self.handle_forensic_key(key),
            AppScreen::Settings => self.handle_settings_key(key),
            #[cfg(all(feature = "live", target_os = "linux"))]
            AppScreen::LiveDashboard
            | AppScreen::HpaDcoManager
            | AppScreen::AtaSecurityManager
            | AppScreen::KernelModuleStatus => self.handle_live_screen_key(key),
        }
    }

    #[cfg(all(feature = "live", target_os = "linux"))]
    fn handle_live_screen_key(&mut self, key: KeyEvent) {
        // Handle confirmation flow first — if a destructive action is pending,
        // 'y' confirms and anything else cancels.
        if let Some(ref action) = self.live_confirm_action.clone() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.live_confirm_action = None;
                    self.execute_live_confirmed_action(action);
                }
                _ => {
                    self.log_push("Action cancelled".into());
                    self.live_confirm_action = None;
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.live_drive_index > 0 {
                    self.live_drive_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.drives.is_empty() && self.live_drive_index < self.drives.len() - 1 {
                    self.live_drive_index += 1;
                }
            }

            // ── HPA/DCO Manager actions ────────────────────────────────
            KeyCode::Char('d') if self.screen == AppScreen::HpaDcoManager => {
                self.live_detect_hidden_areas();
            }
            KeyCode::Char('r') if self.screen == AppScreen::HpaDcoManager => {
                self.live_confirm_action = Some("Remove HPA".to_string());
                self.log_push("Press 'y' to confirm HPA removal, any other key to cancel".into());
            }
            KeyCode::Char('R') if self.screen == AppScreen::HpaDcoManager => {
                self.live_confirm_action = Some("Restore DCO".to_string());
                self.log_push("Press 'y' to confirm DCO restore, any other key to cancel".into());
            }
            KeyCode::Char('F') if self.screen == AppScreen::HpaDcoManager => {
                self.live_freeze_dco();
            }

            // ── ATA Security Manager actions ───────────────────────────
            KeyCode::Char('u') if self.screen == AppScreen::AtaSecurityManager => {
                self.live_unfreeze_drives();
            }

            // ── Dashboard quick-nav ────────────────────────────────────
            KeyCode::Char('1') if self.screen == AppScreen::LiveDashboard => {
                self.refresh_drives();
                self.live_drive_index = 0;
                self.screen = AppScreen::HpaDcoManager;
            }
            KeyCode::Char('2') if self.screen == AppScreen::LiveDashboard => {
                self.refresh_drives();
                self.live_drive_index = 0;
                self.screen = AppScreen::AtaSecurityManager;
            }
            KeyCode::Char('3') if self.screen == AppScreen::LiveDashboard => {
                self.screen = AppScreen::KernelModuleStatus;
            }

            // ── Kernel Module Status refresh ───────────────────────────
            KeyCode::Char('r') if self.screen == AppScreen::KernelModuleStatus => {
                self.live_refresh_capabilities();
            }

            _ => {}
        }
    }

    /// Detect HPA and DCO on the currently selected drive and update the
    /// drive's `hidden_areas` field with the results.
    #[cfg(all(feature = "live", target_os = "linux"))]
    fn live_detect_hidden_areas(&mut self) {
        if self.drives.is_empty() {
            self.log_push("No drives available".into());
            return;
        }
        let idx = self.live_drive_index.min(self.drives.len() - 1);
        let device_path = self.drives[idx].path.display().to_string();

        self.log_push(format!("Detecting hidden areas on {}...", device_path));

        // HPA detection
        match drivewipe_live::hpa::detect_hpa(&device_path) {
            Ok(hpa) => {
                self.drives[idx].hidden_areas.hpa_enabled = hpa.hpa_present;
                if hpa.hpa_present {
                    self.drives[idx].hidden_areas.hpa_size = Some(hpa.hpa_sectors * 512);
                    self.drives[idx].hidden_areas.hpa_native_max_lba = Some(hpa.native_max_lba);
                    self.drives[idx].hidden_areas.hpa_current_max_lba = Some(hpa.current_max_lba);
                    self.log_push(format!(
                        "  HPA detected: {} sectors hidden ({} bytes)",
                        hpa.hpa_sectors,
                        drivewipe_core::types::format_bytes(hpa.hpa_sectors * 512)
                    ));
                } else {
                    self.drives[idx].hidden_areas.hpa_size = None;
                    self.log_push("  HPA: none detected".into());
                }
            }
            Err(e) => {
                self.log_push(format!("  HPA detection failed: {e}"));
            }
        }

        // DCO detection
        match drivewipe_live::dco::detect_dco(&device_path) {
            Ok(dco) => {
                self.drives[idx].hidden_areas.dco_enabled = dco.dco_present;
                if dco.dco_present {
                    self.drives[idx].hidden_areas.dco_size = Some(dco.dco_hidden_bytes);
                    self.drives[idx].hidden_areas.dco_factory_max_lba = Some(dco.factory_max_lba);
                    self.drives[idx].hidden_areas.dco_features_restricted =
                        dco.restricted_features.clone();
                    self.log_push(format!(
                        "  DCO detected: {} sectors hidden ({})",
                        dco.dco_hidden_sectors,
                        drivewipe_core::types::format_bytes(dco.dco_hidden_bytes)
                    ));
                    if !dco.restricted_features.is_empty() {
                        self.log_push(format!(
                            "  DCO restrictions: {}",
                            dco.restricted_features.join(", ")
                        ));
                    }
                } else {
                    self.drives[idx].hidden_areas.dco_size = None;
                    self.log_push("  DCO: none detected".into());
                }
            }
            Err(e) => {
                self.log_push(format!("  DCO detection failed: {e}"));
            }
        }

        // ATA security state
        match drivewipe_live::ata_security::query_ata_security(&device_path) {
            Ok(info) => {
                self.drives[idx].ata_security = info.to_core_state();
                self.log_push(format!("  ATA Security: {}", info.summary));
            }
            Err(e) => {
                self.log_push(format!("  ATA security query failed: {e}"));
            }
        }
    }

    /// Execute a confirmed destructive live action.
    #[cfg(all(feature = "live", target_os = "linux"))]
    fn execute_live_confirmed_action(&mut self, action: &str) {
        if self.drives.is_empty() {
            self.log_push("No drives available".into());
            return;
        }
        let idx = self.live_drive_index.min(self.drives.len() - 1);
        let device_path = self.drives[idx].path.display().to_string();

        match action {
            "Remove HPA" => {
                self.log_push(format!("Removing HPA on {}...", device_path));
                match drivewipe_live::hpa::remove_hpa(&device_path) {
                    Ok(result) => {
                        self.drives[idx].hidden_areas.hpa_enabled = result.hpa_present;
                        self.drives[idx].hidden_areas.hpa_size = if result.hpa_present {
                            Some(result.hpa_sectors * 512)
                        } else {
                            None
                        };
                        if result.hpa_present {
                            self.log_push(format!(
                                "WARNING: HPA still present after removal ({} sectors remaining)",
                                result.hpa_sectors
                            ));
                        } else {
                            self.log_push(
                                "HPA removed successfully — full capacity restored".into(),
                            );
                        }
                    }
                    Err(e) => {
                        self.log_push(format!("HPA removal failed: {e}"));
                    }
                }
            }
            "Restore DCO" => {
                self.log_push(format!("Restoring DCO on {}...", device_path));
                match drivewipe_live::dco::restore_dco(&device_path) {
                    Ok(_) => {
                        self.drives[idx].hidden_areas.dco_enabled = false;
                        self.drives[idx].hidden_areas.dco_size = None;
                        self.drives[idx]
                            .hidden_areas
                            .dco_features_restricted
                            .clear();
                        self.log_push(
                            "DCO restored to factory settings — power cycle recommended".into(),
                        );
                    }
                    Err(e) => {
                        self.log_push(format!("DCO restore failed: {e}"));
                    }
                }
            }
            _ => {
                self.log_push(format!("Unknown action: {action}"));
            }
        }
    }

    /// Freeze DCO on the currently selected drive.
    #[cfg(all(feature = "live", target_os = "linux"))]
    fn live_freeze_dco(&mut self) {
        if self.drives.is_empty() {
            self.log_push("No drives available".into());
            return;
        }
        let idx = self.live_drive_index.min(self.drives.len() - 1);
        let device_path = self.drives[idx].path.display().to_string();

        self.log_push(format!("Freezing DCO on {}...", device_path));
        match drivewipe_live::dco::freeze_dco(&device_path) {
            Ok(()) => {
                self.log_push("DCO frozen — no further DCO changes until power cycle".into());
            }
            Err(e) => {
                self.log_push(format!("DCO freeze failed: {e}"));
            }
        }
    }

    /// Unfreeze all SATA drives via suspend/resume cycle.
    #[cfg(all(feature = "live", target_os = "linux"))]
    fn live_unfreeze_drives(&mut self) {
        self.log_push("Checking for frozen drives...".into());

        if !drivewipe_live::unfreeze::any_drives_frozen() {
            self.log_push("No frozen drives detected — no action needed".into());
            return;
        }

        self.log_push("Frozen drives detected. Initiating suspend/resume cycle...".into());
        self.log_push("WARNING: System will briefly suspend to RAM".into());

        match drivewipe_live::unfreeze::unfreeze_drives() {
            Ok(()) => {
                self.log_push("All drives unfrozen successfully".into());
                // Re-query ATA security state for all SATA drives
                for drive in &mut self.drives {
                    if drive.transport == drivewipe_core::types::Transport::Sata {
                        let path = drive.path.display().to_string();
                        if let Ok(info) = drivewipe_live::ata_security::query_ata_security(&path) {
                            drive.ata_security = info.to_core_state();
                        }
                    }
                }
            }
            Err(e) => {
                self.log_push(format!("Unfreeze failed: {e}"));
                self.log_push(
                    "Some drives may still be frozen. Try manual unfreeze or power cycle.".into(),
                );
            }
        }
    }

    /// Refresh live capabilities (kernel module status, system info).
    #[cfg(all(feature = "live", target_os = "linux"))]
    fn live_refresh_capabilities(&mut self) {
        self.log_push("Refreshing kernel module status...".into());
        let caps = drivewipe_live::capabilities::LiveCapabilities::probe();

        self.live_status_lines.clear();
        self.live_status_lines.push("DRIVEWIPE LIVE".to_string());

        if caps.kernel_module.loaded {
            let ver = caps.kernel_module.version.as_deref().unwrap_or("unknown");
            self.live_status_lines
                .push(format!("Kernel module: loaded (v{})", ver));
            self.log_push(format!(
                "Module v{} — capabilities: {:#06x}",
                ver, caps.kernel_module.raw_capabilities
            ));
        } else {
            self.live_status_lines
                .push("Kernel module: not loaded".to_string());
            self.log_push("Kernel module not loaded — using userspace fallback".into());
        }

        if caps.system.pxe_booted {
            self.live_status_lines.push("Boot: PXE network".to_string());
        }

        self.live_status_lines.push(format!(
            "System: {} cores, {} RAM, kernel {}",
            caps.system.cpu_cores,
            drivewipe_core::types::format_bytes(caps.system.total_ram),
            caps.system.kernel_version
        ));

        self.live_status_lines.push(format!(
            "Hardware: {} SATA, {} NVMe, {} USB drives",
            caps.hardware.sata_drives, caps.hardware.nvme_drives, caps.hardware.usb_drives
        ));

        self.log_push("Capabilities refreshed".into());
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
                    self.screen = AppScreen::MainMenu;
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
                self.screen = AppScreen::MainMenu;
            }
            _ => {}
        }
    }

    // ── Error keys ──────────────────────────────────────────────────────

    fn handle_error_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                self.screen = AppScreen::MainMenu;
            }
            _ => {}
        }
    }

    // ── Help keys ───────────────────────────────────────────────────────

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
                self.screen = AppScreen::MainMenu;
            }
            _ => {}
        }
    }

    // ── Main menu keys ─────────────────────────────────────────────────

    fn handle_main_menu_key(&mut self, key: KeyEvent) {
        let menu_items = self.main_menu_item_count();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.main_menu_index > 0 {
                    self.main_menu_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.main_menu_index < menu_items - 1 {
                    self.main_menu_index += 1;
                }
            }
            KeyCode::Enter => self.activate_main_menu_item(),
            // Quick access keys
            KeyCode::Char('w') | KeyCode::Char('1') => {
                self.refresh_drives();
                self.screen = AppScreen::DriveSelection;
            }
            KeyCode::Char('h') | KeyCode::Char('2') => {
                self.refresh_drives();
                self.screen = AppScreen::DriveHealth;
            }
            KeyCode::Char('c') | KeyCode::Char('3') => {
                self.refresh_drives();
                self.clone_source_index = None;
                self.clone_target_index = None;
                self.screen = AppScreen::CloneSetup;
            }
            KeyCode::Char('p') | KeyCode::Char('4') => {
                self.refresh_drives();
                self.screen = AppScreen::PartitionManager;
            }
            KeyCode::Char('f') | KeyCode::Char('5') => {
                self.refresh_drives();
                self.forensic_result_lines.clear();
                self.screen = AppScreen::ForensicAnalysis;
            }
            KeyCode::Char('s') | KeyCode::Char('6') => {
                self.settings_index = 0;
                self.screen = AppScreen::Settings;
            }
            _ => {}
        }
    }

    /// Number of items in the main menu (varies if live mode is active).
    pub fn main_menu_item_count(&self) -> usize {
        #[cfg(all(feature = "live", target_os = "linux"))]
        if self.live_mode {
            return 11; // 7 base + 4 live items
        }
        7
    }

    /// Activate the currently selected main menu item.
    fn activate_main_menu_item(&mut self) {
        match self.main_menu_index {
            0 => {
                self.refresh_drives();
                self.screen = AppScreen::DriveSelection;
            }
            1 => {
                self.refresh_drives();
                self.screen = AppScreen::DriveHealth;
            }
            2 => {
                self.refresh_drives();
                self.clone_source_index = None;
                self.clone_target_index = None;
                self.clone_mode = "block".to_string();
                self.screen = AppScreen::CloneSetup;
            }
            3 => {
                self.refresh_drives();
                self.screen = AppScreen::PartitionManager;
            }
            4 => {
                self.refresh_drives();
                self.forensic_result_lines.clear();
                self.forensic_progress_pct = 0.0;
                self.screen = AppScreen::ForensicAnalysis;
            }
            5 => {
                self.settings_index = 0;
                self.screen = AppScreen::Settings;
            }
            6 => {
                #[cfg(all(feature = "live", target_os = "linux"))]
                if self.live_mode {
                    // In live mode, index 6 = Live Dashboard
                    self.screen = AppScreen::LiveDashboard;
                    return;
                }
                self.should_quit = true;
            }
            #[cfg(all(feature = "live", target_os = "linux"))]
            7 => {
                self.refresh_drives();
                self.live_drive_index = 0;
                self.screen = AppScreen::HpaDcoManager;
            }
            #[cfg(all(feature = "live", target_os = "linux"))]
            8 => {
                self.refresh_drives();
                self.live_drive_index = 0;
                self.screen = AppScreen::AtaSecurityManager;
            }
            #[cfg(all(feature = "live", target_os = "linux"))]
            9 => {
                self.screen = AppScreen::KernelModuleStatus;
            }
            #[cfg(feature = "live")]
            10 => {
                self.should_quit = true;
            }
            _ => {
                // Non-live mode index 6+ or unknown
                self.should_quit = true;
            }
        }
    }

    // ── Health keys ────────────────────────────────────────────────────

    fn handle_health_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let max = self.drives.len().saturating_sub(1);
                let i = match self.table_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                self.table_state.select(Some(i.min(max)));
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
            KeyCode::Enter => {
                // Show health details for selected drive
                if let Some(i) = self.table_state.selected() {
                    self.focused_drive_index = Some(i);
                    self.health_display_lines = self.build_health_display(i);
                }
            }
            KeyCode::Char('r') => {
                self.refresh_drives();
            }
            _ => {}
        }
    }

    fn build_health_display(&self, drive_idx: usize) -> Vec<String> {
        let mut lines = Vec::new();
        if drive_idx >= self.drives.len() {
            return lines;
        }
        let drive = &self.drives[drive_idx];
        lines.push(format!("Drive: {} ({})", drive.model, drive.path.display()));
        lines.push(format!(
            "Serial: {}",
            if drive.serial.is_empty() {
                "N/A"
            } else {
                &drive.serial
            }
        ));
        lines.push(format!("Type: {} / {}", drive.drive_type, drive.transport));
        lines.push(format!("Capacity: {}", drive.capacity_display()));
        lines.push(String::new());
        match drive.smart_healthy {
            Some(true) => lines.push("SMART Status: HEALTHY".to_string()),
            Some(false) => lines.push("SMART Status: UNHEALTHY".to_string()),
            None => lines.push("SMART Status: Not available".to_string()),
        }
        lines.push(format!("Block size: {} bytes", drive.block_size));
        lines.push(format!(
            "Supports TRIM: {}",
            if drive.supports_trim { "Yes" } else { "No" }
        ));
        lines.push(format!(
            "Self-encrypting: {}",
            if drive.is_sed { "Yes" } else { "No" }
        ));
        if let Some(ref pt) = drive.partition_table {
            lines.push(format!(
                "Partition table: {} ({} partitions)",
                pt, drive.partition_count
            ));
        }
        if drive.hidden_areas.hpa_enabled || drive.hidden_areas.dco_enabled {
            let hpa = drive
                .hidden_areas
                .hpa_size
                .map(format_bytes)
                .unwrap_or_default();
            let dco = drive
                .hidden_areas
                .dco_size
                .map(format_bytes)
                .unwrap_or_default();
            lines.push(format!("Hidden areas: HPA={}, DCO={}", hpa, dco));
        }
        lines
    }

    // ── Clone setup keys ───────────────────────────────────────────────

    fn handle_clone_setup_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let max = self.drives.len().saturating_sub(1);
                let i = match self.table_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                self.table_state.select(Some(i.min(max)));
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
            KeyCode::Char('s') => {
                // Set source drive
                if let Some(i) = self.table_state.selected() {
                    self.clone_source_index = Some(i);
                    self.log_push(format!("Clone source: {}", self.drives[i].path.display()));
                }
            }
            KeyCode::Char('t') => {
                // Set target drive
                if let Some(i) = self.table_state.selected() {
                    self.clone_target_index = Some(i);
                    self.log_push(format!("Clone target: {}", self.drives[i].path.display()));
                }
            }
            KeyCode::Char('m') => {
                // Toggle clone mode
                self.clone_mode = if self.clone_mode == "block" {
                    "partition".to_string()
                } else {
                    "block".to_string()
                };
                self.log_push(format!("Clone mode: {}", self.clone_mode));
            }
            KeyCode::Enter => {
                if self.clone_source_index.is_some() && self.clone_target_index.is_some() {
                    if self.clone_source_index == self.clone_target_index {
                        self.log_push("Source and target cannot be the same drive".into());
                    } else {
                        self.clone_progress_fraction = 0.0;
                        self.clone_throughput = String::new();
                        self.start_clone();
                    }
                } else {
                    self.log_push("Select both source (s) and target (t) drives first".into());
                }
            }
            _ => {}
        }
    }

    // ── Partition keys ─────────────────────────────────────────────────

    fn handle_partition_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let max = self.drives.len().saturating_sub(1);
                let i = match self.table_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                self.table_state.select(Some(i.min(max)));
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
            KeyCode::Enter => {
                if let Some(i) = self.table_state.selected() {
                    self.focused_drive_index = Some(i);
                    self.read_partition_table(i);
                }
            }
            KeyCode::Char('r') => {
                self.refresh_drives();
            }
            _ => {}
        }
    }

    // ── Forensic keys ──────────────────────────────────────────────────

    fn handle_forensic_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let max = self.drives.len().saturating_sub(1);
                let i = match self.table_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                self.table_state.select(Some(i.min(max)));
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
            KeyCode::Enter => {
                if let Some(i) = self.table_state.selected() {
                    self.focused_drive_index = Some(i);
                    self.start_forensic_scan(i);
                }
            }
            KeyCode::Char('r') => {
                self.refresh_drives();
            }
            _ => {}
        }
    }

    // ── Settings keys ──────────────────────────────────────────────────

    fn handle_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = AppScreen::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.settings_index > 0 {
                    self.settings_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.settings_index < 7 {
                    self.settings_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Toggle boolean settings
                match self.settings_index {
                    0 => self.config.auto_report_json = !self.config.auto_report_json,
                    1 => self.config.notifications_enabled = !self.config.notifications_enabled,
                    2 => {
                        self.config.sleep_prevention_enabled = !self.config.sleep_prevention_enabled
                    }
                    3 => self.config.auto_health_pre_wipe = !self.config.auto_health_pre_wipe,
                    _ => {}
                }
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
                    p.current_pass = pass_number;

                    // Apply exponential smoothing to reduce throughput fluctuations
                    // alpha=0.3 balances responsiveness with stability
                    const SMOOTHING_FACTOR: f64 = 0.3;
                    if p.throughput_bps == 0.0 {
                        // First sample, use raw value
                        p.throughput_bps = throughput_bps;
                    } else {
                        // Exponential moving average: new = alpha * raw + (1-alpha) * old
                        p.throughput_bps = SMOOTHING_FACTOR * throughput_bps
                            + (1.0 - SMOOTHING_FACTOR) * p.throughput_bps;
                    }
                    p.push_throughput(p.throughput_bps);

                    // Update advanced stats for modern UI
                    p.current_sector = bytes_written / 512; // Assume 512-byte sectors
                    p.write_operations += 1;

                    // Calculate IOPS (operations per second)
                    let now = Instant::now();
                    let elapsed = now.duration_since(p.last_update).as_secs_f64();
                    if elapsed >= 0.1 {
                        // Update IOPS every 100ms
                        p.iops = 1.0 / elapsed.max(0.001);
                        p.last_update = now;
                    }
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
            // Health events
            ProgressEvent::HealthCheckStarted { .. } => {
                self.log_push("Health check started".into());
            }
            ProgressEvent::HealthCheckCompleted { .. } => {
                self.log_push("Health check completed".into());
            }
            ProgressEvent::HealthSnapshotSaved { .. } => {
                self.log_push("Health snapshot saved".into());
            }

            // Clone events
            ProgressEvent::CloneStarted { .. } => {
                self.log_push("Clone operation started".into());
            }
            ProgressEvent::CloneProgress {
                bytes_copied,
                total_bytes,
                throughput_bps,
                ..
            } => {
                if total_bytes > 0 {
                    self.clone_progress_fraction = bytes_copied as f64 / total_bytes as f64;
                }
                self.clone_throughput = format_throughput(throughput_bps);
            }
            ProgressEvent::CloneCompleted { duration_secs, .. } => {
                self.log_push(format!("Clone completed in {duration_secs:.1}s"));
                self.clone_progress_fraction = 1.0;
            }

            // Partition events
            ProgressEvent::PartitionOperationStarted { operation, .. } => {
                self.log_push(format!("Partition operation started: {operation}"));
            }
            ProgressEvent::PartitionOperationCompleted { operation, .. } => {
                self.log_push(format!("Partition operation completed: {operation}"));
            }

            // Forensic events
            ProgressEvent::ForensicScanStarted { .. } => {
                self.log_push("Forensic scan started".into());
                self.forensic_progress_pct = 0.0;
            }
            ProgressEvent::ForensicScanProgress {
                bytes_scanned,
                total_bytes,
                ..
            } => {
                if total_bytes > 0 {
                    self.forensic_progress_pct =
                        (bytes_scanned as f64 / total_bytes as f64 * 100.0) as f32;
                }
            }
            ProgressEvent::ForensicScanCompleted { duration_secs, .. } => {
                self.log_push(format!("Forensic scan completed in {duration_secs:.1}s"));
                self.forensic_progress_pct = 100.0;
            }

            // Catch any future new events
            #[allow(unreachable_patterns)]
            _ => {
                log::debug!("Unhandled progress event: {:?}", event);
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

            // Log debug file locations
            let io_debug_log = std::env::temp_dir().join("drivewipe_debug.log");
            let thread_debug_log = std::env::temp_dir().join("drivewipe_thread_debug.log");
            self.log_push(format!("I/O log: {}", io_debug_log.display()));
            self.log_push(format!("Thread log: {}", thread_debug_log.display()));

            std::thread::spawn(move || {
                let debug_log = std::env::temp_dir().join("drivewipe_thread_debug.log");
                let write_debug = |msg: &str| {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&debug_log)
                        .and_then(|mut f| {
                            use std::io::Write;
                            writeln!(f, "{}", msg)
                        });
                };

                write_debug("=== WIPE THREAD STARTED ===");
                write_debug(&format!("Device: {}", drive_info.path.display()));
                write_debug(&format!("Method: {}", method));
                // Build a fresh registry that contains ALL methods (software +
                // firmware + custom). We then find our method by id and consume the
                // registry to extract the owned Box<dyn WipeMethod>.
                write_debug("Building method registry...");
                let mut registry = WipeMethodRegistry::new();
                registry.register_custom_methods(&config);

                let boxed_method = match registry.into_method(&method) {
                    Some(m) => m,
                    None => {
                        write_debug(&format!("ERROR: Method not found: {}", method));
                        let _ = progress_tx.send(ProgressEvent::Error {
                            session_id: Uuid::new_v4(),
                            message: format!("Method not found for session: {method}"),
                        });
                        return;
                    }
                };

                write_debug("Boxed method created");
                let session = WipeSession::new(drive_info.clone(), boxed_method, config);
                write_debug(&format!("Session created, ID: {}", session.session_id));

                // Open the device for raw I/O.
                write_debug("Opening device for I/O...");
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
                    Ok(d) => {
                        write_debug("Device opened SUCCESSFULLY");
                        d
                    }
                    Err(e) => {
                        let err_msg =
                            format!("Failed to open {}: {}", drive_info.path.display(), e);
                        write_debug(&format!("Device open FAILED: {}", err_msg));
                        eprintln!("DEVICE OPEN ERROR: {}", err_msg); // Also print to stderr for debugging
                        let _ = progress_tx.send(ProgressEvent::Error {
                            session_id: session.session_id,
                            message: err_msg,
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

                write_debug("Calling session.execute()...");
                match session.execute(&mut device, &progress_tx, &cancel_token, None) {
                    Ok(result) => {
                        write_debug(&format!(
                            "session.execute() returned OK, outcome: {}",
                            result.outcome
                        ));
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
                        write_debug(&format!("session.execute() returned ERROR: {}", e));
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
                write_debug("=== WIPE THREAD ENDING ===");
            });
        }
    }

    // ── Start clone operation ────────────────────────────────────────────

    fn start_clone(&mut self) {
        let source_idx = match self.clone_source_index {
            Some(i) => i,
            None => return,
        };
        let target_idx = match self.clone_target_index {
            Some(i) => i,
            None => return,
        };

        if source_idx >= self.drives.len() || target_idx >= self.drives.len() {
            self.log_push("Invalid drive selection".into());
            return;
        }

        let source_info = self.drives[source_idx].clone();
        let target_info = self.drives[target_idx].clone();
        let clone_mode = self.clone_mode.clone();
        let cancel_token = self.cancel_token.clone();

        let progress_tx = match &self.progress_tx {
            Some(tx) => tx.clone(),
            None => {
                self.log_push("No progress channel available".into());
                return;
            }
        };

        self.screen = AppScreen::CloneProgress;
        self.log_push(format!(
            "Starting {} clone: {} -> {}",
            clone_mode,
            source_info.path.display(),
            target_info.path.display(),
        ));

        std::thread::spawn(move || {
            use drivewipe_core::clone::{CloneConfig, CloneMode, CompressionMode};

            let mode = if clone_mode == "partition" {
                CloneMode::Partition
            } else {
                CloneMode::Block
            };

            let config = CloneConfig {
                source: source_info.path.clone(),
                target: target_info.path.clone(),
                mode,
                compression: CompressionMode::None,
                encrypt: false,
                verify: true,
                block_size: 4 * 1024 * 1024,
            };

            // Open source (read-only) and target (read-write)
            let source_result = drivewipe_core::io::open_device(&source_info.path, false);
            let target_result = drivewipe_core::io::open_device(&target_info.path, true);

            let mut source = match source_result {
                Ok(d) => d,
                Err(e) => {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Failed to open source: {}", e),
                    });
                    return;
                }
            };

            let mut target = match target_result {
                Ok(d) => d,
                Err(e) => {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Failed to open target: {}", e),
                    });
                    return;
                }
            };

            let result = match mode {
                CloneMode::Block => drivewipe_core::clone::block::clone_block(
                    source.as_mut(),
                    target.as_mut(),
                    &config,
                    &progress_tx,
                    &cancel_token,
                ),
                CloneMode::Partition => {
                    drivewipe_core::clone::partition_aware::clone_partition_aware(
                        source.as_mut(),
                        target.as_mut(),
                        &config,
                        &progress_tx,
                        &cancel_token,
                    )
                }
            };

            match result {
                Ok(clone_result) => {
                    let _ = progress_tx.send(ProgressEvent::CloneCompleted {
                        session_id: clone_result.session_id,
                        duration_secs: clone_result.duration_secs,
                        verified: clone_result.verification_passed.unwrap_or(false),
                    });
                }
                Err(e) => {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Clone failed: {}", e),
                    });
                }
            }
        });
    }

    // ── Start forensic scan ─────────────────────────────────────────────

    fn start_forensic_scan(&mut self, drive_idx: usize) {
        if drive_idx >= self.drives.len() {
            return;
        }

        let drive_info = self.drives[drive_idx].clone();
        let cancel_token = self.cancel_token.clone();

        let progress_tx = match &self.progress_tx {
            Some(tx) => tx.clone(),
            None => {
                self.log_push("No progress channel available".into());
                return;
            }
        };

        self.forensic_result_lines = vec![format!("Scanning {}...", drive_info.path.display())];
        self.forensic_progress_pct = 0.0;

        self.log_push(format!(
            "Starting forensic scan on {}",
            drive_info.path.display()
        ));

        std::thread::spawn(move || {
            use drivewipe_core::forensic::{ForensicConfig, ForensicSession};

            let config = ForensicConfig::default();
            let session = ForensicSession::new(config);

            let device_result = drivewipe_core::io::open_device(&drive_info.path, false);
            let mut device = match device_result {
                Ok(d) => d,
                Err(e) => {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Failed to open device for forensic scan: {}", e),
                    });
                    return;
                }
            };

            match session.execute(
                device.as_mut(),
                &drive_info.path.display().to_string(),
                &drive_info.serial,
                &progress_tx,
                &cancel_token,
            ) {
                Ok(result) => {
                    let _ = progress_tx.send(ProgressEvent::ForensicScanCompleted {
                        session_id: Uuid::new_v4(),
                        duration_secs: result.duration_secs,
                        total_findings: result.signature_hits.len() as u32,
                    });
                }
                Err(e) => {
                    let _ = progress_tx.send(ProgressEvent::Error {
                        session_id: Uuid::new_v4(),
                        message: format!("Forensic scan failed: {}", e),
                    });
                }
            }
        });
    }

    // ── Read partition table ────────────────────────────────────────────

    fn read_partition_table(&mut self, drive_idx: usize) {
        if drive_idx >= self.drives.len() {
            return;
        }

        let drive = &self.drives[drive_idx];
        self.partition_lines = vec![
            format!("Partition table for: {}", drive.path.display()),
            format!("Model: {}", drive.model),
            format!("Capacity: {}", drive.capacity_display()),
        ];

        let pt_type = drive.partition_table.as_deref().unwrap_or("Unknown");
        self.partition_lines
            .push(format!("Table type: {}", pt_type));
        self.partition_lines
            .push(format!("Partition count: {}", drive.partition_count));
        self.partition_lines.push(String::new());

        // Try to read the actual partition table from the device
        match drivewipe_core::io::open_device(&drive.path, false) {
            Ok(mut device) => {
                // Read first 34 sectors (enough for GPT header + entries)
                let read_size = 34 * 512;
                let mut buf = vec![0u8; read_size];
                match device.read_at(0, &mut buf) {
                    Ok(bytes_read) => {
                        match drivewipe_core::partition::PartitionTable::parse(&buf[..bytes_read]) {
                            Ok(table) => {
                                let partitions = table.partitions();
                                if partitions.is_empty() {
                                    self.partition_lines
                                        .push("No partitions found.".to_string());
                                } else {
                                    for p in partitions {
                                        self.partition_lines.push(format!(
                                            "  #{}: {} (LBA {}-{}, {})",
                                            p.index,
                                            if p.name.is_empty() {
                                                &p.type_id
                                            } else {
                                                &p.name
                                            },
                                            p.start_lba,
                                            p.end_lba,
                                            format_partition_size(p.size_bytes),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                self.partition_lines.push(format!("Parse error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        self.partition_lines.push(format!("Read error: {}", e));
                    }
                }
            }
            Err(e) => {
                self.partition_lines
                    .push(format!("Cannot open device: {}", e));
                self.partition_lines
                    .push("Use CLI with elevated privileges:".to_string());
                self.partition_lines.push(format!(
                    "  drivewipe partition list -d {}",
                    drive.path.display()
                ));
            }
        }
    }
}

fn format_partition_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000_000 {
        format!("{:.1} TB", bytes as f64 / 1_000_000_000_000.0)
    } else if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} B", bytes)
    }
}
