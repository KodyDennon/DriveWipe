use iced::widget::{button, column, container, progress_bar, text};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the wipe progress screen.
pub fn view<'a>(
    device: &'a str,
    method: &'a str,
    fraction: f32,
    throughput: &'a str,
    pass_info: &'a str,
    is_complete: bool,
    is_running: bool,
) -> Element<'a, Message> {
    let title = if is_complete {
        text("Wipe Complete")
            .size(theme::FONT_SIZE_XL)
            .color(theme::STATUS_HEALTHY)
    } else {
        text("Wipe in Progress...")
            .size(theme::FONT_SIZE_XL)
            .color(theme::WARNING)
    };

    let device_info = text(format!("Device: {}", device))
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_PRIMARY);

    let method_info = text(format!("Method: {}", method))
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    let pass_text = text(pass_info)
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    let bar = progress_bar(0.0..=1.0, fraction);

    let pct_text = text(format!("{:.1}%", fraction * 100.0))
        .size(theme::FONT_SIZE_LG)
        .color(theme::PRIMARY);

    let throughput_text = text(format!("Throughput: {}", throughput))
        .size(theme::FONT_SIZE_MD)
        .color(theme::STATUS_INFO);

    let mut col = column![
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

    // Show cancel button while running, back button when complete or cancelled
    if is_running && !is_complete {
        col = col.push(
            button(
                text("Cancel Wipe")
                    .size(theme::FONT_SIZE_MD)
                    .color(theme::DANGER),
            )
            .on_press(Message::CancelWipe),
        );
    } else {
        // Complete or cancelled — show back button
        col = col.push(
            button(text("Back to Menu").size(theme::FONT_SIZE_MD))
                .on_press(Message::NavigateToMenu),
        );
    }

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::BG_MEDIUM)),
            ..Default::default()
        })
        .into()
}
