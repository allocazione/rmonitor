//! Application controller — event loop, background task spawning, and
//! the core tick/render cycle targeting 60 FPS.
//!
//! Handles keyboard input for Dashboard, Docker, and Settings tabs.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent, MouseButton};
use std::sync::Arc;
use std::time::Duration;

use crate::core::config::AppConfig;
use crate::modules::system::metrics::SysInfoMetrics;
use crate::modules::network::provider::{fetch_public_ip, GeoIpCache};
use crate::modules::system::history::watch_user_history;
use crate::providers::MetricProvider;
use crate::core::state::{ActiveTab, AlertEntry, DockerAction, ProcessSort};
use crate::core::store::Store;
use crate::ui;

// ---------------------------------------------------------------------------
// Background task spawning
// ---------------------------------------------------------------------------

/// Spawn all background metric-gathering tasks.
pub fn spawn_metric_tasks(store: &Store, config: &AppConfig) -> Arc<SysInfoMetrics> {
    let metrics = Arc::new(SysInfoMetrics::new());
    let interval = Duration::from_millis(config.general.refresh_rate_ms);

    // Combined refresh task (CPU, Memory, Network, Processes)
    {
        let store = store.clone();
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            loop {
                metrics.refresh_all(&store).await;
                tokio::time::sleep(interval).await;
            }
        });
    }

    // Disk refresh task (less frequent — every 10s)
    {
        let store = store.clone();
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            loop {
                metrics.refresh_disk(&store).await;
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }

    metrics
}

/// Spawn the command history watcher.
pub fn spawn_history_watcher(store: &Store) {
    let store = store.clone();
    tokio::spawn(async move {
        watch_user_history(store).await;
    });
}

/// Spawn the public IP fetch (one-shot with retry).
pub fn spawn_public_ip_fetch(store: &Store, config: &AppConfig) {
    let store = store.clone();
    let config = config.clone();
    tokio::spawn(async move {
        for attempt in 0..3u32 {
            fetch_public_ip(&store, &config).await;
            let ip = store.read().await.public_ip.clone();
            if ip != "Fetching..." && ip != "Unavailable" && ip != "Error" {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(attempt))).await;
        }
    });
}

/// Spawn the connection watcher (platform-specific).
pub fn spawn_connection_watcher(store: &Store, config: &AppConfig) {
    let store = store.clone();
    let geo_cache = Arc::new(GeoIpCache::new(config));

    #[cfg(target_os = "linux")]
    {
        use crate::modules::security::unix::UnixConnectionProvider;
        use crate::providers::ConnectionProvider;
        use crate::core::state::AlertEntry;
        use chrono::Utc;

        // Check if running as root on Linux and notify if not
        let is_root = unsafe { libc::getuid() == 0 };
        if !is_root {
            let store_c = store.clone();
            tokio::spawn(async move {
                let now = Utc::now();
                let alert = AlertEntry {
                    message: "Running without root: Security logs disabled. Try: sudo env \"PATH=$PATH\" rmonitor".into(),
                    timestamp: now,
                    expires_at: now + chrono::Duration::seconds(15),
                };
                store_c.write().await.push_alert(alert);
            });
        }

        let provider = UnixConnectionProvider::new(config, geo_cache);
        tokio::spawn(async move {
            provider.watch_connections(&store).await;
        });
    }

    #[cfg(any(target_os = "openbsd", target_os = "freebsd", target_os = "macos"))]
    {
        use crate::modules::security::unix::UnixConnectionProvider;
        use crate::providers::ConnectionProvider;

        let provider = UnixConnectionProvider::new(config, geo_cache);
        tokio::spawn(async move {
            provider.watch_connections(&store).await;
        });
    }

    #[cfg(target_os = "windows")]
    {
        use crate::modules::security::windows::WindowsConnectionProvider;
        use crate::providers::ConnectionProvider;

        let provider = WindowsConnectionProvider::new(config, geo_cache);
        tokio::spawn(async move {
            provider.watch_connections(&store).await;
        });
    }

    #[cfg(not(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "macos", target_os = "windows")))]
    {
        let store_c = store.clone();
        tokio::spawn(async move {
            store_c
                .write()
                .await
                .permission_warnings
                .push("Connection monitoring not supported on this OS".into());
        });
    }
}

/// Spawn the Docker container monitoring task.
pub fn spawn_docker_watcher(store: &Store) {
    let store = store.clone();
    tokio::spawn(async move {
        crate::modules::docker::provider::watch_docker(store).await;
    });
}

// ---------------------------------------------------------------------------
// Tab cycling helper
// ---------------------------------------------------------------------------

