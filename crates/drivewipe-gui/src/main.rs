//! DriveWipe Graphical User Interface (GUI)
//!
//! The `drivewipe-gui` binary is a cross-platform desktop application built
//! using the `iced` framework. It provides a simplified, visual-first workflow
//! for all DriveWipe features, making complex sanitization tasks accessible
//! to non-technical operators.

mod screens;
mod theme;

use iced::futures::SinkExt;
use iced::widget::{column, container, text};
use iced::{Element, Length, Task as IcedTask};

/// Application screen states.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Menu,
    DriveSelect,
    MethodSelect,
    Confirm,
    WipeProgress,
    Health,
    Clone,
    Partition,
    Forensic,
    Settings,
}

/// All messages the application can handle.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    NavigateToMenu,
    NavigateBack,
    Navigate(Screen),

    // Drive selection
    ToggleDrive(usize),
    RefreshDrives,
    DrivesLoaded(Result<Vec<drivewipe_core::types::DriveInfo>, String>),
    ProceedToMethodSelect,

    // Method selection
    SelectMethod(usize),
    ProceedToConfirm,

    // Confirmation
    ConfirmInput(String),
    StartWipe,
    WipeEvent(drivewipe_core::progress::ProgressEvent),

    // Health
    ViewDriveHealth(usize),
    HealthLoaded(Result<Vec<String>, String>),

    // Clone
    SelectCloneDrive(usize),

    // Partition
    ViewPartitions(usize),
    PartitionsLoaded(Result<Vec<String>, String>),

    // Forensic
    RunForensicScan(usize),

    // Settings
    ToggleSetting(String, bool),
}

/// Application state.
struct DriveWipeApp {
    screen: Screen,
    drives: Vec<drivewipe_core::types::DriveInfo>,
    selected_drives: Vec<bool>,

    // Method selection
    methods: Vec<(String, String, u32)>,
    selected_method: Option<usize>,

    // Confirmation
    confirm_text: String,

    // Wipe progress
    wipe_device: String,
    wipe_method_name: String,
    wipe_fraction: f32,
    wipe_throughput: String,
    wipe_pass_info: String,
    wipe_complete: bool,
    wipe_running: bool,
    selected_method_id: String,
    selected_device_paths: Vec<std::path::PathBuf>,
    cancel_token: std::sync::Arc<drivewipe_core::session::CancellationToken>,

    // Health
    health_info: Vec<String>,

    // Clone
    clone_source: Option<usize>,
    clone_target: Option<usize>,
    clone_mode: String,

    // Partition
    partition_info: Vec<String>,

    // Forensic
    forensic_results: Vec<String>,

    // Settings
    setting_auto_report: bool,
    setting_notifications: bool,
    setting_sleep_prevention: bool,
    setting_auto_health: bool,
}

impl Default for DriveWipeApp {
    fn default() -> Self {
        let methods = vec![
            ("zero".into(), "Zero Fill".into(), 1),
            ("random".into(), "Random Data".into(), 1),
            ("dod_short".into(), "DoD 5220.22-M (3-pass)".into(), 3),
            ("dod_full".into(), "DoD 5220.22-M ECE (7-pass)".into(), 7),
            ("gutmann".into(), "Gutmann (35-pass)".into(), 35),
            ("drivewipe_secure".into(), "DriveWipe Secure".into(), 4),
        ];

        Self {
            screen: Screen::Menu,
            drives: Vec::new(),
            selected_drives: Vec::new(),
            methods,
            selected_method: None,
            confirm_text: String::new(),
            wipe_device: String::new(),
            wipe_method_name: String::new(),
            wipe_fraction: 0.0,
            wipe_throughput: String::new(),
            wipe_pass_info: String::new(),
            wipe_complete: false,
            wipe_running: false,
            selected_method_id: String::new(),
            selected_device_paths: Vec::new(),
            cancel_token: std::sync::Arc::new(drivewipe_core::session::CancellationToken::new()),
            health_info: Vec::new(),
            clone_source: None,
            clone_target: None,
            clone_mode: "Block".into(),
            partition_info: Vec::new(),
            forensic_results: Vec::new(),
            setting_auto_report: false,
            setting_notifications: true,
            setting_sleep_prevention: true,
            setting_auto_health: true,
        }
    }
}

impl DriveWipeApp {
    fn new() -> (Self, IcedTask<Message>) {
        (Self::default(), IcedTask::none())
    }

