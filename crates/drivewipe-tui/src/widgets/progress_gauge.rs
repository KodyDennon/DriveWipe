use ratatui::prelude::*;
use ratatui::widgets::Gauge;

use drivewipe_core::types::{format_bytes, format_throughput, WipeOutcome};

use crate::ui;

/// Configuration for rendering a per-drive progress gauge.
#[allow(dead_code)]
pub struct DriveProgressGauge<'a> {
    pub device: &'a str,
    pub method: &'a str,
    pub current_pass: u32,
    pub total_passes: u32,
    pub bytes_written: u64,
    pub total_bytes: u64,
    pub throughput_bps: f64,
    pub outcome: Option<WipeOutcome>,
    pub verifying: bool,
}

#[allow(dead_code)]
impl<'a> DriveProgressGauge<'a> {
    /// Compute the progress fraction (0.0..=1.0).
    pub fn fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.bytes_written as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
    }

    /// Compute the ETA in seconds based on current throughput.
    pub fn eta_secs(&self) -> Option<f64> {
        if self.throughput_bps <= 0.0 {
            return None;
        }
        let remaining = self.total_bytes.saturating_sub(self.bytes_written) as f64;
        Some(remaining / self.throughput_bps)
    }

    /// Choose the gauge color based on the wipe state.
    pub fn gauge_color(&self) -> Color {
        match self.outcome {
            Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings) => Color::Green,
            Some(WipeOutcome::Failed) => Color::Red,
            Some(WipeOutcome::Cancelled | WipeOutcome::Interrupted) => Color::Yellow,
            None if self.verifying => Color::Magenta,
            None => Color::Blue,
        }
    }

    /// Build the label string shown inside the gauge.
    pub fn label(&self) -> String {
        let phase = if self.verifying { "Verify" } else { "Write" };
        let written = format_bytes(self.bytes_written);
        let total = format_bytes(self.total_bytes);
        let throughput = format_throughput(self.throughput_bps);
        let eta = self
            .eta_secs()
            .map(|s| ui::format_eta(s))
            .unwrap_or_else(|| "--:--".to_string());

        format!(
            " {}: {}/{} {} ETA {} ",
            phase, written, total, throughput, eta,
        )
    }

    /// Render the gauge into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let fraction = self.fraction();
        let color = self.gauge_color();
        let label = self.label();

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio(fraction)
            .label(Span::styled(
                label,
                Style::default().fg(Color::White),
            ));

        frame.render_widget(gauge, area);
    }
}

/// Render a compact single-line progress bar for a drive wipe.
///
/// Format: `[DEVICE] [METHOD] Pass X/Y [======>   ] XX.X% [THROUGHPUT] ETA HH:MM:SS`
#[allow(dead_code)]
pub fn render_compact(
    frame: &mut Frame,
    area: Rect,
    device: &str,
    method: &str,
    current_pass: u32,
    total_passes: u32,
    fraction: f64,
    throughput_bps: f64,
    outcome: Option<WipeOutcome>,
) {
    let color = match outcome {
        Some(WipeOutcome::Success | WipeOutcome::SuccessWithWarnings) => Color::Green,
        Some(WipeOutcome::Failed) => Color::Red,
        Some(WipeOutcome::Cancelled | WipeOutcome::Interrupted) => Color::Yellow,
        None => Color::Blue,
    };

    let throughput = format_throughput(throughput_bps);
    let label = format!(
        " {device} | {method} | Pass {current_pass}/{total_passes} | {throughput} "
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(fraction.clamp(0.0, 1.0))
        .label(Span::styled(
            label,
            Style::default().fg(Color::White),
        ));

    frame.render_widget(gauge, area);
}
