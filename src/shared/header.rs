//! Header panel — displays hostname, kernel, local IP, and public IP.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;
use crate::shared::fmt::{format_bytes, format_uptime};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let border = colors.border;
    let accent = colors.accent;
    let warn_color = colors.gauge_mid;
    let green_color = colors.gauge_low;
    let purple_color = colors.table_header;

    let mut spans = Vec::with_capacity(16);
    let width = area.width;

    // Special handling for Settings tab to show status message
    if state.active_tab == crate::core::state::ActiveTab::Settings {
        if let Some(status) = state.settings.active_status() {
            spans.push(Span::styled(
                " ⚙  Settings ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            if width > 40 {
                spans.push(Span::styled("  │  ", Style::default().fg(border)));
                spans.push(Span::styled(
                    status,
                    Style::default()
                        .fg(colors.gauge_low)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            spans.push(Span::styled(
                " ⚙  Settings ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            if width > 40 {
                spans.push(Span::styled("  │  ", Style::default().fg(border)));
                spans.push(Span::styled(
                    format!(
                        "Field {}/{}",
                        state.settings.selected + 1,
                        state.settings.fields.len()
                    ),
                    Style::default().fg(fg),
                ));
            }
        }
        if width > 20 {
            spans.push(Span::styled("  │  ", Style::default().fg(border)));
        }
    } else {
        spans.push(Span::styled(
            " ◈ ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            &state.hostname,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));

        if state.is_wsl && width > 50 {
            spans.push(Span::styled(
                " [WSL]",
                Style::default()
                    .fg(warn_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if width > 60 {
            let (os_emoji, os_style) = os_branding(&state.os_name, fg);

            spans.push(Span::styled("  │  ", Style::default().fg(border)));
            spans.push(Span::styled(
                format!("{} ", os_emoji),
                os_style,
            ));
            spans.push(Span::styled(
                state.get_os_info(),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled("  │  ", Style::default().fg(border)));
    }

    // Common metrics shown in all headers - conditionally hide based on width
    if width > 80 {
        spans.push(Span::styled("Kernel: ", Style::default().fg(border)));
        spans.push(Span::styled(&state.kernel_version, Style::default().fg(fg)));
        spans.push(Span::styled("  │  ", Style::default().fg(border)));
    }

    if width > 40 {
        spans.push(Span::styled("LAN: ", Style::default().fg(border)));
        spans.push(Span::styled(
            &state.local_ip,
            Style::default().fg(green_color),
        ));
        spans.push(Span::styled("  │  ", Style::default().fg(border)));
    }

    if width > 100 {
        spans.push(Span::styled("WAN: ", Style::default().fg(border)));
        spans.push(Span::styled(
            &state.public_ip,
            Style::default().fg(if state.public_ip == "Fetching..." {
                warn_color
            } else {
                green_color
            }),
        ));
        spans.push(Span::styled("  │  ", Style::default().fg(border)));
    }

    if width > 70 {
        spans.push(Span::styled("NET: ", Style::default().fg(border)));
        spans.push(Span::styled(
            format!(
                "↓{}/s ↑{}/s",
                format_bytes(state.net_rx),
                format_bytes(state.net_tx)
            ),
            Style::default().fg(purple_color),
        ));
        spans.push(Span::styled("  │  ", Style::default().fg(border)));
    }

    spans.push(Span::styled("UP: ", Style::default().fg(border)));
    spans.push(Span::styled(
        format_uptime(state.uptime_secs),
        Style::default().fg(accent),
    ));

    let line = Line::from(spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " rmonitor ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}

/// Map OS name to an emoji and accent color for the header display.
///
/// Uses a lookup table — add new distros by appending to the array.
fn os_branding(os_name: &str, fallback_fg: Color) -> (&'static str, Style) {
    const OS_TABLE: &[(&[&str], &str, Color)] = &[
        (&["windows"],          "🪟", Color::Cyan),
        (&["ubuntu"],           "🐧", Color::Red),
        (&["debian"],           "🌀", Color::Red),
        (&["arch"],             "🏔️",  Color::Cyan),
        (&["mac", "darwin"],    "🍎", Color::White),
        (&["fedora"],           "🎩", Color::Blue),
        (&["centos", "rhel"],   "⚙️",  Color::Red),
        (&["linux"],            "🐧", Color::Yellow),
    ];

    let lower = os_name.to_lowercase();
    for &(patterns, emoji, color) in OS_TABLE {
        if patterns.iter().any(|p| lower.contains(p)) {
            return (emoji, Style::default().fg(color));
        }
    }
    ("💻", Style::default().fg(fallback_fg))
}
