#![cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "macos"))]

use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::fs::File;
use tokio::process::Command;
use std::process::Stdio;

use crate::core::config::AppConfig;
use crate::modules::network::provider::GeoIpCache;
use crate::providers::ConnectionProvider;
use crate::core::state::{AlertEntry, ConnectionEntry};
use crate::core::store::Store;

pub struct UnixConnectionProvider {
    log_path: PathBuf,
    geo_cache: std::sync::Arc<GeoIpCache>,
    alert_dur_secs: i64,
}

impl UnixConnectionProvider {
    pub fn new(config: &AppConfig, geo_cache: std::sync::Arc<GeoIpCache>) -> Self {
        let log_path = if let Some(ref p) = config.paths.auth_log {
            PathBuf::from(p)
        } else {
            let defaults = ["/var/log/auth.log", "/var/log/secure", "/var/log/authlog"];
            defaults.iter()
                .map(|p| PathBuf::from(p))
                .find(|p| p.exists())
                .unwrap_or_else(|| PathBuf::from("/var/log/auth.log"))
        };
        Self {
            log_path,
            geo_cache,
            alert_dur_secs: config.general.alert_duration_secs as i64,
        }
    }

    #[cfg(target_os = "linux")]
    async fn watch_journal(&self, store: &Store) -> Result<(), Box<dyn std::error::Error>> {
        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.]+)"
        ).unwrap();
        let re_close = Regex::new(
            r"sshd\[(\d+)\]:\s+pam_unix\(sshd:session\):\s+session\s+closed\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Load recent history (last 24h) to catch currently active sessions
        let history = Command::new("journalctl")
            .args(&["-o", "short", "--no-hostname", "-t", "sshd", "--since", "24 hours ago"])
            .output()
            .await?;

        if history.status.success() {
            let content = String::from_utf8_lossy(&history.stdout);
            for line in content.lines() {
                // Don't push alerts for historical connections
                self.process_line_internal(line, store, &re_open, &re_close, alert_dur, false).await;
            }
        }

        // 2. Start following for live updates
        let mut child = Command::new("journalctl")
            .args(&["-f", "-o", "short", "--no-hostname", "-t", "sshd", "--since", "now"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().ok_or("Failed to capture journalctl stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();

        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    self.process_line_internal(&buf, store, &re_open, &re_close, alert_dur, true).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    async fn process_line(
        &self,
        line: &str,
        store: &Store,
        re_open: &Regex,
        re_close: &Regex,
        alert_dur: chrono::Duration,
    ) {
        self.process_line_internal(line, store, re_open, re_close, alert_dur, true).await;
    }

    async fn process_line_internal(
        &self,
        line: &str,
        store: &Store,
        re_open: &Regex,
        re_close: &Regex,
        alert_dur: chrono::Duration,
        push_alert: bool,
    ) {
        let line = line.trim();
        if let Some(caps) = re_open.captures(line) {
            let pid = &caps[1];
            let method = &caps[2];
            let user = &caps[3];
            let ip = &caps[4];
            let sid = format!("ssh-{}-{}", pid, user);
            let geo = self.geo_cache.lookup(ip).await;
            let now = Utc::now();
            let mut st = store.write().await;
            st.add_connection(ConnectionEntry {
                user: user.into(),
                source_ip: ip.into(),
                protocol: format!("SSH ({})", method),
                login_time: now,
                location: geo.display(),
                session_id: sid,
            });
            if push_alert {
                st.push_alert(AlertEntry {
                    message: format!("SSH login: {}@{}", user, ip),
                    timestamp: now,
                    expires_at: now + alert_dur,
                });
            }
        }
        if let Some(caps) = re_close.captures(line) {
            let sid = format!("ssh-{}-{}", &caps[1], &caps[2]);
            store.write().await.remove_connection(&sid);
        }
    }

    async fn watch_files(&self, store: &Store) {
        let file = match File::open(&self.log_path).await {
            Ok(f) => f,
            Err(e) => {
                let msg = format!(
                    "Cannot open {}: {} (try running as root)",
                    self.log_path.display(), e
                );
                let mut st = store.write().await;
                if !st.permission_warnings.contains(&msg) {
                    st.permission_warnings.push(msg);
                }
                return;
            }
        };

        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.]+)"
        ).unwrap();
        let re_close = Regex::new(
            r"sshd\[(\d+)\]:\s+pam_unix\(sshd:session\):\s+session\s+closed\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Read history from the file first
        let mut reader = BufReader::new(file);
        let mut buf = String::new();
        
        // We read the whole file to find active sessions. 
        // For very large logs, this might be slow, but auth logs are usually rotated.
        while let Ok(n) = reader.read_line(&mut buf).await {
            if n == 0 { break; }
            self.process_line_internal(&buf, store, &re_open, &re_close, alert_dur, false).await;
            buf.clear();
        }

        // 2. Continue tailing for live updates
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(_) => {
                    self.process_line_internal(&buf, store, &re_open, &re_close, alert_dur, true).await;
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[async_trait]
impl ConnectionProvider for UnixConnectionProvider {
    async fn watch_connections(&self, store: &Store) {
        #[cfg(target_os = "linux")]
        {
            // Try journalctl first on Linux
            if let Err(e) = self.watch_journal(store).await {
                // Only log if it's not a "not found" error, or if it failed after starting
                eprintln!("journalctl watcher failed or not available: {}. Falling back to file tailing.", e);
            }
        }

        self.watch_files(store).await;
    }
}
