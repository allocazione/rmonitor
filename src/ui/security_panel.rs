//! Security panel — active connections table with alternating row colors.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::config::{parse_hex_color, AppConfig};
use crate::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let border = parse_hex_color(&config.colors.border);
    let bg = parse_hex_color(&config.colors.header_bg);
    let hdr_color = parse_hex_color(&config.colors.table_header);
    let row_a = parse_hex_color(&config.colors.table_row_a);
    let row_b = parse_hex_color(&config.colors.table_row_b);
    let fg = parse_hex_color(&config.colors.header_fg);

    let header = Row::new(vec![
        Cell::from(Span::styled("User", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Source IP", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Protocol", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Login Time", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Location", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
    ])
    .height(1);

    let mut rows: Vec<Row> = Vec::new();

    // Permission warnings first
    for warn in &state.permission_warnings {
        rows.push(
            Row::new(vec![
                Cell::from(Span::styled("⚠", Style::default().fg(parse_hex_color("#e0af68")))),
                Cell::from(Span::styled(
                    warn.as_str(),
                    Style::default().fg(parse_hex_color("#e0af68")),
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
                        .fg(parse_hex_color("#bb9af7"))
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(bg)),
        );

    frame.render_widget(table, area);
}