    fn title(&self) -> String {
        match self.screen {
            Screen::Menu => "DriveWipe".into(),
            Screen::DriveSelect => "DriveWipe - Select Drives".into(),
            Screen::MethodSelect => "DriveWipe - Select Method".into(),
            Screen::Confirm => "DriveWipe - Confirm".into(),
            Screen::WipeProgress => "DriveWipe - Wiping".into(),
            Screen::Health => "DriveWipe - Drive Health".into(),
            Screen::Clone => "DriveWipe - Drive Clone".into(),
            Screen::Partition => "DriveWipe - Partition Manager".into(),
            Screen::Forensic => "DriveWipe - Forensic Analysis".into(),
            Screen::Settings => "DriveWipe - Settings".into(),
        }
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        match message {
            Message::NavigateToMenu => {
                self.screen = Screen::Menu;
                IcedTask::none()
            }
            Message::NavigateBack => {
                self.screen = match self.screen {
                    Screen::MethodSelect => Screen::DriveSelect,
                    Screen::Confirm => Screen::MethodSelect,
                    _ => Screen::Menu,
                };
                IcedTask::none()
            }
            Message::Navigate(screen) => {
                self.screen = screen;
                IcedTask::none()
            }

            // Drive selection
            Message::ToggleDrive(i) => {
                if let Some(val) = self.selected_drives.get_mut(i) {
                    *val = !*val;
                }
                IcedTask::none()
            }
            Message::RefreshDrives => IcedTask::perform(
                async {
                    let enumerator = drivewipe_core::drive::create_enumerator();
                    enumerator.enumerate().await.map_err(|e| e.to_string())
                },
                Message::DrivesLoaded,
            ),
            Message::DrivesLoaded(result) => {
                match result {
                    Ok(drives) => {
                        self.drives = drives;
                        self.selected_drives = vec![false; self.drives.len()];
                    }
                    Err(_e) => {
                        // Error handling
                    }
                }
                IcedTask::none()
            }
            Message::ProceedToMethodSelect => {
                if self.selected_drives.iter().any(|s| *s) {
                    self.screen = Screen::MethodSelect;
                }
                IcedTask::none()
            }

            // Method selection
            Message::SelectMethod(i) => {
                self.selected_method = Some(i);
                IcedTask::none()
            }
            Message::ProceedToConfirm => {
                if self.selected_method.is_some() {
                    self.confirm_text.clear();
                    self.screen = Screen::Confirm;
                }
                IcedTask::none()
            }

            // Confirmation
            Message::ConfirmInput(val) => {
                self.confirm_text = val;
                IcedTask::none()
            }
            Message::StartWipe => {
                if self.confirm_text.trim() == "YES" {
                    self.wipe_fraction = 0.0;
                    self.wipe_complete = false;
                    self.wipe_running = true;
                    self.cancel_token =
                        std::sync::Arc::new(drivewipe_core::session::CancellationToken::new());

                    if let Some(mi) = self.selected_method {
                        if let Some((id, name, _)) = self.methods.get(mi) {
                            self.selected_method_id = id.clone();
                            self.wipe_method_name = name.clone();
                        }
                    }

                    self.selected_device_paths = self
                        .drives
                        .iter()
                        .zip(self.selected_drives.iter())
                        .filter(|(_, sel)| **sel)
                        .map(|(d, _)| d.path.clone())
                        .collect();

                    self.wipe_device = self
                        .selected_device_paths
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join(", ");

                    self.screen = Screen::WipeProgress;

                    let method_id = self.selected_method_id.clone();
                    let device_paths = self.selected_device_paths.clone();
                    let cancel_token = self.cancel_token.clone();

                    return IcedTask::run(
                        iced::stream::channel(
                            100,
                            move |output: iced::futures::channel::mpsc::Sender<Message>| async move {
                                let (tx, rx) = crossbeam_channel::unbounded();

                                let mut output_clone = output.clone();
                                tokio::spawn(async move {
                                    while let Ok(event) = rx.recv() {
                                        let _ = output_clone.send(Message::WipeEvent(event)).await;
                                    }
                                });

                                for path in device_paths {
                                    let enumerator = drivewipe_core::drive::create_enumerator();
                                    let drive_info = match enumerator.inspect(&path).await {
                                        Ok(info) => info,
                                        Err(_) => continue,
                                    };

                                    let local_reg = drivewipe_core::wipe::WipeMethodRegistry::new();
                                    let method = match local_reg.into_method(&method_id) {
                                        Some(m) => m,
                                        None => continue,
                                    };

                                    let session = drivewipe_core::session::WipeSession::new(
                                        drive_info,
                                        method,
                                        drivewipe_core::config::DriveWipeConfig::default(),
                                    );

                                    let mut device =
                                        match drivewipe_core::io::open_device(&path, true) {
                                            Ok(d) => d,
                                            Err(_) => continue,
                                        };

                                    let _ = session
                                        .execute(device.as_mut(), &tx, &cancel_token, None)
                                        .await;
                                }
                            },
                        ),
                        |msg| msg,
                    );
                }
                IcedTask::none()
            }
            Message::WipeEvent(event) => {
                use drivewipe_core::progress::ProgressEvent;
                match event {
                    ProgressEvent::SessionStarted { .. } => {
                        self.wipe_pass_info = "Started".into();
                    }
                    ProgressEvent::PassStarted { pass_number, .. } => {
                        self.wipe_pass_info = format!("Pass {}", pass_number);
                    }
                    ProgressEvent::BlockWritten {
                        bytes_written,
                        total_bytes,
                        throughput_bps,
                        ..
                    } => {
                        self.wipe_fraction = bytes_written as f32 / total_bytes as f32;
                        self.wipe_throughput =
                            format!("{:.1} MB/s", throughput_bps / (1024.0 * 1024.0));
                    }
                    ProgressEvent::Completed { .. } => {
                        self.wipe_fraction = 1.0;
                        self.wipe_complete = true;
                        self.wipe_pass_info = "Completed".into();
                    }
                    _ => {}
                }
                IcedTask::none()
            }

            // Health
            Message::ViewDriveHealth(i) => {
                self.health_info.clear();
                if let Some(drive) = self.drives.get(i) {
                    let path = drive.path.clone();
                    return IcedTask::perform(
                        async move {
                            let snapshot =
                                drivewipe_core::health::get_health(&path).await.map_err(
                                    |e: drivewipe_core::error::DriveWipeError| e.to_string(),
                                )?;
                            let mut lines = Vec::new();
                            lines.push(format!("Model: {}", snapshot.device_model));
                            if let Some(temp) = snapshot.temperature_celsius {
                                lines.push(format!("Temperature: {}°C", temp));
                            }
                            if let Some(smart) = snapshot.smart_data {
                                lines.push(format!("SMART Healthy: {}", smart.healthy));
                            }
                            Ok(lines)
                        },
                        Message::HealthLoaded,
                    );
                }
                IcedTask::none()
            }
            Message::HealthLoaded(result) => {
                match result {
                    Ok(lines) => self.health_info = lines,
                    Err(e) => self.health_info = vec![format!("Error: {}", e)],
                }
                IcedTask::none()
            }

            // Clone
            Message::SelectCloneDrive(i) => {
                if self.clone_source.is_none() {
                    self.clone_source = Some(i);
                } else if self.clone_target.is_none() && self.clone_source != Some(i) {
                    self.clone_target = Some(i);
                } else {
                    // Reset selection
                    self.clone_source = Some(i);
                    self.clone_target = None;
                }
                IcedTask::none()
            }

            // Partition
            Message::ViewPartitions(i) => {
                self.partition_info.clear();
                if let Some(drive) = self.drives.get(i) {
                    let path = drive.path.clone();
                    return IcedTask::perform(
                        async move {
                            let mut device = drivewipe_core::io::open_device(&path, false)
                                .map_err(|e: drivewipe_core::error::DriveWipeError| {
                                    e.to_string()
                                })?;
                            let mut buf = vec![0u8; 34 * 512];
                            device.read_at(0, &mut buf).map_err(|e| e.to_string())?;
                            let table = drivewipe_core::partition::PartitionTable::parse(&buf)
                                .map_err(|e| e.to_string())?;

                            let mut lines = Vec::new();
                            lines.push(format!("Table: {:?}", table.table_type()));
                            for part in table.partitions() {
                                lines.push(format!(
                                    "  #{}: {} - {} ({})",
                                    part.index,
                                    part.start_lba,
                                    part.end_lba,
                                    drivewipe_core::format_bytes(part.size_bytes)
                                ));
                            }
                            Ok(lines)
                        },
                        Message::PartitionsLoaded,
                    );
                }
                IcedTask::none()
            }
            Message::PartitionsLoaded(result) => {
                match result {
                    Ok(lines) => self.partition_info = lines,
                    Err(e) => self.partition_info = vec![format!("Error: {}", e)],
                }
                IcedTask::none()
            }

            // Forensic
            Message::RunForensicScan(i) => {
                self.forensic_results.clear();
                if let Some(drive) = self.drives.get(i) {
                    self.forensic_results.push(format!(
                        "Starting forensic scan of {}...",
                        drive.path.display()
                    ));
                }
                IcedTask::none()
            }

            // Settings
            Message::ToggleSetting(key, val) => {
                match key.as_str() {
                    "auto_report" => self.setting_auto_report = val,
                    "notifications" => self.setting_notifications = val,
                    "sleep_prevention" => self.setting_sleep_prevention = val,
                    "auto_health" => self.setting_auto_health = val,
                    _ => {}
                }
                IcedTask::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Menu => self.view_menu(),
            Screen::DriveSelect => screens::drive_select::view(&self.drives, &self.selected_drives),
            Screen::MethodSelect => {
                screens::method_select::view(&self.methods, self.selected_method)
            }
            Screen::Confirm => {
                let method_name = self
                    .selected_method
                    .and_then(|i| self.methods.get(i))
                    .map(|(_, n, _)| n.as_str())
                    .unwrap_or("Unknown");
                let count = self.selected_drives.iter().filter(|s| **s).count();
                screens::confirm::view(count, method_name, &self.confirm_text)
            }
            Screen::WipeProgress => screens::wipe_progress::view(
                &self.wipe_device,
                &self.wipe_method_name,
                self.wipe_fraction,
                &self.wipe_throughput,
                &self.wipe_pass_info,
                self.wipe_complete,
            ),
            Screen::Health => screens::health::view(&self.drives, &self.health_info),
            Screen::Clone => screens::clone::view(
                &self.drives,
                self.clone_source,
                self.clone_target,
                &self.clone_mode,
            ),
            Screen::Partition => screens::partition::view(&self.drives, &self.partition_info),
            Screen::Forensic => screens::forensic::view(&self.drives, &self.forensic_results),
            Screen::Settings => screens::settings::view(
                self.setting_auto_report,
                self.setting_notifications,
                self.setting_sleep_prevention,
                self.setting_auto_health,
            ),
        }
    }

    fn view_menu(&self) -> Element<'_, Message> {
        use iced::widget::button;

        let title = text("DriveWipe")
            .size(theme::FONT_SIZE_TITLE)
            .color(theme::PRIMARY);
        let subtitle = text("Secure Drive Management")
            .size(theme::FONT_SIZE_LG)
            .color(theme::PRIMARY_DARK);

        let version_text = text(format!("v{}", env!("CARGO_PKG_VERSION")))
            .size(theme::FONT_SIZE_SM)
            .color(theme::TEXT_MUTED);

        let menu_items: Vec<(&str, Screen, iced::Color)> = vec![
            ("Secure Wipe", Screen::DriveSelect, theme::DANGER),
            ("Drive Health", Screen::Health, theme::STATUS_HEALTHY),
            ("Drive Clone", Screen::Clone, theme::STATUS_INFO),
            ("Partition Manager", Screen::Partition, theme::SECONDARY),
            ("Forensic Analysis", Screen::Forensic, theme::STATUS_WARNING),
            ("Settings", Screen::Settings, theme::TEXT_SECONDARY),
        ];

        let mut menu_col = column![].spacing(theme::SPACING_MD);
        for (label, screen, color) in menu_items {
            menu_col = menu_col.push(
                button(text(label).size(theme::FONT_SIZE_LG).color(color))
                    .on_press(Message::Navigate(screen))
                    .width(Length::Fixed(300.0)),
            );
        }

        let content = column![title, subtitle, version_text, menu_col]
            .spacing(theme::SPACING_LG)
            .padding(theme::SPACING_XL)
            .align_x(iced::Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(theme::BG_DARK)),
                ..Default::default()
            })
            .into()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}

fn main() -> iced::Result {
    iced::application(DriveWipeApp::new, DriveWipeApp::update, DriveWipeApp::view)
        .title(DriveWipeApp::title)
        .subscription(DriveWipeApp::subscription)
        .window_size((900.0, 650.0))
        .run()
}
