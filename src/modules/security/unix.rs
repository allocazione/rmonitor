#![cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "macos"))]

use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::fs::File;
use tokio::process::Command;
use std::process::Stdio;

use crate::core::config::AppConfig;
use crate::modules::network::provider::GeoIpCache;
use crate::providers::ConnectionProvider;
use crate::core::state::{AlertEntry, ConnectionEntry};
use crate::core::store::Store;
use crate::modules::security::connection_tracker::ConnectionTracker;

use std::sync::Arc;

pub struct UnixConnectionProvider {
    #[allow(dead_code)]
    log_path: PathBuf,
    geo_cache: Arc<GeoIpCache>,
    alert_dur_secs: i64,
}

impl UnixConnectionProvider {
    pub fn new(config: &AppConfig, geo_cache: Arc<GeoIpCache>) -> Self {
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

    async fn fetch_who_sessions(&self) -> Vec<ConnectionEntry> {
        let output = match Command::new("who")
            .arg("-u")
            .env("LANG", "C")
            .output()
            .await {
            Ok(o) => o,
            Err(_) => {
                match Command::new("who")
                    .env("LANG", "C")
                    .output()
                    .await {
                    Ok(o) => o,
                    Err(_) => return Vec::new(),
                }
            }
        };

        let mut entries = Vec::new();
        let content = String::from_utf8_lossy(&output.stdout);
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 { continue; }

            let user = parts[0];
            
            // TTY is always the second field in `who` output (parts[1])
            let tty = parts[1].to_string();
            let mut ip = "local".to_string();
            
            // Scan remaining parts only for IP in parentheses
            for part in parts.iter().skip(2) {
                if part.starts_with('(') && part.ends_with(')') {
                    let inner = &part[1..part.len()-1];
                    if !inner.is_empty() && !inner.starts_with(':') {
                        ip = inner.to_string();
                    }
                    break; // IP found, no need to continue
                }
            }

            // session_id should be as unique as possible to avoid collisions
            let sid = format!("unix-{}-{}", user, tty);
            let protocol = if ip == "local" { "Console".to_string() } else { "SSH".to_string() };
            let geo = if ip != "local" {
                self.geo_cache.lookup(&ip).await
            } else {
                crate::modules::network::provider::GeoInfo {
                    country: "Local".into(),
                    city: String::new(),
                }
            };

            entries.push(ConnectionEntry {
                user: user.into(),
                source_ip: ip,
                protocol,
                login_time: Utc::now(),
                location: geo.display(),
                session_id: sid,
            });
        }
        entries
    }

    async fn fetch_established_network_hosts(
        &self,
        existing_ips: &HashSet<String>,
    ) -> Vec<ConnectionEntry> {
        let mut network_entries = Vec::new();

        // Use the ConnectionTracker (procfs adapter) for reliable, 
        // high-performance connection tracking on Debian.
        let connections = match ConnectionTracker::get_established_connections() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        for conn in connections {
            // Only add if not already present from session sources
            if !existing_ips.contains(&conn.remote_ip) {
                let geo = self.geo_cache.lookup(&conn.remote_ip).await;
                network_entries.push(ConnectionEntry {
                    user: "unknown (socket)".into(),
                    source_ip: conn.remote_ip,
                    protocol: conn.protocol,
                    login_time: Utc::now(),
                    location: geo.display(),
                    session_id: format!("socket-{}", conn.remote_ip),
                });
            }
        }

        network_entries
    }

    async fn sync_sessions(&self, store: &Store) {
        let mut fresh_sessions = self.fetch_who_sessions().await;
        let who_remote_count = fresh_sessions
            .iter()
            .filter(|s| s.source_ip != "local")
            .count();
        let existing_ips: HashSet<String> = fresh_sessions
            .iter()
            .filter(|s| s.source_ip != "local")
            .map(|s| s.source_ip.clone())
            .collect();
        let mut network_hosts = self.fetch_established_network_hosts(&existing_ips).await;
        let socket_host_count = network_hosts.len();
        fresh_sessions.append(&mut network_hosts);
        let mut st = store.write().await;
        
        // Remove sessions that are no longer present in our poll-based sources.
        // Keep log-driven session IDs ("ssh-" / "pam-") so live journal events still work.
        st.connections.retain(|c| {
            if c.session_id.starts_with("unix-") || c.session_id.starts_with("net-host-") {
                fresh_sessions.iter().any(|f| f.session_id == c.session_id)
            } else {
                true // Keep log-based sessions for now, they usually have shorter TTL or are removed by 'closed' events
            }
        });

        // Add or update
        for session in fresh_sessions {
            if let Some(existing) = st.connections.iter_mut().find(|c| c.session_id == session.session_id) {
                // Update everything except login_time to preserve the original session start
                existing.user = session.user;
                existing.source_ip = session.source_ip;
                existing.protocol = session.protocol;
                existing.location = session.location;
            } else {
                st.add_connection(session);
            }
        }

        // Monitoring guardrail: detect when login/session sources under-report
        // compared to socket-level evidence, then keep one persistent warning.
        if socket_host_count > who_remote_count {
            let msg = format!(
                "Connection source mismatch detected: who/journal reports {} remote sessions but socket scan found {}. Using socket fallback.",
                who_remote_count, socket_host_count
            );
            if !st.permission_warnings.contains(&msg) {
                st.permission_warnings.push(msg);
            }
        }
    }

