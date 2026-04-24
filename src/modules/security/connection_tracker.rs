#[cfg(target_os = "linux")]
use procfs::net::{tcp, tcp6};

pub struct ConnectionTracker;

#[allow(dead_code)]
pub struct ConnectionInfo {
    pub remote_ip: String,
    pub protocol: String,
}

impl ConnectionTracker {
    /// Fetches all established TCP connections (IPv4 and IPv6) from /proc/net.
    /// Returns a list of ConnectionInfo objects.
    #[cfg(target_os = "linux")]
    pub fn get_established_connections() -> anyhow::Result<Vec<ConnectionInfo>> {
        let mut connections = Vec::new();

        // IPv4 connections
        if let Ok(tcp_v4) = tcp() {
            for entry in tcp_v4 {
                if entry.state == procfs::net::TcpState::Established {
                    let remote_ip = entry.remote_address.ip().to_string();
                    if remote_ip != "0.0.0.0" && remote_ip != "127.0.0.1" {
                        let local_port = entry.local_address.port();
                        let protocol = match local_port {
                            22 => "SSH (socket)".to_string(),
                            3389 => "RDP (socket)".to_string(),
                            _ => format!("TCP:{}", local_port),
                        };
                        connections.push(ConnectionInfo { remote_ip, protocol });
                    }
                }
            }
        }

        // IPv6 connections
        if let Ok(tcp_v6) = tcp6() {
            for entry in tcp_v6 {
                if entry.state == procfs::net::TcpState::Established {
                    let remote_ip = entry.remote_address.ip().to_string();
                    if remote_ip != "::" && remote_ip != "::1" && !remote_ip.starts_with("::ffff:127.") {
                        let local_port = entry.local_address.port();
                        let protocol = match local_port {
                            22 => Some("SSH (socket)".to_string()),
                            3389 => Some("RDP (socket)".to_string()),
                            _ => None,
                        };
                        if let Some(proto) = protocol {
                            connections.push(ConnectionInfo { remote_ip, protocol: proto });
                        }
                    }
                }
            }
        }

        Ok(connections)
    }

    /// Fetches established connections on Windows by parsing netstat output.
    #[cfg(target_os = "windows")]
    pub fn get_established_connections() -> anyhow::Result<Vec<ConnectionInfo>> {
        use std::process::Command;

        let output = Command::new("netstat")
            .arg("-an")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut connections = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Expected format: TCP    Local_IP:Port    Remote_IP:Port    ESTABLISHED
            if parts.len() >= 4 && parts[0] == "TCP" && parts[parts.len() - 1] == "ESTABLISHED" {
                let local_addr = parts[1];
                let remote_addr = parts[2];

                // Parse Local Port
                let local_port = local_addr.rsplit(':').next().unwrap_or("0");

                // Parse Remote IP
                let remote_ip = if remote_addr.starts_with('[') {
                    // IPv6 like [::1]:port
                    remote_addr.trim_start_matches('[').split(']').next().unwrap_or("")
                } else {
                    remote_addr.rsplit(':').nth(1).unwrap_or("")
                };

                if remote_ip != "127.0.0.1" && remote_ip != "0.0.0.0" && remote_ip != "::1" && remote_ip != "::" && !remote_ip.is_empty() {
                    let protocol = match local_port {
                        "22" => Some("SSH (socket)".to_string()),
                        "3389" => Some("RDP (socket)".to_string()),
                        _ => None,
                    };

                    if let Some(proto) = protocol {
                        connections.push(ConnectionInfo {
                            remote_ip: remote_ip.to_string(),
                            protocol: proto,
                        });
                    }
                }
            }
        }

        Ok(connections)
    }

    /// Fallback for non-Linux platforms to maintain cross-platform compilation.
    #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
    #[allow(dead_code)]
    pub fn get_established_connections() -> anyhow::Result<Vec<ConnectionInfo>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_compiles() {
        let conns = ConnectionTracker::get_established_connections();
        assert!(conns.is_ok());
    }
}
