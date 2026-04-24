//! rmonitor — High-performance cross-platform System & SSH/RDP Monitor TUI.
//!
//! Entry point: installs panic hooks for terminal cleanup, loads config,
//! initializes shared state, spawns background tasks, and runs the TUI.

#![deny(warnings)]

mod core;
mod modules;
mod providers;
mod shared;
mod ui;

use core::config::AppConfig;
use core::state::AppState;
use core::store::Store;
use core::app;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // ── 1. Install panic hook BEFORE entering alternate screen ──────────
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        original_hook(panic_info);
    }));

    // ── 2. Load configuration ──────────────────────────────────────────
    let mut config = AppConfig::load();

    // ── 3. Initialize shared state ─────────────────────────────────────
    let state = AppState::new(&config);
    let store = Store::new(state);

    // ── 4. Spawn background data-gathering tasks ───────────────────────
    let metrics = app::spawn_metric_tasks(&store, &config);
    app::spawn_connection_watcher(&store, &config);
    app::spawn_public_ip_fetch(&store, &config);
    app::spawn_docker_watcher(&store);
    app::spawn_history_watcher(&store);

    // ── 5. Enter the TUI ───────────────────────────────────────────────
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);

    let result = app::run_event_loop(&mut terminal, &store, &mut config, metrics).await;

    // ── 6. Restore terminal (graceful exit) ────────────────────────────
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();

    // Force clear terminal screen and move cursor to (1,1)
    use crossterm::{execute, terminal::{Clear, ClearType}, cursor::MoveTo};
    let _ = execute!(std::io::stdout(), Clear(ClearType::All), MoveTo(0, 0));

    result
}
