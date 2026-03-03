use iced::widget::{button, column, container, radio, scrollable, text};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the wipe method selection screen.
pub fn view<'a>(
    methods: &[(String, String, u32)], // (id, name, passes)
    selected_method: Option<usize>,
) -> Element<'a, Message> {
    let title = text("Select Wipe Method")
        .size(theme::FONT_SIZE_XL);

    let mut method_list = column![].spacing(theme::SPACING_SM);
    for (i, (id, name, passes)) in methods.iter().enumerate() {
        let label = format!("{} ({} pass{})", name, passes, if *passes == 1 { "" } else { "es" });
        let r = radio(label, i, selected_method, Message::SelectMethod);
        method_list = method_list.push(r);
        let _ = id; // suppress unused warning
    }

    let buttons_row = column![
        button(text("Continue").size(theme::FONT_SIZE_MD))
            .on_press(Message::ProceedToConfirm),
        button(text("Back").size(theme::FONT_SIZE_MD))
            .on_press(Message::NavigateBack),
    ]
    .spacing(theme::SPACING_MD);

    let content =
        column![title, scrollable(method_list).height(Length::Fill), buttons_row]
            .spacing(theme::SPACING_LG)
            .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
