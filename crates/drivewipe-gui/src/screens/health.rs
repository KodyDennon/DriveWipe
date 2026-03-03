use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the drive health screen.
pub fn view<'a>(
    drives: &[drivewipe_core::types::DriveInfo],
    health_info: &'a [String],
) -> Element<'a, Message> {
    let title = text("Drive Health")
        .size(theme::FONT_SIZE_XL);

    let mut drive_buttons = column![].spacing(theme::SPACING_SM);
    for (i, drive) in drives.iter().enumerate() {
        let health_str = match drive.smart_healthy {
            Some(true) => "OK",
            Some(false) => "FAIL",
            None => "N/A",
        };
        let label = format!(
            "{} - {} [{}]",
            drive.path.display(),
            drive.model,
            health_str,
        );
        drive_buttons = drive_buttons.push(
            button(text(label).size(theme::FONT_SIZE_MD))
                .on_press(Message::ViewDriveHealth(i))
                .width(Length::Fill),
        );
    }

    let mut info_col = column![].spacing(theme::SPACING_SM);
    for line in health_info {
        info_col = info_col.push(text(line.as_str()).size(theme::FONT_SIZE_MD));
    }

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD))
        .on_press(Message::NavigateToMenu);

    let content = column![
        title,
        scrollable(drive_buttons).height(Length::FillPortion(2)),
        scrollable(info_col).height(Length::FillPortion(3)),
        back_btn,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
