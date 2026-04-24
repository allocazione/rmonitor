//! Alert toast — floating notification overlay for new login events.

use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::config::{parse_hex_color, AppConfig};
use crate::state::AppState;

/// Render the newest non-expired alert as a floating toast in the bottom-right corner.
pub fn render(frame: &mut Frame, full_area: Rect, state: &AppState, config: &AppConfig) {
    let now = Utc::now();

    // Find the newest non-expired alert
    let active_alert = state
        .alerts
        .iter()
        .rev()
        .find(|a| a.expires_at > now);

    let alert = match active_alert {
        Some(a) => a,
        None => return, // nothing to show
    };

    let alert_bg = parse_hex_color(&config.colors.alert_bg);
    let alert_fg = parse_hex_color(&config.colors.alert_fg);

    // Calculate toast dimensions and position (bottom-right)
    let toast_width = 45u16.min(full_area.width.saturating_sub(4));
    let toast_height = 3u16;
    let x = full_area.width.saturating_sub(toast_width + 2);
    let y = full_area.height.saturating_sub(toast_height + 2);

    let toast_area = Rect::new(x, y, toast_width, toast_height);

    // Clear the area behind the toast
    frame.render_widget(Clear, toast_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(alert_fg).bg(alert_bg))
        .style(Style::default().bg(alert_bg))
        .title(Span::styled(
            format!(" ⚡ Alert {} ", alert.timestamp.format("%H:%M:%S")),
            Style::default()
                .fg(alert_fg)
                .bg(alert_bg)
                .add_modifier(Modifier::BOLD),
        ));

    let text = Line::from(Span::styled(
        &alert.message,
        Style::default()
            .fg(alert_fg)
            .bg(alert_bg)
            .add_modifier(Modifier::BOLD),
    ));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, toast_area);
}
