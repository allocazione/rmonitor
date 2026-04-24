//! CPU panel — renders a grid of Gauge widgets, one per logical core.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Gauge};
use ratatui::Frame;

use crate::config::{parse_hex_color, AppConfig};
use crate::state::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let border = parse_hex_color(&config.colors.border);
    let bg = parse_hex_color(&config.colors.header_bg);

    let block = Block::default()
        .title(Span::styled(
            " CPU Cores ",
            Style::default()
                .fg(parse_hex_color("#7aa2f7"))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.cpu_usages.is_empty() {
        return;
    }

    let num_cores = state.cpu_usages.len();
    let cols: usize = if num_cores <= 8 { 2 } else { 4 };
    let rows = num_cores.div_ceil(cols);

    // Create row constraints
    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Length(1))
        .collect();

    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(inner);

    // Create column constraints
    let col_constraints: Vec<Constraint> = (0..cols)
        .map(|_| Constraint::Ratio(1, cols as u32))
        .collect();

    for (i, usage) in state.cpu_usages.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;

        if row >= row_areas.len() {
            break;
        }

        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(row_areas[row]);

        if col >= col_areas.len() {
            break;
        }

        let color = gauge_color(*usage, config);
        let label = format!("C{:02} {:5.1}%", i, usage);
        let ratio = (*usage / 100.0).clamp(0.0, 1.0);

        let gauge = Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(color)
                    .bg(parse_hex_color(&config.colors.gauge_empty)),
            )
            .label(Span::styled(
                label,
                Style::default()
                    .fg(parse_hex_color(&config.colors.header_fg))
                    .add_modifier(Modifier::BOLD),
            ))
            .ratio(ratio);

        frame.render_widget(gauge, col_areas[col]);
    }
}

/// Choose gauge color based on usage threshold.
fn gauge_color(usage: f64, config: &AppConfig) -> ratatui::style::Color {
    if usage > 80.0 {
        parse_hex_color(&config.colors.gauge_high)
    } else if usage > 50.0 {
        parse_hex_color(&config.colors.gauge_mid)
    } else {
        parse_hex_color(&config.colors.gauge_low)
    }
}
