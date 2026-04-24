//! Docker panel — displays a table of running containers with live stats.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

/// Render the Docker monitoring panel.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let accent = colors.accent;
    let green = colors.gauge_low;
    let yellow = colors.gauge_mid;
    let red = colors.gauge_high;
    let purple = colors.table_header;
    let highlight_bg = colors.highlight;
    let hdr_color = colors.table_header;
    let row_a = colors.table_row_a;
    let row_b = colors.table_row_b;

    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title bar
            Constraint::Min(5),    // container table
            Constraint::Length(1), // help bar
        ])
        .split(area);

    // ── Title bar ───────────────────────────────────────────────────────
    let container_count = state.containers.iter().filter(|c| c.state == "running").count();
    let total_count = state.containers.len();

    let title_line = Line::from(vec![
        Span::styled(" 🐳 Docker ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled(
            format!("{} running / {} total", container_count, total_count),
            Style::default().fg(if state.docker_available { green } else { red }),
        ),
        if let Some(ref err) = state.docker_error {
            Span::styled(format!("  │  ⚠ {}", err), Style::default().fg(yellow))
        } else {
            Span::raw("")
        },
    ]);

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " Docker Containers ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(title_line).block(title_block), chunks[0]);

    // ── Container table ─────────────────────────────────────────────────
    if !state.docker_available {
        let msg = state.docker_error.as_deref().unwrap_or("Docker daemon not detected");
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(bg))
            .title(Span::styled(" Status ", Style::default().fg(yellow)));

        let text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  ⚠  {}", msg),
                Style::default().fg(yellow).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Make sure Docker Desktop or the Docker daemon is running.",
                Style::default().fg(fg),
            )),
            Line::from(Span::styled(
                "  The monitor will automatically detect it when available.",
                Style::default().fg(border),
            )),
        ])
        .block(block);

        frame.render_widget(text, chunks[1]);
    } else {
        let header = Row::new(vec![
            Cell::from(Span::styled("Name", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled("Image", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled("Status", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled("CPU%", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled("Memory", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled("Net I/O", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        ])
        .height(1);

        let mut rows: Vec<Row> = Vec::new();

        if state.containers.is_empty() {
            rows.push(
                Row::new(vec![
                    Cell::from(""),
                    Cell::from(Span::styled("No containers found", Style::default().fg(border))),
                    Cell::from(""), Cell::from(""), Cell::from(""), Cell::from(""),
                ])
                .style(Style::default().bg(row_a)),
            );
        }

        for (i, c) in state.containers.iter().enumerate() {
            let bg_color = if i % 2 == 0 { row_a } else { row_b };
            let is_selected = i == state.docker_selected;

            let state_color = match c.state.as_str() {
                "running" => green,
                "paused" => yellow,
                "exited" | "dead" => red,
                _ => border,
            };

            let name_style = if is_selected {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };

            let cpu_str = if c.state == "running" {
                format!("{:.1}%", c.cpu_percent)
            } else {
                "—".into()
            };

            let mem_str = if c.state == "running" && c.mem_limit > 0 {
                format!(
                    "{} / {}",
                    format_bytes(c.mem_usage),
                    format_bytes(c.mem_limit)
                )
            } else if c.state == "running" {
                format_bytes(c.mem_usage)
            } else {
                "—".into()
            };

            let net_str = if c.state == "running" {
                format!("↓{} ↑{}", format_bytes(c.net_rx), format_bytes(c.net_tx))
            } else {
                "—".into()
            };

            let row_style = if is_selected {
                Style::default().bg(highlight_bg)
            } else {
                Style::default().bg(bg_color)
            };

            rows.push(
                Row::new(vec![
                    Cell::from(Span::styled(&c.name, name_style)),
                    Cell::from(Span::styled(&c.image, Style::default().fg(fg))),
                    Cell::from(Span::styled(&c.status, Style::default().fg(state_color))),
                    Cell::from(Span::styled(cpu_str, Style::default().fg(purple))),
                    Cell::from(Span::styled(mem_str, Style::default().fg(fg))),
                    Cell::from(Span::styled(net_str, Style::default().fg(fg))),
                ])
                .style(row_style),
            );
        }

        let widths = [
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(18),
            Constraint::Percentage(10),
            Constraint::Percentage(17),
            Constraint::Percentage(15),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border))
                    .style(Style::default().bg(bg)),
            );

        frame.render_widget(table, chunks[1]);
    }

    // ── Help bar ────────────────────────────────────────────────────────
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Nav ", Style::default().fg(fg)),
        Span::styled(" 1-4", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Tabs ", Style::default().fg(fg)),
        Span::styled(" q", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Quit", Style::default().fg(fg)),
    ]))
    .style(Style::default().bg(bg));

    frame.render_widget(help, chunks[2]);
}

/// Format bytes to human-readable string.
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
