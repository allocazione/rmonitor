//! Security panel — active connections table with alternating row colors.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let warn_color = colors.gauge_mid;
    let purple = colors.table_header;
    let hdr_color = colors.table_header;
    let row_a = colors.table_row_a;
    let row_b = colors.table_row_b;

    let header = Row::new(vec![
        Cell::from(Span::styled("User", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Source IP", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Protocol", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Login Time", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Location", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
    ])
    .height(1);

    let mut rows: Vec<Row> = Vec::with_capacity(state.permission_warnings.len() + state.connections.len() + 1);

    // Permission warnings first
    for warn in &state.permission_warnings {
        rows.push(
            Row::new(vec![
                Cell::from(Span::styled("⚠", Style::default().fg(warn_color))),
                Cell::from(Span::styled(
                    warn.as_str(),
                    Style::default().fg(warn_color),
                )),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().bg(row_a)),
        );
    }

    if state.connections.is_empty() && state.permission_warnings.is_empty() {
        rows.push(
            Row::new(vec![
                Cell::from(""),
                Cell::from(Span::styled(
                    "No active connections detected",
                    Style::default().fg(border),
                )),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().bg(row_a)),
        );
    }

    for (i, conn) in state.connections.iter().enumerate() {
        let bg_color = if i % 2 == 0 { row_a } else { row_b };
        // Optimization: use a reusable buffer or simpler formatting if possible, 
        // but format! is generally okay for this volume.
        let time_str = conn.login_time.format("%H:%M:%S").to_string();

        rows.push(
            Row::new(vec![
                Cell::from(Span::styled(&conn.user, Style::default().fg(fg))),
                Cell::from(Span::styled(&conn.source_ip, Style::default().fg(fg))),
                Cell::from(Span::styled(&conn.protocol, Style::default().fg(fg))),
                Cell::from(Span::styled(time_str, Style::default().fg(fg))),
                Cell::from(Span::styled(&conn.location, Style::default().fg(fg))),
            ])
            .style(Style::default().bg(bg_color)),
        );
    }

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " Active Connections ",
                    Style::default()
                        .fg(purple)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(bg)),
        );

    frame.render_widget(table, area);
}
