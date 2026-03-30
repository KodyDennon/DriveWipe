use iced::widget::{button, column, container, scrollable, text, text_input};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the confirmation screen.
pub fn view<'a>(
    device_paths: &[String],
    method_name: &'a str,
    confirm_text: &'a str,
) -> Element<'a, Message> {
    let drive_count = device_paths.len();

    let title = text("Confirm Operation")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let warning = text(format!(
        "WARNING: This will permanently destroy all data on {} drive{}!",
        drive_count,
        if drive_count == 1 { "" } else { "s" }
    ))
    .size(theme::FONT_SIZE_LG)
    .color(theme::DANGER);

    let method_info = text(format!("Method: {}", method_name))
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    // Show the actual device paths being wiped
    let mut device_col = column![].spacing(theme::SPACING_SM);
    device_col = device_col.push(
        text("Drives to wipe:")
            .size(theme::FONT_SIZE_MD)
            .color(theme::TEXT_PRIMARY),
    );
    for path in device_paths {
        device_col = device_col.push(
            text(format!("  {}", path))
                .size(theme::FONT_SIZE_MD)
                .color(theme::DANGER),
        );
    }

    let instruction = text("Type YES to confirm:")
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_PRIMARY);

    let input = text_input("Type YES", confirm_text)
        .on_input(Message::ConfirmInput)
        .size(theme::FONT_SIZE_LG);

    let is_confirmed = confirm_text.trim() == "YES";

    let mut confirm_btn = button(text("WIPE").size(theme::FONT_SIZE_LG));
    if is_confirmed {
        confirm_btn = confirm_btn.on_press(Message::StartWipe);
    }

    let cancel_btn =
        button(text("Cancel").size(theme::FONT_SIZE_MD)).on_press(Message::NavigateBack);

    let content = column![
        title,
        warning,
        method_info,
        scrollable(device_col).height(Length::FillPortion(2)),
        instruction,
        input,
        confirm_btn,
        cancel_btn,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::BG_LIGHT)),
            ..Default::default()
        })
        .into()
}
