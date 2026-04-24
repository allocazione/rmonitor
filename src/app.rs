//! Application controller — event loop, background task spawning, and
//! the core tick/render cycle targeting 60 FPS.
//!
//! Handles keyboard input for Dashboard, Docker, and Settings tabs.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::sync::Arc;
use std::time::Duration;

use crate::config::AppConfig;
use crate::providers::metrics::SysInfoMetrics;
use crate::providers::network::{self, GeoIpCache};
use crate::providers::MetricProvider;
use crate::state::{ActiveTab, AlertEntry};
use crate::store::Store;
use crate::ui;

// ---------------------------------------------------------------------------
// Background task spawning
// ---------------------------------------------------------------------------

/// Spawn all background metric-gathering tasks.
pub fn spawn_metric_tasks(store: &Store, config: &AppConfig) {
    let metrics = Arc::new(SysInfoMetrics::new());
    let interval = Duration::from_millis(config.general.refresh_rate_ms);

    // CPU refresh task
    {
        let store = store.clone();
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            loop {
                metrics.refresh_cpu(&store).await;
                tokio::time::sleep(interval).await;
            }
        });
    }

    // Memory refresh task
    {
        let store = store.clone();
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            loop {
                metrics.refresh_memory(&store).await;
                tokio::time::sleep(interval).await;
            }
        });
    }

    // Network refresh task
    {
        let store = store.clone();
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            loop {
                metrics.refresh_network(&store).await;
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
}

/// Spawn the public IP fetch (one-shot with retry).
pub fn spawn_public_ip_fetch(store: &Store, config: &AppConfig) {
    let store = store.clone();
    let config = config.clone();
    tokio::spawn(async move {
        for attempt in 0..3u32 {
            network::fetch_public_ip(&store, &config).await;
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

    #[cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "macos"))]
    {
        use crate::providers::unix_connection::UnixConnectionProvider;
        use crate::providers::ConnectionProvider;

        let provider = UnixConnectionProvider::new(config, geo_cache);
        tokio::spawn(async move {
            provider.watch_connections(&store).await;
        });
    }

    #[cfg(target_os = "windows")]
    {
        use crate::providers::windows_connection::WindowsConnectionProvider;
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
        crate::providers::docker::watch_docker(store).await;
    });
}

// ---------------------------------------------------------------------------
// Tab cycling helper
// ---------------------------------------------------------------------------

fn next_tab(current: ActiveTab) -> ActiveTab {
    match current {
        ActiveTab::Dashboard => ActiveTab::Docker,
        ActiveTab::Docker => ActiveTab::Settings,
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
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match last_state.active_tab {
                    ActiveTab::Dashboard => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Tab => {
                                store.write().await.active_tab = next_tab(ActiveTab::Dashboard);
                            }
                            KeyCode::Char('2') => {
                                store.write().await.active_tab = ActiveTab::Docker;
                            }
                            KeyCode::Char('3') => {
                                store.write().await.active_tab = ActiveTab::Settings;
                            }
                            _ => {}
                        }
                    }
                    ActiveTab::Docker => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Tab => {
                                store.write().await.active_tab = next_tab(ActiveTab::Docker);
                            }
                            KeyCode::Char('1') => {
                                store.write().await.active_tab = ActiveTab::Dashboard;
                            }
                            KeyCode::Char('3') => {
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
                            _ => {}
                        }
                    }
                    ActiveTab::Settings => {
                        if last_state.settings.editing {
                            handle_settings_edit_mode(store, config, key.code).await;
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Tab => {
                                    store.write().await.active_tab = next_tab(ActiveTab::Settings);
                                }
                                KeyCode::Char('1') => {
                                    store.write().await.active_tab = ActiveTab::Dashboard;
                                }
                                KeyCode::Char('2') => {
                                    store.write().await.active_tab = ActiveTab::Docker;
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
