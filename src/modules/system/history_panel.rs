use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Row, Table, Clear, Paragraph};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let accent = colors.accent;
    let hdr_color = colors.table_header;
    let row_a = colors.table_row_a;
    let row_b = colors.table_row_b;

    let header = Row::new(vec![
        Cell::from(Span::styled("User", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Last Command", Style::default().fg(hdr_color).add_modifier(Modifier::BOLD))),
    ])
    .height(1);

    let mut rows: Vec<Row> = Vec::with_capacity(state.user_commands.len());

    if state.user_commands.is_empty() {
        rows.push(
            Row::new(vec![
                Cell::from(""),
                Cell::from(Span::styled(
                    "No command history detected",
                    Style::default().fg(border),
                )),
            ])
            .style(Style::default().bg(row_a)),
        );
    }

    for (i, info) in state.user_commands.iter().enumerate() {
        let bg_color = if i % 2 == 0 { row_a } else { row_b };
        let mut style = Style::default().fg(fg).bg(bg_color);
        
        if state.user_selected == i {
            style = style.add_modifier(Modifier::REVERSED);
        }

        rows.push(
            Row::new(vec![
                Cell::from(Span::styled(&info.username, Style::default())),
                Cell::from(Span::styled(&info.last_command, Style::default())),
            ])
            .style(style),
        );
    }

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(80),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " User Activity ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        );

    frame.render_widget(table, area);

    // Render Popup if show_user_history is true
    if state.show_user_history {
        if let Some(user_info) = state.user_commands.get(state.user_selected) {
            let popup_area = Rect {
                x: area.x + area.width / 10,
                y: area.y + area.height / 10,
                width: (area.width * 8) / 10,
                height: (area.height * 8) / 10,
            };
            frame.render_widget(Clear, popup_area);

            let history_text: Vec<String> = user_info.history.iter().cloned().collect();
            let content = history_text.join("\n");

            let p = Paragraph::new(content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(accent))
                        .title(Span::styled(
                            format!(" History: {} (↑/↓ to scroll, Esc to close) ", user_info.username),
                            Style::default().fg(accent).add_modifier(Modifier::BOLD),
                        )),
                )
                .style(Style::default().fg(fg).bg(bg))
                .scroll((state.user_history_scroll, 0));

            frame.render_widget(p, popup_area);
        }
    }
}