fn next_tab(current: ActiveTab) -> ActiveTab {
    match current {
        ActiveTab::Dashboard => ActiveTab::Docker,
        ActiveTab::Docker => ActiveTab::Processes,
        ActiveTab::Processes => ActiveTab::Settings,
        ActiveTab::Settings => ActiveTab::Dashboard,
    }
}

// ---------------------------------------------------------------------------
// Main event loop
// ---------------------------------------------------------------------------

/// Run the TUI event loop at the configured FPS.
pub async fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    store: &Store,
    config: &mut AppConfig,
    metrics: Arc<SysInfoMetrics>,
) -> std::io::Result<()> {
    let fps = config.general.ui_fps.clamp(1, 120);
    let tick_rate = Duration::from_millis(1000 / fps as u64);

    let mut last_state = store.snapshot().await;

    loop {
        if let Some(fresh) = store.try_snapshot() {
            last_state = fresh;
        }

        last_state.prune_alerts();

        let state_ref = &last_state;
        let config_ref = &*config;
        terminal.draw(|frame| {
            ui::draw(frame, state_ref, config_ref);
        })?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match last_state.active_tab {
                        ActiveTab::Dashboard => {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Esc => {
                                    let mut st = store.write().await;
                                    if st.show_user_history {
                                        st.show_user_history = false;
                                        st.user_history_scroll = 0;
                                    } else {
                                        return Ok(());
                                    }
                                }
                                KeyCode::Tab => {
                                    let mut st = store.write().await;
                                    st.active_tab = next_tab(ActiveTab::Dashboard);
                                    st.show_user_history = false;
                                    st.user_history_scroll = 0;
                                }
                                KeyCode::Char('1') => {
                                    let mut st = store.write().await;
                                    st.active_tab = ActiveTab::Dashboard;
                                    st.show_user_history = false;
                                    st.user_history_scroll = 0;
                                }
                                KeyCode::Char('2') => {
                                    let mut st = store.write().await;
                                    st.active_tab = ActiveTab::Docker;
                                    st.show_user_history = false;
                                    st.user_history_scroll = 0;
                                }
                                KeyCode::Char('3') => {
                                    let mut st = store.write().await;
                                    st.active_tab = ActiveTab::Processes;
                                    st.show_user_history = false;
                                    st.user_history_scroll = 0;
                                }
                                KeyCode::Char('4') => {
                                    let mut st = store.write().await;
                                    st.active_tab = ActiveTab::Settings;
                                    st.show_user_history = false;
                                    st.user_history_scroll = 0;
                                }
                                KeyCode::Up => {
                                    let mut st = store.write().await;
                                    if st.show_user_history {
                                        st.user_history_scroll = st.user_history_scroll.saturating_sub(1);
                                    } else if st.user_selected > 0 {
                                        st.user_selected -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let mut st = store.write().await;
                                    if st.show_user_history {
                                        st.user_history_scroll = st.user_history_scroll.saturating_add(1);
                                    } else {
                                        let max = st.user_commands.len().saturating_sub(1);
                                        if st.user_selected < max {
                                            st.user_selected += 1;
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    let mut st = store.write().await;
                                    if !st.user_commands.is_empty() {
                                        st.show_user_history = !st.show_user_history;
                                        st.user_history_scroll = 0;
                                    }
                                }
                                _ => {}
                            }
                        }
                        ActiveTab::Docker => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    store.write().await.active_tab = ActiveTab::Dashboard;
                                }
                                KeyCode::Tab => {
                                    store.write().await.active_tab = next_tab(ActiveTab::Docker);
                                }
                                KeyCode::Char('1') => {
                                    store.write().await.active_tab = ActiveTab::Dashboard;
                                }
                                KeyCode::Char('3') => {
                                    store.write().await.active_tab = ActiveTab::Processes;
                                }
                                KeyCode::Char('4') => {
                                    store.write().await.active_tab = ActiveTab::Settings;
                                }
                                KeyCode::Up => {
                                    let mut st = store.write().await;
                                    if st.docker_selected > 0 {
                                        st.docker_selected -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let mut st = store.write().await;
                                    let max = st.containers.len().saturating_sub(1);
                                    if st.docker_selected < max {
                                        st.docker_selected += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    let mut st = store.write().await;
                                    if !st.containers.is_empty() {
                                        st.show_docker_details = !st.show_docker_details;
                                    }
                                }
                                KeyCode::Char('s') => {
                                    let mut st = store.write().await;
                                    if let Some(c) = st.containers.get(st.docker_selected) {
                                        st.docker_action_request = Some((DockerAction::Stop, c.id.clone()));
                                    }
                                }
                                KeyCode::Char('u') => {
                                    let mut st = store.write().await;
                                    if let Some(c) = st.containers.get(st.docker_selected) {
                                        st.docker_action_request = Some((DockerAction::Start, c.id.clone()));
                                    }
                                }
                                KeyCode::Char('r') => {
                                    let mut st = store.write().await;
                                    if let Some(c) = st.containers.get(st.docker_selected) {
                                        st.docker_action_request = Some((DockerAction::Restart, c.id.clone()));
                                    }
                                }
                                KeyCode::Char('k') => {
                                    let mut st = store.write().await;
                                    if let Some(c) = st.containers.get(st.docker_selected) {
                                        st.docker_action_request = Some((DockerAction::Kill, c.id.clone()));
                                    }
                                }
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    let mut st = store.write().await;
                                    if let Some((action, id)) = st.docker_action_request.take() {
                                        st.docker_action_confirmed = Some((action, id));
                                    }
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') => {
                                    let mut st = store.write().await;
                                    st.docker_action_request = None;
                                }
                                _ => {}
                            }
                        }
                        ActiveTab::Processes => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    store.write().await.active_tab = ActiveTab::Dashboard;
                                }
                                KeyCode::Tab => {
                                    store.write().await.active_tab = next_tab(ActiveTab::Processes);
                                }
                                KeyCode::Char('1') => {
                                    store.write().await.active_tab = ActiveTab::Dashboard;
                                }
                                KeyCode::Char('2') => {
                                    store.write().await.active_tab = ActiveTab::Docker;
                                }
                                KeyCode::Char('4') => {
                                    store.write().await.active_tab = ActiveTab::Settings;
                                }
                                KeyCode::Up => {
                                    let mut st = store.write().await;
                                    if st.processes_selected > 0 {
                                        st.processes_selected -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let mut st = store.write().await;
                                    let max = st.processes.len().saturating_sub(1);
                                    if st.processes_selected < max {
                                        st.processes_selected += 1;
                                    }
                                }
                                KeyCode::PageUp => {
                                    let mut st = store.write().await;
                                    st.processes_selected = st.processes_selected.saturating_sub(20);
                                }
                                KeyCode::PageDown => {
                                    let mut st = store.write().await;
                                    let max = st.processes.len().saturating_sub(1);
                                    st.processes_selected = (st.processes_selected + 20).min(max);
                                }
                                KeyCode::Char('f') => {
                                    let mut st = store.write().await;
                                    st.processes_frozen = !st.processes_frozen;
                                }
                                KeyCode::Char('p') => {
                                    let mut st = store.write().await;
                                    if st.processes_sort_by == ProcessSort::Pid {
                                        st.processes_sort_asc = !st.processes_sort_asc;
                                    } else {
                                        st.processes_sort_by = ProcessSort::Pid;
                                        st.processes_sort_asc = true;
                                    }
                                }
                                KeyCode::Char('n') => {
                                    let mut st = store.write().await;
                                    if st.processes_sort_by == ProcessSort::Name {
                                        st.processes_sort_asc = !st.processes_sort_asc;
                                    } else {
                                        st.processes_sort_by = ProcessSort::Name;
                                        st.processes_sort_asc = true;
                                    }
                                }
                                KeyCode::Char('c') => {
                                    let mut st = store.write().await;
                                    if st.processes_sort_by == ProcessSort::Cpu {
                                        st.processes_sort_asc = !st.processes_sort_asc;
                                    } else {
                                        st.processes_sort_by = ProcessSort::Cpu;
                                        st.processes_sort_asc = false;
                                    }
                                }
                                KeyCode::Char('m') => {
                                    let mut st = store.write().await;
                                    if st.processes_sort_by == ProcessSort::Memory {
                                        st.processes_sort_asc = !st.processes_sort_asc;
                                    } else {
                                        st.processes_sort_by = ProcessSort::Memory;
                                        st.processes_sort_asc = false;
                                    }
                                }
                                KeyCode::Char('k') => {
                                    let pid = {
                                        let st = store.read().await;
                                        st.processes.get(st.processes_selected).map(|p| p.pid)
                                    };
                                    if let Some(pid) = pid {
                                        metrics.kill_process(pid);
                                    }
                                }
                                _ => {}
                            }
                        }
                        ActiveTab::Settings => {
                            if last_state.settings.editing {
                                handle_settings_edit_mode(store, config, key.code).await;
                            } else {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        store.write().await.active_tab = ActiveTab::Dashboard;
                                    }
                                    KeyCode::Tab => {
                                        store.write().await.active_tab = next_tab(ActiveTab::Settings);
                                    }
                                    KeyCode::Char('1') => {
                                        store.write().await.active_tab = ActiveTab::Dashboard;
                                    }
                                    KeyCode::Char('2') => {
                                        store.write().await.active_tab = ActiveTab::Docker;
                                    }
                                    KeyCode::Char('3') => {
                                        store.write().await.active_tab = ActiveTab::Processes;
                                    }
                                    KeyCode::Up => {
                                        let mut st = store.write().await;
                                        if st.settings.selected > 0 {
                                            st.settings.selected -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        let mut st = store.write().await;
                                        let max = st.settings.fields.len().saturating_sub(1);
                                        if st.settings.selected < max {
                                            st.settings.selected += 1;
                                        }
                                    }
                                    KeyCode::Home => {
                                        store.write().await.settings.selected = 0;
                                    }
                                    KeyCode::End => {
                                        let mut st = store.write().await;
                                        st.settings.selected = st.settings.fields.len().saturating_sub(1);
                                    }
                                    KeyCode::PageUp => {
                                        let mut st = store.write().await;
                                        st.settings.selected = st.settings.selected.saturating_sub(10);
                                    }
                                    KeyCode::PageDown => {
                                        let mut st = store.write().await;
                                        let max = st.settings.fields.len().saturating_sub(1);
                                        st.settings.selected = (st.settings.selected + 10).min(max);
                                    }
                                    KeyCode::Enter => {
                                        let mut st = store.write().await;
                                        let idx = st.settings.selected;
                                        st.settings.edit_buffer = st.settings.fields[idx].value.clone();
                                        st.settings.editing = true;
                                    }
                                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        save_settings(store, config).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    let rect = terminal.get_frame().area();
                    handle_mouse_event(store, mouse, rect).await;
                }
                _ => {}
            }
        }
    }
}

