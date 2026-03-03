use iced::widget::{button, column, container, text, text_input};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the confirmation screen.
pub fn view<'a>(
    drive_count: usize,
    method_name: &'a str,
    confirm_text: &'a str,
) -> Element<'a, Message> {
    let title = text("Confirm Operation")
        .size(theme::FONT_SIZE_XL);

    let warning = text(format!(
        "WARNING: This will permanently destroy all data on {} drive{}!",
        drive_count,
        if drive_count == 1 { "" } else { "s" }
    ))
    .size(theme::FONT_SIZE_LG);

    let method_info = text(format!("Method: {}", method_name))
        .size(theme::FONT_SIZE_MD);

    let instruction = text("Type YES to confirm:")
        .size(theme::FONT_SIZE_MD);

    let input = text_input("Type YES", confirm_text)
        .on_input(Message::ConfirmInput)
        .size(theme::FONT_SIZE_LG as f32);

    let is_confirmed = confirm_text.trim() == "YES";

    let mut confirm_btn = button(text("WIPE").size(theme::FONT_SIZE_LG));
    if is_confirmed {
        confirm_btn = confirm_btn.on_press(Message::StartWipe);
    }

    let cancel_btn = button(text("Cancel").size(theme::FONT_SIZE_MD))
        .on_press(Message::NavigateBack);

    let content = column![
        title,
        warning,
        method_info,
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
        .into()
}
