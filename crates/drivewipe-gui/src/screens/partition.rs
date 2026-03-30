use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the partition manager screen.
pub fn view<'a>(
    drives: &[drivewipe_core::types::DriveInfo],
    partition_info: &'a [String],
    loading: bool,
) -> Element<'a, Message> {
    let title = text("Partition Manager")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let mut drive_buttons = column![].spacing(theme::SPACING_SM);
    for (i, drive) in drives.iter().enumerate() {
        let pt = drive.partition_table.as_deref().unwrap_or("Unknown");
        let label = format!(
            "{} - {} ({}, {} partitions)",
            drive.path.display(),
            drive.model,
            pt,
            drive.partition_count,
        );
        drive_buttons = drive_buttons.push(
            button(
                text(label)
                    .size(theme::FONT_SIZE_MD)
                    .color(theme::TEXT_PRIMARY),
            )
            .on_press(Message::ViewPartitions(i))
            .width(Length::Fill),
        );
    }

    let mut info_col = column![].spacing(theme::SPACING_SM);
    if loading {
        info_col = info_col.push(
            text("Loading...")
                .size(theme::FONT_SIZE_MD)
                .color(theme::STATUS_INFO),
        );
    }
    for line in partition_info {
        info_col = info_col.push(
            text(line.as_str())
                .size(theme::FONT_SIZE_MD)
                .color(theme::TEXT_SECONDARY),
        );
    }

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD)).on_press(Message::NavigateToMenu);

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
