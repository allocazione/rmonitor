//! Security monitoring — connection tracking (SSH/RDP), alerts, and panel UI.
//!
//! Platform-specific providers:
//! - `unix`: uses `who -u` + journalctl/auth.log tailing
//! - `windows`: uses Windows Event Log (Security channel, Event IDs 4624/4634)

pub mod unix;
pub mod windows;
pub mod panel;
pub mod alerts;
