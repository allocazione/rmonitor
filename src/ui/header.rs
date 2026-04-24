//! Header panel — displays hostname, kernel, local IP, and public IP.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::config::{parse_hex_color, AppConfig};
use crate::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let bg = parse_hex_color(&config.colors.header_bg);
    let fg = parse_hex_color(&config.colors.header_fg);
    let border = parse_hex_color(&config.colors.border);

    let mut spans = vec![
        Span::styled(
            " ◈ ",
            Style::default()
                .fg(parse_hex_color("#7aa2f7"))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &state.hostname,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ),
    ];

    if state.is_wsl {
        spans.push(Span::styled(
            " [WSL]",
            Style::default()
                .fg(parse_hex_color("#e0af68"))
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.extend([
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled("Kernel: ", Style::default().fg(border)),
        Span::styled(&state.kernel_version, Style::default().fg(fg)),
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled("LAN: ", Style::default().fg(border)),
        Span::styled(
            &state.local_ip,
            Style::default().fg(parse_hex_color("#9ece6a")),
        ),
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled("WAN: ", Style::default().fg(border)),
        Span::styled(
            &state.public_ip,
            Style::default().fg(if state.public_ip == "Fetching..." {
                parse_hex_color("#e0af68")
            } else {
                parse_hex_color("#9ece6a")
            }),
        ),
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled("NET: ", Style::default().fg(border)),
        Span::styled(
            format!(
                "↓{}/s ↑{}/s",
                format_bytes(state.net_rx),
                format_bytes(state.net_tx)
            ),
            Style::default().fg(parse_hex_color("#bb9af7")),
        ),
    ]);

    let line = Line::from(spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " rmonitor ",
            Style::default()
                .fg(parse_hex_color("#7aa2f7"))
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}

/// Helper to format bytes into human readable format (KB, MB, GB, etc.)
fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.1}GB", b / GB)
    } else if b >= MB {
        format!("{:.1}MB", b / MB)
    } else if b >= KB {
        format!("{:.1}KB", b / KB)
    } else {
        format!("{}B", bytes)
    }
}
