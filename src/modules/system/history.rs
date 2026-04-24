use std::fs;

#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::path::Path;

use crate::core::state::UserCommandInfo;
use crate::core::store::Store;

/// Fetch command history for all users on the system.
pub fn fetch_user_history() -> Vec<UserCommandInfo> {
    let mut users = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Use /etc/passwd to find users, but handle potential lack of access gracefully
        if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
            for line in passwd.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 6 {
                    let username = parts[0].to_string();
                    let home_dir = parts[5];
                    
                    // Focus on human users (typically in /home or /root)
                    if home_dir.starts_with("/home/") || home_dir == "/root" {
                        if let Some(info) = get_linux_user_history(username, home_dir) {
                            users.push(info);
                        }
                    }
                }
            }
        } else {
            // Fallback: If /etc/passwd is inaccessible, at least try current user's home
            if let Ok(home) = std::env::var("HOME") {
                if let Ok(user) = std::env::var("USER") {
                    if let Some(info) = get_linux_user_history(user, &home) {
                        users.push(info);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(entries) = fs::read_dir("C:\\Users") {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let username = entry.file_name().to_string_lossy().to_string();
                        let lower = username.to_lowercase();
                        if lower == "public" || lower == "default" || lower == "all users" || lower == "default user" {
                            continue;
                        }
                        let home_path = entry.path();
                        if let Some(info) = get_windows_user_history(username, &home_path) {
                            users.push(info);
                        }
                    }
                }
            }
        }
    }

    users
}

#[cfg(target_os = "linux")]
fn get_linux_user_history(username: String, home_dir: &str) -> Option<UserCommandInfo> {
    let history_files = [".bash_history", ".zsh_history", ".history"];
    let mut history = VecDeque::with_capacity(300);
    let mut last_command = String::new();

    for file in history_files {
        let path = PathBuf::from(home_dir).join(file);
        if let Ok(content) = fs::read_to_string(&path) {
            let filtered_lines: Vec<String> = content
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| {
                    // Handle Zsh extended history format: ": 1234567890:0;command"
                    if line.starts_with(':') {
                        if let Some(pos) = line.find(';') {
                            return line[pos + 1..].to_string();
                        }
                    }
                    line.to_string()
                })
                .collect();

            if !filtered_lines.is_empty() {
                // Take last 300
                let start = filtered_lines.len().saturating_sub(300);
                let last_300: Vec<String> = filtered_lines[start..].iter().rev().cloned().collect();
                
                last_command = last_300[0].clone();
                history = last_300.into_iter().collect();
                break;
            }
        }
    }

    if !history.is_empty() {
        Some(UserCommandInfo { username, last_command, history })
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn get_windows_user_history(username: String, home_path: &Path) -> Option<UserCommandInfo> {
    let ps_history_path = home_path.join("AppData\\Roaming\\Microsoft\\Windows\\PowerShell\\PSReadLine\\ConsoleHost_history.txt");
    
    if let Ok(content) = fs::read_to_string(&ps_history_path) {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        if !lines.is_empty() {
            let start = lines.len().saturating_sub(300);
            let last_300: Vec<String> = lines[start..].iter().rev().cloned().collect();

            return Some(UserCommandInfo {
                username,
                last_command: last_300[0].clone(),
                history: last_300.into_iter().collect(),
            });
        }
    }
    None
}

/// Periodically refresh user command history in the background.
pub async fn watch_user_history(store: Store) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        let history = fetch_user_history();
        let mut state = store.write().await;
        state.user_commands = history;
    }
}
