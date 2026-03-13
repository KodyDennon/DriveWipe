use iced::widget::{button, checkbox, column, container, text};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the settings screen.
pub fn view<'a>(
    auto_report: bool,
    notifications: bool,
    sleep_prevention: bool,
    auto_health: bool,
) -> Element<'a, Message> {
    let title = text("Settings")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let auto_report_cb = checkbox(auto_report)
        .label("Auto JSON Reports")
        .on_toggle(|v| Message::ToggleSetting("auto_report".into(), v));

    let notifications_cb = checkbox(notifications)
        .label("Desktop Notifications")
        .on_toggle(|v| Message::ToggleSetting("notifications".into(), v));

    let sleep_cb = checkbox(sleep_prevention)
        .label("Sleep Prevention")
        .on_toggle(|v| Message::ToggleSetting("sleep_prevention".into(), v));

    let health_cb = checkbox(auto_health)
        .label("Auto Health Check Before Wipe")
        .on_toggle(|v| Message::ToggleSetting("auto_health".into(), v));

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD)).on_press(Message::NavigateToMenu);

    let content = column![
        title,
        auto_report_cb,
        notifications_cb,
        sleep_cb,
        health_cb,
        back_btn,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
