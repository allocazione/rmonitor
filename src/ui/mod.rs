//! Top-level UI dispatcher — lays out the dashboard and delegates
//! to sub-panel rendering functions. Supports tab switching between
//! Dashboard, Docker, and Settings views.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::core::config::AppConfig;
use crate::core::state::{ActiveTab, AppState};
use crate::modules::system::{cpu, memory, processes, history_panel};
use crate::modules::docker::panel as docker_panel;
use crate::modules::security::{panel as security_panel, alerts as alert_toast};
use crate::shared::{header, settings as settings_panel};

/// Minimum terminal dimensions required for rendering.
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 12;

/// Render the entire UI into the given frame.
pub fn draw(frame: &mut Frame, state: &AppState, config: &AppConfig) {
    let area = frame.area();

    // Guard: terminal too small
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    match state.active_tab {
        ActiveTab::Dashboard => draw_dashboard(frame, area, state, config),
        ActiveTab::Docker => docker_panel::render(frame, area, state, config),
        ActiveTab::Processes => processes::render(frame, area, state, config),
        ActiveTab::Settings => settings_panel::render(frame, area, state, config),
    }
}

/// Render the dashboard (main monitoring view).
fn draw_dashboard(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                            // header
            Constraint::Length(calc_cpu_panel_height(state)), // CPU + memory
            Constraint::Min(6),                               // security + history
            Constraint::Length(1),                            // status / help bar
        ])
        .split(area);

    header::render(frame, main_chunks[0], state, config);

    let metrics_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    cpu::render(frame, metrics_chunks[0], state, config);
    memory::render(frame, metrics_chunks[1], state, config);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[2]);

    security_panel::render(frame, bottom_chunks[0], state, config);
    history_panel::render(frame, bottom_chunks[1], state, config);

    render_status_bar(frame, main_chunks[3], state, config);

    alert_toast::render(frame, area, state, config);
}

/// Calculate the height needed for the CPU panel based on core count.
fn calc_cpu_panel_height(state: &AppState) -> u16 {
    let cores = state.cpu_usages.len().max(1) as u16;
    let rows = cores.div_ceil(2) + 2;
    rows.clamp(5, 14)
}

/// Render a minimal help/status bar at the bottom with tab indicators.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState, config: &AppConfig) {
    let colors = config.get_colors();
    let border_color = colors.border;
    let accent = colors.accent;
    let fg = colors.header_fg;

    let tab_style = |tab: ActiveTab| -> Style {
        if state.active_tab == tab {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(border_color)
        }
    };

    let tab_fg = |tab: ActiveTab| -> Style {
        if state.active_tab == tab {
            Style::default().fg(fg)
        } else {
            Style::default().fg(border_color)
        }
    };

    let spans = vec![
        Span::styled(" Dashboard (1) ", tab_style(ActiveTab::Dashboard)),
        Span::styled(" Docker (2) ", tab_style(ActiveTab::Docker)),
        Span::styled(" Processes (3) ", tab_style(ActiveTab::Processes)),
        Span::styled(" Settings (4) ", tab_style(ActiveTab::Settings)),
        Span::styled("  │  ", Style::default().fg(border_color)),
        Span::styled(" ↑↓ Nav ", Style::default().fg(fg)),
        Span::styled(" Enter History ", Style::default().fg(fg)),
        Span::styled("  │  ", Style::default().fg(border_color)),
        Span::styled(" Quit (q/Esc) ", tab_fg(ActiveTab::Dashboard)),
    ];

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

/// Render a fallback message when the terminal is too small.
fn render_too_small(frame: &mut Frame, area: Rect) {
    let msg = "Terminal too small!";
    frame.render_widget(Paragraph::new(msg), area);
}
