use procfs::net::{tcp, tcp6};

pub struct ConnectionTracker;

pub struct ConnectionInfo {
    pub remote_ip: String,
    pub protocol: String,
}

impl ConnectionTracker {
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
                            22 => "SSH (socket)".to_string(),
                            3389 => "RDP (socket)".to_string(),
                            _ => format!("TCP:{}", local_port),
                        };
                        connections.push(ConnectionInfo { remote_ip, protocol });
                    }
                }
            }
        }

        Ok(connections)
    }
}
