use iced::widget::{button, column, container, progress_bar, text, text_input};
use iced::{Element, Length};

use crate::Message;
use crate::theme;

/// Clone screen state passed as a single struct to stay within clippy's arg limit.
pub struct CloneViewState<'a> {
    pub drives: &'a [drivewipe_core::types::DriveInfo],
    pub source: Option<usize>,
    pub target: Option<usize>,
    pub mode: &'a str,
    pub running: bool,
    pub progress: f32,
    pub throughput: &'a str,
    pub complete: bool,
    pub confirm_text: &'a str,
}

/// View for the clone setup screen.
pub fn view<'a>(state: &CloneViewState<'a>) -> Element<'a, Message> {
    let drives = state.drives;
    let source = state.source;
    let target = state.target;
    let mode = state.mode;
    let running = state.running;
    let progress = state.progress;
    let throughput = state.throughput;
    let complete = state.complete;
    let confirm_text = state.confirm_text;

    let title = text("Drive Clone")
        .size(theme::FONT_SIZE_XL)
        .color(theme::TEXT_PRIMARY);

    let source_str = match source {
        Some(i) if i < drives.len() => {
            format!("Source: {} ({})", drives[i].path.display(), drives[i].model)
        }
        _ => "Source: Not selected".to_string(),
    };

    let target_str = match target {
        Some(i) if i < drives.len() => {
            format!("Target: {} ({})", drives[i].path.display(), drives[i].model)
        }
        _ => "Target: Not selected".to_string(),
    };

    let source_text = text(source_str)
        .size(theme::FONT_SIZE_MD)
        .color(if source.is_some() {
            theme::STATUS_HEALTHY
        } else {
            theme::TEXT_MUTED
        });
    let target_text = text(target_str)
        .size(theme::FONT_SIZE_MD)
        .color(if target.is_some() {
            theme::STATUS_INFO
        } else {
            theme::TEXT_MUTED
        });
    let mode_text = text(format!("Mode: {}", mode))
        .size(theme::FONT_SIZE_MD)
        .color(theme::TEXT_SECONDARY);

    let info = text("Select source and target drives from the list below.")
        .size(theme::FONT_SIZE_SM)
        .color(theme::TEXT_MUTED);

    let mut drive_col = column![].spacing(theme::SPACING_SM);
    for (i, drive) in drives.iter().enumerate() {
        let label = format!(
            "{} - {} ({})",
            drive.path.display(),
            drive.model,
            drive.capacity_display()
        );
        let btn = button(text(label).size(theme::FONT_SIZE_SM)).width(Length::Fill);
        let btn = if running {
            btn // Disable drive selection while clone is running
        } else {
            btn.on_press(Message::SelectCloneDrive(i))
        };
        drive_col = drive_col.push(btn);
    }

    let back_btn = button(text("Back").size(theme::FONT_SIZE_MD)).on_press(Message::NavigateToMenu);

    let mut content = column![title, source_text, target_text, mode_text, info, drive_col,]
        .spacing(theme::SPACING_LG)
        .padding(theme::SPACING_XL);

    // Show confirmation UI when both drives are selected and not running
    if source.is_some() && target.is_some() && !running && !complete {
        let clone_warning = text(
            "WARNING: Cloning will overwrite ALL data on the target drive! Type YES to confirm.",
        )
        .size(theme::FONT_SIZE_MD)
        .color(theme::DANGER);

        let clone_input = text_input("Type YES to confirm", confirm_text)
            .on_input(Message::CloneConfirmInput)
            .size(theme::FONT_SIZE_MD);

        let is_confirmed = confirm_text.trim() == "YES";
        let mut start_btn = button(
            text("Start Clone")
                .size(theme::FONT_SIZE_LG)
                .color(theme::TEXT_PRIMARY),
        )
        .width(Length::Fixed(200.0));
        if is_confirmed {
            start_btn = start_btn.on_press(Message::StartClone);
        }

        content = content.push(clone_warning);
        content = content.push(clone_input);
        content = content.push(start_btn);
    }

    // Show progress when running
    if running {
        let pbar = progress_bar(0.0..=1.0, progress);
        let pct_text = text(format!("{:.1}%", progress * 100.0))
            .size(theme::FONT_SIZE_MD)
            .color(theme::STATUS_INFO);
        content = content.push(pbar);
        content = content.push(pct_text);
        if !throughput.is_empty() {
            let tp_text = text(throughput.to_string())
                .size(theme::FONT_SIZE_SM)
                .color(theme::TEXT_SECONDARY);
            content = content.push(tp_text);
        }
        let cancel_btn = button(
            text("Cancel Clone")
                .size(theme::FONT_SIZE_MD)
                .color(theme::DANGER),
        )
        .on_press(Message::CancelClone);
        content = content.push(cancel_btn);
    }

    // Show completion status
    if complete {
        let done_text = text("Clone completed successfully!")
            .size(theme::FONT_SIZE_LG)
            .color(theme::STATUS_HEALTHY);
        content = content.push(done_text);
    }

    content = content.push(back_btn);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
