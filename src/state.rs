//! Application state — the central Model in the MVC pattern.
//!
//! `AppState` is the single source of truth, shared between background
//! data-gathering tasks and the UI render loop via `Store`.

use chrono::{DateTime, Utc};
use std::collections::VecDeque;

use crate::config::AppConfig;

/// Maximum number of memory history samples (one per second → 60s window).
pub const MEM_HISTORY_CAP: usize = 60;

/// Maximum number of alerts to keep in the queue.
pub const ALERT_QUEUE_CAP: usize = 10;

// ---------------------------------------------------------------------------
// Connection & alert types
// ---------------------------------------------------------------------------

/// A single active SSH / RDP / console connection.
#[derive(Debug, Clone)]
pub struct ConnectionEntry {
    /// Username that logged in
    pub user: String,
    /// Source IP address (or "local" for console logins)
    pub source_ip: String,
    /// Protocol identifier: "SSH", "RDP", "Console", etc.
    pub protocol: String,
    /// Timestamp when the session started
    pub login_time: DateTime<Utc>,
    /// GeoIP-resolved location, e.g. "US, New York"
    pub location: String,
    /// Unique session identifier for deduplication / removal on logoff
    pub session_id: String,
}

/// A transient alert notification (toast).
#[derive(Debug, Clone)]
pub struct AlertEntry {
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tab & Settings state
// ---------------------------------------------------------------------------

/// Which tab is currently active in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Docker,
    Settings,
}

/// A running Docker container's live stats.
#[derive(Debug, Clone)]
pub struct DockerContainer {
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub cpu_percent: f64,
    pub mem_usage: u64,
    pub mem_limit: u64,
    pub net_rx: u64,
    pub net_tx: u64,
}

/// A single editable field in the settings panel.
#[derive(Debug, Clone)]
pub struct SettingsField {
    /// Display label
    pub label: String,
    /// Section header (e.g. "General", "Colors")
    pub section: String,
    /// The config key path (e.g. "general.ui_fps")
    pub key: String,
    /// Current value being edited (as string)
    pub value: String,
}

/// State for the interactive settings editor.
#[derive(Debug, Clone)]
pub struct SettingsState {
    /// All editable fields
    pub fields: Vec<SettingsField>,
    /// Currently selected field index
    pub selected: usize,
    /// Whether we're in edit mode for the selected field
    pub editing: bool,
    /// The edit buffer (what the user is typing)
    pub edit_buffer: String,
    /// Status message shown at the top of the settings panel
    pub status_message: Option<(String, DateTime<Utc>)>,
}