    #[cfg(target_os = "linux")]
    async fn watch_journal(&self, store: &Store) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.:a-fA-F]+)"
        ).unwrap();
        let re_pam = Regex::new(
            r"(\S+)\[(\d+)\]:\s+pam_unix\((\S+):session\):\s+session\s+(opened|closed)\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Initial sync
        self.sync_sessions(store).await;

        // 2. Spawn a periodic sync task
        let store_c = store.clone();
        let provider_arc = Arc::new(UnixConnectionProvider {
            log_path: self.log_path.clone(),
            geo_cache: self.geo_cache.clone(),
            alert_dur_secs: self.alert_dur_secs,
        });
        
        let provider_c = provider_arc.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                provider_c.sync_sessions(&store_c).await;
            }
        });

        // 3. Load recent history (last 24h)
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

        // 4. Start following for live updates
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
                    // Trigger a quick sync after a log event to catch 'who' updates
                    self.sync_sessions(store).await;
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
            let _proc_name = &caps[1];
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

    #[allow(dead_code)]
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
                
                // Fallback: just poll 'who' periodically if we can't read logs
                loop {
                    self.sync_sessions(store).await;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        };

        let re_open = Regex::new(
            r"sshd\[(\d+)\]:\s+Accepted\s+(\w+)\s+for\s+(\w+)\s+from\s+([\d.:a-fA-F]+)"
        ).unwrap();
        let re_pam = Regex::new(
            r"(\S+)\[(\d+)\]:\s+pam_unix\((\S+):session\):\s+session\s+(opened|closed)\s+for\s+user\s+(\w+)"
        ).unwrap();
        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // 1. Initial sync
        self.sync_sessions(store).await;

        // 2. Spawn periodic sync
        let store_c = store.clone();
        let provider_arc = Arc::new(UnixConnectionProvider {
            log_path: self.log_path.clone(),
            geo_cache: self.geo_cache.clone(),
            alert_dur_secs: self.alert_dur_secs,
        });
        let provider_c = provider_arc.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                provider_c.sync_sessions(&store_c).await;
            }
        });

        // 3. Read history from the file
        let mut reader = BufReader::new(file);
        let mut buf = String::new();
        
        while let Ok(n) = reader.read_line(&mut buf).await {
            if n == 0 { break; }
            self.process_line_internal(&buf, store, &re_open, &re_pam, alert_dur, false).await;
            buf.clear();
        }

        // 4. Continue tailing for live updates
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(_) => {
                    self.process_line_internal(&buf, store, &re_open, &re_pam, alert_dur, true).await;
                    self.sync_sessions(store).await;
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UnixConnectionProvider;

    #[test]
    fn parse_host_port_ipv4() {
        let parsed = UnixConnectionProvider::parse_host_port("192.168.1.10:22");
        assert_eq!(parsed, Some(("192.168.1.10".to_string(), "22".to_string())));
    }

    #[test]
    fn parse_host_port_ipv6() {
        let parsed = UnixConnectionProvider::parse_host_port("[2001:db8::1]:3389");
        assert_eq!(parsed, Some(("2001:db8::1".to_string(), "3389".to_string())));
    }

    #[test]
    fn parse_established_line_ss_and_netstat() {
        let ss_line = "0 0 10.0.0.5:22 10.0.0.21:54122";
        let netstat_line = "tcp 0 0 10.0.0.5:22 10.0.0.22:54720 ESTABLISHED";

        let a = UnixConnectionProvider::parse_established_line(ss_line);
        let b = UnixConnectionProvider::parse_established_line(netstat_line);

        assert_eq!(a, Some(("10.0.0.21".to_string(), "SSH (socket)".to_string())));
        assert_eq!(b, Some(("10.0.0.22".to_string(), "SSH (socket)".to_string())));
    }

    #[test]
    fn parse_established_line_ignores_loopback_and_unknown_ports() {
        let loopback = "0 0 127.0.0.1:22 127.0.0.1:55555";
        let unknown = "0 0 10.0.0.5:8443 10.0.0.9:55211";

        assert_eq!(UnixConnectionProvider::parse_established_line(loopback), None);
        assert_eq!(UnixConnectionProvider::parse_established_line(unknown), None);
    }
}

#[async_trait]
impl ConnectionProvider for UnixConnectionProvider {
    async fn watch_connections(&self, store: &Store) {
        #[cfg(target_os = "linux")]
        {
            // Try journalctl first on Linux
            let mut failed = false;
            match self.watch_journal(store).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("journalctl watcher failed or not available: {}. Falling back to file tailing.", e);
                    failed = true;
                }
            }
            
            if failed {
                // On failure, fall through to watch_files
                self.watch_files(store).await;
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-linux (BSD/macOS), we poll who and then tail files
            self.watch_files(store).await;
        }
    }
}
