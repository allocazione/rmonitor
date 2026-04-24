use std::fs;
use std::path::PathBuf;
use crate::core::state::UserCommandInfo;
use crate::core::store::Store;
use std::collections::VecDeque;

/// Fetch command history for all users on the system.
pub fn fetch_user_history() -> Vec<UserCommandInfo> {
    let mut users = Vec::new();

    #[cfg(target_os = "linux")]
    {
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
    let mut history = VecDeque::with_capacity(100);
    let mut last_command = String::new();

    for file in history_files {
        let path = PathBuf::from(home_dir).join(file);
        if let Ok(content) = fs::read_to_string(&path) {
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if !lines.is_empty() {
                // Take last 100
                let start = lines.len().saturating_sub(100);
                let last_100: Vec<String> = lines[start..].iter().rev().cloned().collect();
                
                last_command = last_100[0].clone();
                history = last_100.into_iter().collect();
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
fn get_windows_user_history(username: String, home_path: &PathBuf) -> Option<UserCommandInfo> {
    let ps_history_path = home_path.join("AppData\\Roaming\\Microsoft\\Windows\\PowerShell\\PSReadLine\\ConsoleHost_history.txt");
    
    if let Ok(content) = fs::read_to_string(&ps_history_path) {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        if !lines.is_empty() {
            let start = lines.len().saturating_sub(100);
            let last_100: Vec<String> = lines[start..].iter().rev().cloned().collect();

            return Some(UserCommandInfo {
                username,
                last_command: last_100[0].clone(),
                history: last_100.into_iter().collect(),
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