/// Handle keyboard input while editing a settings field.
async fn handle_settings_edit_mode(store: &Store, config: &mut AppConfig, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            store.write().await.settings.editing = false;
        }
        KeyCode::Enter => {
            let mut st = store.write().await;
            let idx = st.settings.selected;
            st.settings.fields[idx].value = st.settings.edit_buffer.clone();
            st.settings.editing = false;
            st.settings.apply_to_config(config);
            st.settings.set_status("✓ Applied (Ctrl+S to save to disk)".into());
        }
        KeyCode::Backspace => {
            store.write().await.settings.edit_buffer.pop();
        }
        KeyCode::Char(c) => {
            store.write().await.settings.edit_buffer.push(c);
        }
        _ => {}
    }
}

/// Save the current settings to disk.
async fn save_settings(store: &Store, config: &mut AppConfig) {
    {
        let st = store.read().await;
        st.settings.apply_to_config(config);
    }

    match config.save() {
        Ok(()) => {
            let path = AppConfig::user_config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "config file".into());
            let mut st = store.write().await;
            st.settings.set_status(format!("✓ Saved to {}", path));

            let now = chrono::Utc::now();
            st.push_alert(AlertEntry {
                message: "Settings saved successfully".into(),
                timestamp: now,
                expires_at: now + chrono::Duration::seconds(3),
            });
        }
        Err(e) => {
            let mut st = store.write().await;
            st.settings.set_status(format!("✗ Save failed: {}", e));
        }
    }
}

