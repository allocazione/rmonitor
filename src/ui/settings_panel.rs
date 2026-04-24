//! Settings panel — full-screen interactive config editor.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::AppState;

/// Render the settings editor panel.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border = colors.border;
    let bg = colors.header_bg;
    let fg = colors.header_fg;
    let accent = colors.accent;
    let highlight_bg = colors.highlight;
    let edit_color = colors.gauge_low;
    let section_color = colors.table_header;
    let warn_color = colors.gauge_mid;

    // Clear the area
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title + status
            Constraint::Min(5),    // fields list
            Constraint::Length(2), // help bar
        ])
        .split(area);

    // ── Title bar ───────────────────────────────────────────────────────
    let title_text = if let Some(status) = state.settings.active_status() {
        Line::from(vec![
            Span::styled(
                " ⚙ Settings ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(border)),
            Span::styled(
                status,
                Style::default().fg(edit_color).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " ⚙ Settings ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(border)),
            Span::styled(
                format!(
                    "Field {}/{}",
                    state.settings.selected + 1,
                    state.settings.fields.len()
                ),
                Style::default().fg(fg),
            ),
        ])
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " rmonitor Settings ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(title_text).block(title_block), chunks[0]);

    // ── Fields list ─────────────────────────────────────────────────────
    let fields_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg));

    let inner = fields_block.inner(chunks[1]);
    frame.render_widget(fields_block, chunks[1]);

    let visible_height = inner.height as usize;
    let total_fields = state.settings.fields.len();

    // Calculate scroll offset to keep selected field visible
    let scroll_offset = if state.settings.selected >= visible_height {
        state.settings.selected - visible_height + 1
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);
    let mut last_section = String::new();

    for idx in scroll_offset..total_fields.min(scroll_offset + visible_height) {
        let field = &state.settings.fields[idx];
        let is_selected = idx == state.settings.selected;

        // Section header
        if field.section != last_section {
            last_section = field.section.clone();
            // Only add section header if we have room
            if lines.len() < visible_height {
                lines.push(Line::from(vec![Span::styled(
                    format!("  ── {} ", field.section),
                    Style::default()
                        .fg(section_color)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )]));
            }
        }

        if lines.len() >= visible_height {
            break;
        }

        let label_width = 22;
        let padded_label = format!("  {:<width$}", field.label, width = label_width);

        let (value_display, value_style) = if is_selected && state.settings.editing {
            // Show edit buffer with cursor
            let buf = format!("{}▌", state.settings.edit_buffer);
            (
                buf,
                Style::default().fg(edit_color).add_modifier(Modifier::BOLD),
            )
        } else {
            let v = if field.value.is_empty() {
                "(empty)".to_string()
            } else {
                field.value.clone()
            };
            (v, Style::default().fg(fg))
        };

        let row_style = if is_selected {
            Style::default().bg(highlight_bg)
        } else {
            Style::default()
        };

        let indicator = if is_selected {
            if state.settings.editing {
                Span::styled(
                    " ✎ ",
                    Style::default().fg(edit_color).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    " ▶ ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                )
            }
        } else {
            Span::styled("   ", Style::default())
        };

        // Color preview swatch for color fields
        let preview = if field.key.starts_with("colors.") && !field.value.is_empty() {
            let color = config.parse_color(&field.value);
            Span::styled(" ██ ", Style::default().fg(color))
        } else {
            Span::raw("")
        };

        lines.push(
            Line::from(vec![
                indicator,
                Span::styled(
                    padded_label,
                    Style::default().fg(if is_selected { warn_color } else { border }),
                ),
                Span::styled(value_display, value_style),
                preview,
            ])
            .style(row_style),
        );
    }

    let fields_paragraph = Paragraph::new(lines);
    frame.render_widget(fields_paragraph, inner);

    // Scrollbar
    if total_fields > visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(total_fields).position(state.settings.selected);
        let scrollbar =
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(border));
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    // ── Help bar ────────────────────────────────────────────────────────
    let help_spans = if state.settings.editing {
        vec![
            Span::styled(
                " Type",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to edit  ", Style::default().fg(fg)),
            Span::styled(
                "Esc",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Cancel  ", Style::default().fg(fg)),
            Span::styled(
                "Enter",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Confirm  ", Style::default().fg(fg)),
        ]
    } else {
        vec![
            Span::styled(
                " ↑↓",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(fg)),
            Span::styled(
                "Enter",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Edit  ", Style::default().fg(fg)),
            Span::styled(
                "Ctrl+S",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Save  ", Style::default().fg(fg)),
            Span::styled(
                "Tab",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Dashboard  ", Style::default().fg(fg)),
            Span::styled(
                "q",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Quit", Style::default().fg(fg)),
        ]
    };

    let help_line = Line::from(help_spans);
    let help = Paragraph::new(vec![help_line, Line::from("")]).style(Style::default().bg(bg));
    frame.render_widget(help, chunks[2]);
}
