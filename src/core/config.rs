//! Configuration loading, parsing, and saving for rmonitor.
//!
//! Loads settings from `~/.config/rmonitor/config.toml` (Linux)
//! or `%APPDATA%\rmonitor\config.toml` (Windows), falling back
//! to compiled-in defaults from `config.default.toml`.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

// Embedded default config
const DEFAULT_CONFIG: &str = include_str!("../../config.default.toml");

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub colors: ColorConfig,
    #[serde(default)]
    pub paths: PathConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(skip)]
    pub parsed_colors: OnceLock<ParsedColors>,
}

#[derive(Debug, Clone)]
pub struct ParsedColors {
    pub header_bg: Color,
    pub header_fg: Color,
    pub gauge_low: Color,
    pub gauge_mid: Color,
    pub gauge_high: Color,
    pub gauge_empty: Color,
    pub sparkline: Color,
    pub table_header: Color,
    pub table_row_a: Color,
    pub table_row_b: Color,
    pub alert_bg: Color,
    pub alert_fg: Color,
    pub border: Color,
    pub accent: Color,
    pub highlight: Color,
}

impl AppConfig {
    pub fn get_colors(&self) -> &ParsedColors {
        self.parsed_colors.get_or_init(|| ParsedColors {
            header_bg: self.parse_color(&self.colors.header_bg),
            header_fg: self.parse_color(&self.colors.header_fg),
            gauge_low: self.parse_color(&self.colors.gauge_low),
            gauge_mid: self.parse_color(&self.colors.gauge_mid),
            gauge_high: self.parse_color(&self.colors.gauge_high),
            gauge_empty: self.parse_color(&self.colors.gauge_empty),
            sparkline: self.parse_color(&self.colors.sparkline),
            table_header: self.parse_color(&self.colors.table_header),
            table_row_a: self.parse_color(&self.colors.table_row_a),
            table_row_b: self.parse_color(&self.colors.table_row_b),
            alert_bg: self.parse_color(&self.colors.alert_bg),
            alert_fg: self.parse_color(&self.colors.alert_fg),
            border: self.parse_color(&self.colors.border),
            accent: self.parse_color(&self.colors.accent),
            highlight: self.parse_color(&self.colors.highlight),
        })
    }

    /// Parse a hex color string like "#RRGGBB" into a ratatui Color.
    pub fn parse_color(&self, hex: &str) -> Color {
        parse_hex_color(hex)
    }
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneralConfig {
    #[serde(default = "default_refresh_rate")]
    pub refresh_rate_ms: u64,
    #[serde(default = "default_ui_fps")]
    pub ui_fps: u32,
    #[serde(default = "default_alert_duration")]
    pub alert_duration_secs: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            refresh_rate_ms: default_refresh_rate(),
            ui_fps: default_ui_fps(),
            alert_duration_secs: default_alert_duration(),
        }
    }
}

fn default_refresh_rate() -> u64 {
    1000
}
fn default_ui_fps() -> u32 {
    60
}
fn default_alert_duration() -> u64 {
    5
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorConfig {
    #[serde(default = "default_header_bg")]
    pub header_bg: String,
    #[serde(default = "default_header_fg")]
    pub header_fg: String,
    #[serde(default = "default_gauge_low")]
    pub gauge_low: String,
    #[serde(default = "default_gauge_mid")]
    pub gauge_mid: String,
    #[serde(default = "default_gauge_high")]
    pub gauge_high: String,
    #[serde(default = "default_gauge_empty")]
    pub gauge_empty: String,
    #[serde(default = "default_sparkline")]
    pub sparkline: String,
    #[serde(default = "default_table_header")]
    pub table_header: String,
    #[serde(default = "default_table_row_a")]
    pub table_row_a: String,
    #[serde(default = "default_table_row_b")]
    pub table_row_b: String,
    #[serde(default = "default_alert_bg")]
    pub alert_bg: String,
    #[serde(default = "default_alert_fg")]
    pub alert_fg: String,
    #[serde(default = "default_border")]
    pub border: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_highlight")]
    pub highlight: String,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            header_bg: default_header_bg(),
            header_fg: default_header_fg(),
            gauge_low: default_gauge_low(),
            gauge_mid: default_gauge_mid(),
            gauge_high: default_gauge_high(),
            gauge_empty: default_gauge_empty(),
            sparkline: default_sparkline(),
            table_header: default_table_header(),
            table_row_a: default_table_row_a(),
            table_row_b: default_table_row_b(),
            alert_bg: default_alert_bg(),
            alert_fg: default_alert_fg(),
            border: default_border(),
            accent: default_accent(),
            highlight: default_highlight(),
        }
    }
}