async fn handle_mouse_event(store: &Store, mouse: MouseEvent, rect: ratatui::layout::Rect) {
    if mouse.kind == event::MouseEventKind::Down(MouseButton::Left) {
        let mut st = store.write().await;
        if st.active_tab == ActiveTab::Dashboard {
            // Dashboard layout:
            // Header (3)
            // CPU/Mem (calc_cpu_panel_height)
            // Security (left) / History (right) -> main_chunks[2]
            
            let cpu_height = {
                let cores = st.cpu_usages.len().max(1) as u16;
                let rows = cores.div_ceil(2) + 2;
                rows.clamp(5, 14)
            };
            
            let start_y = 3 + cpu_height;
            let end_y = rect.height.saturating_sub(1);
            
            // Check if click is in the bottom area (Security/History)
            if mouse.row >= start_y && mouse.row < end_y {
                // Check if click is in the right half (History)
                if mouse.column >= rect.width / 2 && !st.user_commands.is_empty() {
                    // Determine which row was clicked if possible
                    let row_clicked = (mouse.row - start_y).saturating_sub(2) as usize; // -2 for header/border
                    if row_clicked < st.user_commands.len() {
                        if st.user_selected == row_clicked && st.show_user_history {
                            st.show_user_history = false;
                        } else {
                            st.user_selected = row_clicked;
                            st.show_user_history = true;
                        }
                    } else if !st.user_commands.is_empty() {
                        // Fallback: toggle if clicked in the general area but past the rows
                        st.show_user_history = !st.show_user_history;
                    }
                }
            }
        }
    }
}
