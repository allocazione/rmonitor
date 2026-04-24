//! Memory panel — renders a Sparkline of 60-second memory usage history.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Sparkline};
use ratatui::Frame;

use crate::config::{parse_hex_color, AppConfig};
use crate::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let border = parse_hex_color(&config.colors.border);
    let bg = parse_hex_color(&config.colors.header_bg);
    let spark_color = parse_hex_color(&config.colors.sparkline);

    // Format title with current usage
    let title = if state.mem_total > 0 {
        let used_gb = state.mem_used as f64 / 1_073_741_824.0;
        let total_gb = state.mem_total as f64 / 1_073_741_824.0;
        let pct = (state.mem_used as f64 / state.mem_total as f64) * 100.0;
        format!(" Memory: {:.1} / {:.1} GB ({:.0}%) ", used_gb, total_gb, pct)
    } else {
        " Memory ".to_string()
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(spark_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg));

    // Convert VecDeque<u64> to Vec<u64> for the sparkline data
    let data: Vec<u64> = state.mem_history.iter().copied().collect();

    let sparkline = Sparkline::default()
        .block(block)
        .data(&data)
        .max(state.mem_total)
        .style(Style::default().fg(spark_color));

    frame.render_widget(sparkline, area);
}