impl SettingsState {
    /// Build settings fields from the current config.
    pub fn from_config(config: &AppConfig) -> Self {
        let fields = vec![
            // General
            SettingsField {
                label: "UI FPS".into(),
                section: "General".into(),
                key: "general.ui_fps".into(),
                value: config.general.ui_fps.to_string(),
            },
            SettingsField {
                label: "Refresh Rate (ms)".into(),
                section: "General".into(),
                key: "general.refresh_rate_ms".into(),
                value: config.general.refresh_rate_ms.to_string(),
            },
            SettingsField {
                label: "Alert Duration (s)".into(),
                section: "General".into(),
                key: "general.alert_duration_secs".into(),
                value: config.general.alert_duration_secs.to_string(),
            },
            // Network
            SettingsField {
                label: "Public IP URL".into(),
                section: "Network".into(),
                key: "network.public_ip_url".into(),
                value: config.network.public_ip_url.clone(),
            },
            SettingsField {
                label: "GeoIP URL Template".into(),
                section: "Network".into(),
                key: "network.geoip_url_template".into(),
                value: config.network.geoip_url_template.clone(),
            },
            SettingsField {
                label: "GeoIP Cache Size".into(),
                section: "Network".into(),
                key: "network.geoip_cache_size".into(),
                value: config.network.geoip_cache_size.to_string(),
            },
            SettingsField {
                label: "Request Timeout (s)".into(),
                section: "Network".into(),
                key: "network.request_timeout_secs".into(),
                value: config.network.request_timeout_secs.to_string(),
            },
            // Paths
            SettingsField {
                label: "Auth Log Path".into(),
                section: "Paths".into(),
                key: "paths.auth_log".into(),
                value: config.paths.auth_log.clone().unwrap_or_default(),
            },
            // Colors
            SettingsField {
                label: "Header BG".into(),
                section: "Colors".into(),
                key: "colors.header_bg".into(),
                value: config.colors.header_bg.clone(),
            },
            SettingsField {
                label: "Header FG".into(),
                section: "Colors".into(),
                key: "colors.header_fg".into(),
                value: config.colors.header_fg.clone(),
            },
            SettingsField {
                label: "Gauge Low".into(),
                section: "Colors".into(),
                key: "colors.gauge_low".into(),
                value: config.colors.gauge_low.clone(),
            },
            SettingsField {
                label: "Gauge Mid".into(),
                section: "Colors".into(),
                key: "colors.gauge_mid".into(),
                value: config.colors.gauge_mid.clone(),
            },
            SettingsField {
                label: "Gauge High".into(),
                section: "Colors".into(),
                key: "colors.gauge_high".into(),
                value: config.colors.gauge_high.clone(),
            },
            SettingsField {
                label: "Gauge Empty".into(),
                section: "Colors".into(),
                key: "colors.gauge_empty".into(),
                value: config.colors.gauge_empty.clone(),
            },
            SettingsField {
                label: "Sparkline".into(),
                section: "Colors".into(),
                key: "colors.sparkline".into(),
                value: config.colors.sparkline.clone(),
            },
            SettingsField {
                label: "Table Header".into(),
                section: "Colors".into(),
                key: "colors.table_header".into(),
                value: config.colors.table_header.clone(),
            },
            SettingsField {
                label: "Table Row A".into(),
                section: "Colors".into(),
                key: "colors.table_row_a".into(),
                value: config.colors.table_row_a.clone(),
            },
            SettingsField {
                label: "Table Row B".into(),
                section: "Colors".into(),
                key: "colors.table_row_b".into(),
                value: config.colors.table_row_b.clone(),
            },
            SettingsField {
                label: "Alert BG".into(),
                section: "Colors".into(),
                key: "colors.alert_bg".into(),
                value: config.colors.alert_bg.clone(),
            },
            SettingsField {
                label: "Alert FG".into(),
                section: "Colors".into(),
                key: "colors.alert_fg".into(),
                value: config.colors.alert_fg.clone(),
            },
            SettingsField {
                label: "Border".into(),
                section: "Colors".into(),
                key: "colors.border".into(),
                value: config.colors.border.clone(),
            },
        ];

        Self {
            fields,
            selected: 0,
            editing: false,
            edit_buffer: String::new(),
            status_message: None,
        }
    }

