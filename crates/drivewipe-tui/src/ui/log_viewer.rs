use chrono::{DateTime, Utc};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Draw a scrollable log viewer within the given area.
///
/// `messages` is the full log buffer (oldest first).
/// `scroll_offset` is the number of lines scrolled up from the bottom (0 = latest).
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    messages: &[(DateTime<Utc>, String)],
    scroll_offset: usize,
) {
    let total = messages.len();
    let scroll_info = if scroll_offset > 0 {
        format!(" Log ({scroll_offset} up) ")
    } else {
        " Log ".to_string()
    };

    let block = Block::default()
        .title(scroll_info)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    if visible_height == 0 || total == 0 {
        return;
    }

    // Calculate the window of messages to display.
    // scroll_offset 0 means show the newest messages at the bottom.
    let end = total.saturating_sub(scroll_offset);
    let start = end.saturating_sub(visible_height);

    let lines: Vec<Line> = messages[start..end]
        .iter()
        .map(|(ts, msg)| {
            let time_str = ts.format("%H:%M:%S").to_string();

            // Color based on message content.
            let msg_style = if msg.starts_with("ERROR:") || msg.starts_with("FAIL") {
                Style::default().fg(Color::Red)
            } else if msg.starts_with("WARNING:") || msg.starts_with("WARN") {
                Style::default().fg(Color::Yellow)
            } else if msg.contains("completed") || msg.contains("PASSED") || msg.contains("DONE") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };

            Line::from(vec![
                Span::styled(
                    format!("[{time_str}] "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(msg.as_str(), msg_style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