fn default_header_bg() -> String {
    "#1a1b26".into()
}
fn default_header_fg() -> String {
    "#c0caf5".into()
}
fn default_gauge_low() -> String {
    "#9ece6a".into()
}
fn default_gauge_mid() -> String {
    "#e0af68".into()
}
fn default_gauge_high() -> String {
    "#f7768e".into()
}
fn default_gauge_empty() -> String {
    "#3b4261".into()
}
fn default_sparkline() -> String {
    "#7aa2f7".into()
}
fn default_table_header() -> String {
    "#bb9af7".into()
}
fn default_table_row_a() -> String {
    "#1a1b26".into()
}
fn default_table_row_b() -> String {
    "#24283b".into()
}
fn default_alert_bg() -> String {
    "#f7768e".into()
}
fn default_alert_fg() -> String {
    "#1a1b26".into()
}
fn default_border() -> String {
    "#565f89".into()
}
fn default_accent() -> String {
    "#7aa2f7".into()
}
fn default_highlight() -> String {
    "#283457".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PathConfig {
    /// Path to auth log file (Linux/Unix only).
    #[serde(default)]
    pub auth_log: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    #[serde(default = "default_public_ip_url")]
    pub public_ip_url: String,
    #[serde(default = "default_geoip_url_template")]
    pub geoip_url_template: String,
    #[serde(default = "default_geoip_cache_size")]
    pub geoip_cache_size: usize,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            public_ip_url: default_public_ip_url(),
            geoip_url_template: default_geoip_url_template(),
            geoip_cache_size: default_geoip_cache_size(),
            request_timeout_secs: default_request_timeout_secs(),
        }
    }
}

fn default_public_ip_url() -> String {
    "https://api.ipify.org".into()
}
fn default_geoip_url_template() -> String {
    "http://ip-api.com/json/{ip}?fields=status,country,city".into()
}
fn default_geoip_cache_size() -> usize {
    128
}
fn default_request_timeout_secs() -> u64 {
    3
}

// ---------------------------------------------------------------------------
// Color parsing
// ---------------------------------------------------------------------------

/// Parse a hex color string like "#RRGGBB" into a ratatui Color.
fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

// ---------------------------------------------------------------------------
// Loading & saving
// ---------------------------------------------------------------------------

impl AppConfig {
    /// Load configuration.
    ///
    /// Priority:
    /// 1. User config file (`~/.config/rmonitor/config.toml` or `%APPDATA%/rmonitor/config.toml`)
    /// 2. Embedded default config
    pub fn load() -> Self {
        if let Some(path) = Self::user_config_path() {
            if path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = toml::from_str::<AppConfig>(&contents) {
                        return cfg;
                    }
                    eprintln!(
                        "Warning: Failed to parse config at {}. Using defaults.",
                        path.display()
                    );
                }
            }
        }

        // Fallback to embedded defaults
        toml::from_str::<AppConfig>(DEFAULT_CONFIG)
            .expect("Embedded default config must be valid TOML")
    }

    /// Serialize the current config to TOML and write to the user config path.
    /// Creates parent directories if they don't exist.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::user_config_path()
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&path, toml_str)
            .map_err(|e| format!("Failed to write config to {}: {}", path.display(), e))?;

        Ok(())
    }

    /// Returns the platform-appropriate user config path.
    pub fn user_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("rmonitor").join("config.toml"))
    }
}