    /// Apply the current settings fields back into an AppConfig.
    pub fn apply_to_config(&self, config: &mut AppConfig) {
        for field in &self.fields {
            match field.key.as_str() {
                "general.ui_fps" => {
                    config.general.ui_fps = field.value.parse().unwrap_or(60);
                }
                "general.refresh_rate_ms" => {
                    config.general.refresh_rate_ms = field.value.parse().unwrap_or(1000);
                }
                "general.alert_duration_secs" => {
                    config.general.alert_duration_secs = field.value.parse().unwrap_or(5);
                }
                "network.public_ip_url" => {
                    config.network.public_ip_url = field.value.clone();
                }
                "network.geoip_url_template" => {
                    config.network.geoip_url_template = field.value.clone();
                }
                "network.geoip_cache_size" => {
                    config.network.geoip_cache_size = field.value.parse().unwrap_or(128);
                }
                "network.request_timeout_secs" => {
                    config.network.request_timeout_secs = field.value.parse().unwrap_or(3);
                }
                "paths.auth_log" => {
                    config.paths.auth_log = if field.value.is_empty() {
                        None
                    } else {
                        Some(field.value.clone())
                    };
                }
                key if key.starts_with("colors.") => {
                    let color_field = &key[7..];
                    match color_field {
                        "header_bg" => config.colors.header_bg = field.value.clone(),
                        "header_fg" => config.colors.header_fg = field.value.clone(),
                        "gauge_low" => config.colors.gauge_low = field.value.clone(),
                        "gauge_mid" => config.colors.gauge_mid = field.value.clone(),
                        "gauge_high" => config.colors.gauge_high = field.value.clone(),
                        "gauge_empty" => config.colors.gauge_empty = field.value.clone(),
                        "sparkline" => config.colors.sparkline = field.value.clone(),
                        "table_header" => config.colors.table_header = field.value.clone(),
                        "table_row_a" => config.colors.table_row_a = field.value.clone(),
                        "table_row_b" => config.colors.table_row_b = field.value.clone(),
                        "alert_bg" => config.colors.alert_bg = field.value.clone(),
                        "alert_fg" => config.colors.alert_fg = field.value.clone(),
                        "border" => config.colors.border = field.value.clone(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// Set a status message that auto-expires after 3 seconds.
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Utc::now() + chrono::Duration::seconds(3)));
    }

    /// Get the current status message if it hasn't expired.
    pub fn active_status(&self) -> Option<&str> {
        self.status_message.as_ref().and_then(|(msg, expires)| {
            if Utc::now() < *expires {
                Some(msg.as_str())
            } else {
                None
            }
        })
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Central application state shared across all threads.
#[derive(Debug, Clone)]
pub struct AppState {
    // ── System info (static, set once) ──────────────────────────────────
    pub hostname: String,
    pub kernel_version: String,
    pub is_wsl: bool,
    pub local_ip: String,

    // ── Network (fetched async) ─────────────────────────────────────────
    pub public_ip: String,

    // ── CPU (per logical core, 0.0–100.0) ───────────────────────────────
    pub cpu_usages: Vec<f64>,

    // ── Memory ──────────────────────────────────────────────────────────
    pub mem_total: u64,
    pub mem_used: u64,
    /// Ring buffer of `mem_used` samples for the sparkline (newest at back).
    pub mem_history: VecDeque<u64>,

    // ── Disk ────────────────────────────────────────────────────────────
    pub disk_total: u64,
    pub disk_used: u64,

    // ── Network Traffic ─────────────────────────────────────────────────
    pub net_rx: u64,
    pub net_tx: u64,
    pub net_total_rx: u64,
    pub net_total_tx: u64,

    // ── Security connections ────────────────────────────────────────────
    pub connections: Vec<ConnectionEntry>,

    // ── Alert toasts ────────────────────────────────────────────────────
    pub alerts: VecDeque<AlertEntry>,

    // ── Permission warnings ─────────────────────────────────────────────
    pub permission_warnings: Vec<String>,

    // ── Docker ──────────────────────────────────────────────────────────
    pub docker_available: bool,
    pub docker_error: Option<String>,
    pub containers: Vec<DockerContainer>,
    pub docker_selected: usize,

    // ── UI state ────────────────────────────────────────────────────────
    pub active_tab: ActiveTab,
    pub settings: SettingsState,
}

impl AppState {
    /// Create a new `AppState` pre-populated with static system info.
    pub fn new(config: &AppConfig) -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into());

        #[allow(unused_mut)]
        let mut kernel_version = sysinfo::System::kernel_version()
            .unwrap_or_else(|| "unknown".into());

        #[allow(unused_mut)]
        let mut is_wsl = false;
        #[cfg(target_os = "linux")]
        {
            if let Ok(version_str) = std::fs::read_to_string("/proc/version") {
                let lower = version_str.to_lowercase();
                if lower.contains("microsoft") || lower.contains("wsl") {
                    is_wsl = true;
                    kernel_version = format!("{} (WSL)", kernel_version);
                }
            }
        }

        let local_ip = local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "N/A".into());

        let settings = SettingsState::from_config(config);

        Self {
            hostname,
            kernel_version,
            is_wsl,
            local_ip,
            public_ip: "Fetching...".into(),
            cpu_usages: Vec::new(),
            mem_total: 0,
            mem_used: 0,
            mem_history: VecDeque::with_capacity(MEM_HISTORY_CAP),
            disk_total: 0,
            disk_used: 0,
            net_rx: 0,
            net_tx: 0,
            net_total_rx: 0,
            net_total_tx: 0,
            connections: Vec::new(),
            alerts: VecDeque::with_capacity(ALERT_QUEUE_CAP),
            permission_warnings: Vec::new(),
            docker_available: false,
            docker_error: None,
            containers: Vec::new(),
            docker_selected: 0,
            active_tab: ActiveTab::Dashboard,
            settings,
        }
    }

    /// Push a memory sample into the ring buffer, evicting the oldest if full.
    pub fn push_mem_sample(&mut self, used: u64) {
        if self.mem_history.len() >= MEM_HISTORY_CAP {
            self.mem_history.pop_front();
        }
        self.mem_history.push_back(used);
    }

    /// Push an alert, evicting the oldest if the queue is full.
    pub fn push_alert(&mut self, alert: AlertEntry) {
        if self.alerts.len() >= ALERT_QUEUE_CAP {
            self.alerts.pop_front();
        }
        self.alerts.push_back(alert);
    }

    /// Remove expired alerts.
    pub fn prune_alerts(&mut self) {
        let now = Utc::now();
        self.alerts.retain(|a| a.expires_at > now);
    }

    /// Add a connection (dedup by session_id).
    pub fn add_connection(&mut self, conn: ConnectionEntry) {
        if !self.connections.iter().any(|c| c.session_id == conn.session_id) {
            self.connections.push(conn);
        }
    }

    /// Remove a connection by session_id.
    pub fn remove_connection(&mut self, session_id: &str) {
        self.connections.retain(|c| c.session_id != session_id);
    }
}
