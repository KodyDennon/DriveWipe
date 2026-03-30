use iced::widget::{button, column, container, radio, scrollable, text};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the wipe method selection screen.
pub fn view<'a>(
    methods: &[(String, String, u32)], // (id, name, passes)
    selected_method: Option<usize>,
    status_message: Option<&'a str>,
) -> Element<'a, Message> {
    let title = text("Select Wipe Method")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let mut method_list = column![].spacing(theme::SPACING_SM);
    for (i, (id, name, passes)) in methods.iter().enumerate() {
        let label = format!(
            "{} ({} pass{})",
            name,
            passes,
            if *passes == 1 { "" } else { "es" }
        );
        let r = radio(label, i, selected_method, Message::SelectMethod);
        method_list = method_list.push(r);
        let _ = id; // suppress unused warning
    }

    let mut continue_btn = button(
        text("Continue")
            .size(theme::FONT_SIZE_MD)
            .color(theme::SECONDARY),
    );
    if selected_method.is_some() {
        continue_btn = continue_btn.on_press(Message::ProceedToConfirm);
    }

    let buttons_row = column![
        continue_btn,
        button(text("Back").size(theme::FONT_SIZE_MD)).on_press(Message::NavigateBack),
    ]
    .spacing(theme::SPACING_MD);

    let mut content = column![
        title,
        scrollable(method_list).height(Length::Fill),
        buttons_row
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    if let Some(msg) = status_message {
        content = content.push(text(msg).size(theme::FONT_SIZE_MD).color(theme::WARNING));
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
