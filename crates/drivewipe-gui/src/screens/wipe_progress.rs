use iced::widget::{column, container, progress_bar, text};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the wipe progress screen.
pub fn view<'a>(
    device: &'a str,
    method: &'a str,
    fraction: f32,
    throughput: &'a str,
    pass_info: &'a str,
    is_complete: bool,
) -> Element<'a, Message> {
    let title = if is_complete {
        text("Wipe Complete").size(theme::FONT_SIZE_XL)
    } else {
        text("Wipe in Progress...").size(theme::FONT_SIZE_XL)
    };

    let device_info = text(format!("Device: {}", device))
        .size(theme::FONT_SIZE_MD);

    let method_info = text(format!("Method: {}", method))
        .size(theme::FONT_SIZE_MD);

    let pass_text = text(pass_info).size(theme::FONT_SIZE_MD);

    let bar = progress_bar(0.0..=1.0, fraction)
        .height(20);

    let pct_text = text(format!("{:.1}%", fraction * 100.0))
        .size(theme::FONT_SIZE_LG);

    let throughput_text = text(format!("Throughput: {}", throughput))
        .size(theme::FONT_SIZE_MD);

    let content = column![
        title,
        device_info,
        method_info,
        pass_text,
        bar,
        pct_text,
        throughput_text,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
