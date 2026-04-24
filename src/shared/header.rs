//! Header panel — displays hostname, kernel, local IP, and public IP.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

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
            let os_lower = state.os_name.to_lowercase();
            let (os_emoji, os_style) = if os_lower.contains("windows") {
                ("🪟", Style::default().fg(Color::Cyan))
            } else if os_lower.contains("ubuntu") {
                ("🐧", Style::default().fg(Color::Red))
            } else if os_lower.contains("debian") {
                ("🌀", Style::default().fg(Color::Red))
            } else if os_lower.contains("arch") {
                ("🏔️", Style::default().fg(Color::Cyan))
            } else if os_lower.contains("mac") || os_lower.contains("darwin") {
                ("🍎", Style::default().fg(Color::White))
            } else if os_lower.contains("fedora") {
                ("🎩", Style::default().fg(Color::Blue))
            } else if os_lower.contains("centos") || os_lower.contains("rhel") {
                ("⚙️", Style::default().fg(Color::Red))
            } else if os_lower.contains("linux") {
                ("🐧", Style::default().fg(Color::Yellow))
            } else {
                ("💻", Style::default().fg(fg))
            };

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

/// Helper to format seconds into a human-readable uptime string.
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {:02}h {:02}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {:02}m {:02}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {:02}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}
