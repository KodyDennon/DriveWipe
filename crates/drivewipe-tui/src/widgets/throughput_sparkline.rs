use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Sparkline};

use drivewipe_core::types::format_throughput;

/// Draw a throughput sparkline showing recent throughput samples.
///
/// `history` contains the most recent throughput samples in bytes/sec.
/// `current_bps` is the current throughput value for the title.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    history: &[f64],
    current_bps: f64,
) {
    let title = if current_bps > 0.0 {
        format!(" Throughput: {} ", format_throughput(current_bps))
    } else {
        " Throughput ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if history.is_empty() {
        // Render empty block.
        frame.render_widget(block, area);
        return;
    }

    // Convert f64 throughput values to u64 for the Sparkline widget.
    // We scale relative to the max value in the history to get good resolution.
    let max_val = history
        .iter()
        .copied()
        .fold(0.0f64, f64::max)
        .max(1.0);

    // Scale to 0..100 range for display.
    let data: Vec<u64> = history
        .iter()
        .map(|&v| ((v / max_val) * 100.0).round() as u64)
        .collect();

    // Only show as many points as fit in the available width.
    let inner_width = block.inner(area).width as usize;
    let visible_data = if data.len() > inner_width {
        &data[data.len() - inner_width..]
    } else {
        &data
    };

    let sparkline = Sparkline::default()
        .block(block)
        .data(visible_data)
        .style(Style::default().fg(Color::Cyan))
        .max(100);

    frame.render_widget(sparkline, area);
}

/// A throughput history buffer that maintains the last N samples.
pub struct ThroughputHistory {
    samples: Vec<f64>,
    max_samples: usize,
}

impl ThroughputHistory {
    /// Create a new history buffer with the given capacity.
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Push a new throughput sample (bytes per second).
    pub fn push(&mut self, bps: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(bps);
    }

    /// Get the current samples as a slice.
    pub fn samples(&self) -> &[f64] {
        &self.samples
    }

    /// Get the most recent sample, or 0.0 if empty.
    pub fn current(&self) -> f64 {
        self.samples.last().copied().unwrap_or(0.0)
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

impl Default for ThroughputHistory {
    fn default() -> Self {
        Self::new(60)
    }
}
