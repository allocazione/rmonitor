//! Processes panel — displays a table of running processes with live stats.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

/// Render the Processes monitoring panel.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let accent = colors.accent;
    let green = colors.gauge_low;
    let purple = colors.table_header;
    let red = colors.gauge_high;
    let highlight_bg = colors.highlight;
    let hdr_color = colors.table_header;
    let row_a = colors.table_row_a;
    let row_b = colors.table_row_b;

    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title bar
            Constraint::Min(5),    // process table
            Constraint::Length(1), // help bar
        ])
        .split(area);

    // ── Title bar ───────────────────────────────────────────────────────
    let status_text = if state.processes_frozen {
        Span::styled(" [FROZEN] ", Style::default().fg(red).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" [LIVE] ", Style::default().fg(green).add_modifier(Modifier::BOLD))
    };

    let title_line = Line::from(vec![
        Span::styled(" ⚙  Processes ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled("  │  ", Style::default().fg(border)),
        status_text,
        Span::styled("  │  ", Style::default().fg(border)),
        Span::styled(
            format!("Top {} by CPU usage", state.processes.len()),
            Style::default().fg(green),
        ),
    ]);

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " Process Manager ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(title_line).block(title_block), chunks[0]);

    // ── Process table ─────────────────────────────────────────────────
    let header = Row::new(vec![
        Cell::from(Span::styled("PID", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Name", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("CPU%", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Memory", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
    ])
    .height(1);

    let mut rows: Vec<Row> = Vec::new();

    if state.processes.is_empty() {
        rows.push(
            Row::new(vec![
                Cell::from(""),
                Cell::from(Span::styled("No processes found", Style::default().fg(border))),
                Cell::from(""), Cell::from(""),
            ])
            .style(Style::default().bg(row_a)),
        );
    }

    for (i, p) in state.processes.iter().enumerate() {
        let bg_color = if i % 2 == 0 { row_a } else { row_b };
        let is_selected = i == state.processes_selected;

        let name_style = if is_selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };

        let row_style = if is_selected {
            Style::default().bg(highlight_bg)
        } else {
            Style::default().bg(bg_color)
        };

        rows.push(
            Row::new(vec![
                Cell::from(Span::styled(format!("{}", p.pid), Style::default().fg(border))),
                Cell::from(Span::styled(&p.name, name_style)),
                Cell::from(Span::styled(format!("{:.1}%", p.cpu_usage), Style::default().fg(purple))),
                Cell::from(Span::styled(format_bytes(p.memory), Style::default().fg(fg))),
            ])
            .style(row_style),
        );
    }

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(50),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
    ];

    let mut table_state = ratatui::widgets::TableState::default();
    table_state.select(Some(state.processes_selected));

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(bg)),
        )
        .row_highlight_style(Style::default().bg(highlight_bg));

    // We use StatefulWidget to let Ratatui handle scrolling into view
    frame.render_stateful_widget(table, chunks[1], &mut table_state);

    // ── Help bar ────────────────────────────────────────────────────────
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Nav ", Style::default().fg(fg)),
        Span::styled(" f", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Freeze ", Style::default().fg(fg)),
        Span::styled(" k", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Kill ", Style::default().fg(fg)),
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
