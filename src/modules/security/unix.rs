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

    async fn fetch_who_sessions(&self, store: &Store) {
        let output = match Command::new("who").arg("-u").output().await {
            Ok(o) => o,
            Err(_) => {
                // Fallback to plain who if -u fails
                match Command::new("who").output().await {
                    Ok(o) => o,
                    Err(_) => return,
                }
            }
        };

        let content = String::from_utf8_lossy(&output.stdout);
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 { continue; }

            let user = parts[0];
            let tty = parts[1];
            
            // Try to find IP in parentheses
            let mut ip = "local".to_string();
            for part in &parts {
                if part.starts_with('(') && part.ends_with(')') {
                    ip = part[1..part.len()-1].to_string();
                    break;
                }
            }

            let sid = if ip == "local" {
                format!("unix-local-{}", tty)
            } else {
                format!("unix-remote-{}", tty)
            };

            let protocol = if ip == "local" { "Console".to_string() } else { "SSH".to_string() };
            let geo = if ip != "local" {
                self.geo_cache.lookup(&ip).await
            } else {
                crate::modules::network::provider::GeoInfo {
                    country: "Local".into(),
                    city: String::new(),
                }
            };

            let mut st = store.write().await;
            st.add_connection(ConnectionEntry {
                user: user.into(),
                source_ip: ip,
                protocol,
                login_time: Utc::now(),
                location: geo.display(),
                session_id: sid,
            });
        }
    }

    #[cfg(target_os = "linux")]
    async fn watch_journal(&self, store: &Store) -> Result<(), Box<dyn std::error::Error>> {
        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.:a-fA-F]+)"
        ).unwrap();
        let re_pam = Regex::new(
            r"(\S+)\[(\d+)\]:\s+pam_unix\((\S+):session\):\s+session\s+(opened|closed)\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Fetch current sessions from 'who' first
        self.fetch_who_sessions(store).await;

        // 2. Load recent history (last 24h)
        let history = Command::new("journalctl")
            .args(&["-o", "short", "--no-hostname", "-t", "sshd", "-t", "login", "-t", "systemd-logind", "--since", "24 hours ago"])
            .output()
            .await?;

        if history.status.success() {
            let content = String::from_utf8_lossy(&history.stdout);
            for line in content.lines() {
                self.process_line_internal(line, store, &re_open, &re_pam, alert_dur, false).await;
            }
        }

        // 3. Start following for live updates
        let mut child = Command::new("journalctl")
            .args(&["-f", "-o", "short", "--no-hostname", "-t", "sshd", "-t", "login", "-t", "systemd-logind", "--since", "now"])
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
                    self.process_line_internal(&buf, store, &re_open, &re_pam, alert_dur, true).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    async fn process_line_internal(
        &self,
        line: &str,
        store: &Store,
        re_open: &Regex,
        re_pam: &Regex,
        alert_dur: chrono::Duration,
        push_alert: bool,
    ) {
        let line = line.trim();
        
        // Handle SSH Accepted lines (to get IP)
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

        // Handle PAM session opened/closed (generic for local and remote)
        if let Some(caps) = re_pam.captures(line) {
            let proc_name = &caps[1];
            let pid = &caps[2];
            let service = &caps[3];
            let action = &caps[4];
            let user = &caps[5];
            
            let sid = if service == "sshd" {
                format!("ssh-{}-{}", pid, user)
            } else {
                format!("pam-{}-{}", pid, user)
            };

            if action == "opened" {
                // Only add if it's not already there (SSH Accepted might have added it already)
                let mut st = store.write().await;
                if !st.connections.iter().any(|c| c.session_id == sid) {
                    let now = Utc::now();
                    st.add_connection(ConnectionEntry {
                        user: user.into(),
                        source_ip: "local".into(),
                        protocol: if service == "sshd" { "SSH".into() } else { format!("Local ({})", service) },
                        login_time: now,
                        location: "Local".into(),
                        session_id: sid,
                    });
                    if push_alert {
                        st.push_alert(AlertEntry {
                            message: format!("Session opened: {} ({})", user, service),
                            timestamp: now,
                            expires_at: now + alert_dur,
                        });
                    }
                }
            } else {
                // session closed
                store.write().await.remove_connection(&sid);
            }
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
                // Even if file fails, we still have 'who' data
                return;
            }
        };

        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.:a-fA-F]+)"
        ).unwrap();
        let re_pam = Regex::new(
            r"(\S+)\[(\d+)\]:\s+pam_unix\((\S+):session\):\s+session\s+(opened|closed)\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Fetch current sessions from 'who'
        self.fetch_who_sessions(store).await;

        // 2. Read history from the file
        let mut reader = BufReader::new(file);
        let mut buf = String::new();
        
        while let Ok(n) = reader.read_line(&mut buf).await {
            if n == 0 { break; }
            self.process_line_internal(&buf, store, &re_open, &re_pam, alert_dur, false).await;
            buf.clear();
        }

        // 3. Continue tailing for live updates
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(_) => {
                    self.process_line_internal(&buf, store, &re_open, &re_pam, alert_dur, true).await;
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
                eprintln!("journalctl watcher failed or not available: {}. Falling back to file tailing.", e);
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-linux (BSD/macOS), we always fetch who and then tail files
            self.fetch_who_sessions(store).await;
            self.watch_files(store).await;
        }
    }
}
