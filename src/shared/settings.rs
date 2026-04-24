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
    let edit_color = colors.gauge_low; // Reusing from colors
    let section_color = colors.table_header; // Reusing
    let warn_color = colors.gauge_mid; // Reusing

    // Clear the area
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header (standardized)
            Constraint::Min(5),    // fields list
            Constraint::Length(1), // help bar (standardized)
        ])
        .split(area);

    // ── Title bar ───────────────────────────────────────────────────────
    let title_area = chunks[0];
    crate::shared::header::render(frame, title_area, state, config);

    // ── Fields list ─────────────────────────────────────────────────────
    let list_area = chunks[1];
    let fields_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " Options ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    let inner = fields_block.inner(list_area);
    frame.render_widget(fields_block, list_area);

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
                Style::default().fg(colors.gauge_mid).add_modifier(Modifier::BOLD),
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
        frame.render_stateful_widget(scrollbar, list_area, &mut scrollbar_state);
    }

    // ── Help bar ────────────────────────────────────────────────────────
    let help_area = chunks[2];
    let help_text = Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(accent)),
        Span::styled("Select ", Style::default().fg(fg)),
        Span::styled(" Enter ", Style::default().fg(accent)),
        Span::styled("Edit ", Style::default().fg(fg)),
        Span::styled(" 1-4 ", Style::default().fg(accent)),
        Span::styled("Tabs ", Style::default().fg(fg)),
        Span::styled(" Esc ", Style::default().fg(accent)),
        Span::styled("Back ", Style::default().fg(fg)),
        Span::styled(" q ", Style::default().fg(accent)),
        Span::styled("Quit ", Style::default().fg(fg)),
    ]);
    
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().bg(bg));
    frame.render_widget(help_paragraph, help_area);
}
