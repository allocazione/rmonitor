//! Memory panel — renders a Sparkline of 60-second memory usage history.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Sparkline};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let spark_color = colors.sparkline;

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

    // Convert VecDeque<u64> to &[u64] for the sparkline data efficiently
    let (slice1, slice2) = state.mem_history.as_slices();
    
    // Sparkline widget in ratatui 0.26+ supports data from slices. 
    // If it's a contiguous slice, we can pass it directly.
    // Otherwise, we collect (minor allocation).
    let data_vec;
    let data: &[u64] = if slice2.is_empty() {
        slice1
    } else {
        data_vec = state.mem_history.iter().copied().collect::<Vec<_>>();
        &data_vec
    };

    let sparkline = Sparkline::default()
        .block(block)
        .data(data)
        .max(state.mem_total)
        .style(Style::default().fg(spark_color));

    frame.render_widget(sparkline, area);
}
