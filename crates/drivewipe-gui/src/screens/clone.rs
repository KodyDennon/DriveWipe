use iced::widget::{button, column, container, text};
use iced::{Element, Length};

use crate::theme;
use crate::Message;

/// View for the clone setup screen.
pub fn view<'a>(
    drives: &[drivewipe_core::types::DriveInfo],
    source: Option<usize>,
    target: Option<usize>,
    mode: &'a str,
) -> Element<'a, Message> {
    let title = text("Drive Clone")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let source_str = match source {
        Some(i) if i < drives.len() => format!("Source: {} ({})", drives[i].path.display(), drives[i].model),
        _ => "Source: Not selected".to_string(),
    };

    let target_str = match target {
        Some(i) if i < drives.len() => format!("Target: {} ({})", drives[i].path.display(), drives[i].model),
        _ => "Target: Not selected".to_string(),
    };

    let source_text = text(source_str)
        .size(theme::FONT_SIZE_MD)
        .color(if source.is_some() { theme::STATUS_HEALTHY } else { theme::TEXT_MUTED });
    let target_text = text(target_str)
        .size(theme::FONT_SIZE_MD)
        .color(if target.is_some() { theme::STATUS_INFO } else { theme::TEXT_MUTED });
    let mode_text = text(format!("Mode: {}", mode))
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    let info = text("Select source and target drives from the list below.")
        .size(theme::FONT_SIZE_SM)
        .color(theme::TEXT_MUTED);

    let mut drive_col = column![].spacing(theme::SPACING_SM);
    for (i, drive) in drives.iter().enumerate() {
        let label = format!("{} - {} ({})", drive.path.display(), drive.model, drive.capacity_display());
        drive_col = drive_col.push(
            button(text(label).size(theme::FONT_SIZE_SM))
                .on_press(Message::SelectCloneDrive(i))
                .width(Length::Fill),
        );
    }

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD))
        .on_press(Message::NavigateToMenu);

    let content = column![
        title, source_text, target_text, mode_text, info, drive_col, back_btn,
    ]
    .spacing(theme::SPACING_LG)
    .padding(theme::SPACING_XL);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
