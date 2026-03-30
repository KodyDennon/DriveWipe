use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// View for the drive selection screen.
pub fn view<'a>(
    drives: &[drivewipe_core::types::DriveInfo],
    selected: &[bool],
    status_message: Option<&'a str>,
) -> Element<'a, Message> {
    let title = text("Select Drives")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let subtitle = text("Choose one or more drives to wipe")
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    let drive_list = if drives.is_empty() {
        column![
            text("No drives detected. Check permissions or connect a drive.")
                .size(theme::FONT_SIZE_MD)
                .color(theme::TEXT_MUTED)
        ]
        .spacing(theme::SPACING_MD)
    } else {
        let mut col = column![].spacing(theme::SPACING_SM);
        for (i, drive) in drives.iter().enumerate() {
            let is_selected = selected.get(i).copied().unwrap_or(false);
            let label = format!(
                "{} {} - {} ({}) {}",
                if is_selected { "[X]" } else { "[ ]" },
                drive.path.display(),
                drive.model,
                drive.capacity_display(),
                if drive.is_boot_drive { "(BOOT)" } else { "" },
            );

            let btn = button(text(label).size(theme::FONT_SIZE_MD))
                .on_press(Message::ToggleDrive(i))
                .width(Length::Fill);

            col = col.push(btn);
        }
        col
    };

    let has_selection = selected.iter().any(|s| *s);

    let mut continue_btn = button(text("Continue").size(theme::FONT_SIZE_MD));
    if has_selection {
        continue_btn = continue_btn.on_press(Message::ProceedToMethodSelect);
    }

    let buttons_row = row![
        button(text("Refresh").size(theme::FONT_SIZE_MD)).on_press(Message::RefreshDrives),
        continue_btn,
    ]
    .spacing(theme::SPACING_MD);

    let mut content = column![
        title,
        subtitle,
        scrollable(drive_list).height(Length::Fill),
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
