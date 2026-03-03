//! DriveWipe Graphical User Interface (GUI)
//!
//! The `drivewipe-gui` binary is a cross-platform desktop application built
//! using the `iced` framework. It provides a simplified, visual-first workflow
//! for all DriveWipe features, making complex sanitization tasks accessible
//! to non-technical operators.

mod screens;
mod theme;

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
    ProceedToMethodSelect,

    // Method selection
    SelectMethod(usize),
    ProceedToConfirm,

    // Confirmation
    ConfirmInput(String),
    StartWipe,

    // Health
    ViewDriveHealth(usize),

    // Clone
    SelectCloneDrive(usize),

    // Partition
    ViewPartitions(usize),

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
            }
            Message::NavigateBack => {
                self.screen = match self.screen {
                    Screen::MethodSelect => Screen::DriveSelect,
                    Screen::Confirm => Screen::MethodSelect,
                    _ => Screen::Menu,
                };
            }
            Message::Navigate(screen) => {
                self.screen = screen;
            }

            // Drive selection
            Message::ToggleDrive(i) => {
                if let Some(val) = self.selected_drives.get_mut(i) {
                    *val = !*val;
                }
            }
            Message::RefreshDrives => {
                // In a real app, this would enumerate drives.
                // For now, clear and let user know.
                self.drives.clear();
                self.selected_drives.clear();
            }
            Message::ProceedToMethodSelect => {
                if self.selected_drives.iter().any(|s| *s) {
                    self.screen = Screen::MethodSelect;
                }
            }

            // Method selection
            Message::SelectMethod(i) => {
                self.selected_method = Some(i);
            }
            Message::ProceedToConfirm => {
                if self.selected_method.is_some() {
                    self.confirm_text.clear();
                    self.screen = Screen::Confirm;
                }
            }

            // Confirmation
            Message::ConfirmInput(val) => {
                self.confirm_text = val;
            }
            Message::StartWipe => {
                if self.confirm_text.trim() == "YES" {
                    self.wipe_fraction = 0.0;
                    self.wipe_complete = false;
                    self.wipe_throughput = "0 MB/s".into();
                    self.wipe_pass_info = "Pass 1".into();

                    if let Some(mi) = self.selected_method {
                        if let Some((_, name, _)) = self.methods.get(mi) {
                            self.wipe_method_name = name.clone();
                        }
                    }

                    self.wipe_device = self
                        .drives
                        .iter()
                        .zip(self.selected_drives.iter())
                        .filter(|(_, sel)| **sel)
                        .map(|(d, _)| d.path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");

                    self.screen = Screen::WipeProgress;
                }
            }

            // Health
            Message::ViewDriveHealth(i) => {
                self.health_info.clear();
                if let Some(drive) = self.drives.get(i) {
                    self.health_info.push(format!(
                        "Drive: {} ({})",
                        drive.path.display(),
                        drive.model
                    ));
                    let status = match drive.smart_healthy {
                        Some(true) => "PASSED",
                        Some(false) => "FAILED",
                        None => "N/A",
                    };
                    self.health_info.push(format!("SMART Status: {}", status));
                }
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
            }

            // Partition
            Message::ViewPartitions(i) => {
                self.partition_info.clear();
                if let Some(drive) = self.drives.get(i) {
                    self.partition_info.push(format!(
                        "Drive: {} ({})",
                        drive.path.display(),
                        drive.model
                    ));
                    let pt = drive.partition_table.as_deref().unwrap_or("Unknown");
                    self.partition_info.push(format!("Partition Table: {}", pt));
                    self.partition_info
                        .push(format!("Partitions: {}", drive.partition_count));
                }
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
            }

            // Settings
            Message::ToggleSetting(key, val) => match key.as_str() {
                "auto_report" => self.setting_auto_report = val,
                "notifications" => self.setting_notifications = val,
                "sleep_prevention" => self.setting_sleep_prevention = val,
                "auto_health" => self.setting_auto_health = val,
                _ => {}
            },
        }
        IcedTask::none()
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
}

fn main() -> iced::Result {
    iced::application(
        DriveWipeApp::title,
        DriveWipeApp::update,
        DriveWipeApp::view,
    )
    .window_size((900.0, 650.0))
    .run_with(DriveWipeApp::new)
}
