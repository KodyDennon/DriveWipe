use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the forensic analysis screen.
pub fn view<'a>(
    drives: &[drivewipe_core::types::DriveInfo],
    forensic_results: &'a [String],
) -> Element<'a, Message> {
    let title = text("Forensic Analysis")
        .size(theme::FONT_SIZE_XL);

    let mut drive_buttons = column![].spacing(theme::SPACING_SM);
    for (i, drive) in drives.iter().enumerate() {
        let label = format!(
            "{} - {} ({})",
            drive.path.display(),
            drive.model,
            drive.capacity_display(),
        );
        drive_buttons = drive_buttons.push(
            button(text(label).size(theme::FONT_SIZE_MD))
                .on_press(Message::RunForensicScan(i))
                .width(Length::Fill),
        );
    }

    let mut results_col = column![].spacing(theme::SPACING_SM);
    if forensic_results.is_empty() {
        results_col = results_col.push(
            text("Select a drive to start forensic analysis.").size(theme::FONT_SIZE_MD),
        );
    } else {
        for line in forensic_results {
            results_col = results_col.push(text(line.as_str()).size(theme::FONT_SIZE_MD));
        }
    }

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD))
        .on_press(Message::NavigateToMenu);

    let content = column![
        title,
        scrollable(drive_buttons).height(Length::FillPortion(2)),
        scrollable(results_col).height(Length::FillPortion(3)),
        back_btn,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
