//! Windows connection provider — subscribes to Security Event Log
//! for Event IDs 4624 (Logon) and 4634 (Logoff) using EvtSubscribe.
//!
//! Only compiled on `target_os = "windows"`.

#![cfg(target_os = "windows")]

use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::config::AppConfig;
use crate::modules::network::provider::GeoIpCache;
use crate::providers::ConnectionProvider;
use crate::core::state::{AlertEntry, ConnectionEntry};
use crate::core::store::Store;

/// Parsed Windows logon event.
#[derive(Debug, Clone)]
enum WinEvent {
    Logon {
        user: String,
        source_ip: String,
        logon_type: u32,
        logon_id: String,
    },
    Logoff {
        logon_id: String,
    },
}

pub struct WindowsConnectionProvider {
    geo_cache: Arc<GeoIpCache>,
    alert_dur_secs: i64,
}

impl WindowsConnectionProvider {
    pub fn new(config: &AppConfig, geo_cache: Arc<GeoIpCache>) -> Self {
        Self {
            geo_cache,
            alert_dur_secs: config.general.alert_duration_secs as i64,
        }
    }
}

#[async_trait]
impl ConnectionProvider for WindowsConnectionProvider {
    async fn watch_connections(&self, store: &Store) {
        let (tx, mut rx) = mpsc::unbounded_channel::<WinEvent>();

        // Spawn the blocking Windows Event Log subscription on a dedicated thread
        let tx_clone = tx.clone();
        let store_clone = store.clone();
        let _sub_handle = std::thread::spawn(move || {
            if let Err(e) = subscribe_security_events(tx_clone) {
                // Send the error as a permission warning via a blocking approach
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    store_clone
                        .write()
                        .await
                        .permission_warnings
                        .push(format!("EventLog: {}", e));
                });
            }
        });

        let alert_dur = chrono::Duration::seconds(self.alert_dur_secs);

        // Process events from the channel
        while let Some(evt) = rx.recv().await {
            match evt {
                WinEvent::Logon {
                    user,
                    source_ip,
                    logon_type,
                    logon_id,
                } => {
                    let protocol = match logon_type {
                        10 => "RDP".to_string(),
                        3 => "Network".to_string(),
                        2 => "Console".to_string(),
                        _ => format!("Type {}", logon_type),
                    };

                    let ip_display = if source_ip.is_empty() || source_ip == "-" {
                        "local".to_string()
                    } else {
                        source_ip.clone()
                    };

                    let geo = if ip_display != "local" {
                        self.geo_cache.lookup(&ip_display).await
                    } else {
                        crate::modules::network::provider::GeoInfo {
                            country: "Local".into(),
                            city: String::new(),
                        }
                    };

                    let now = Utc::now();
                    let conn = ConnectionEntry {
                        user: user.clone(),
                        source_ip: ip_display.clone(),
                        protocol,
                        login_time: now,
                        location: geo.display(),
                        session_id: logon_id,
                    };
                    let alert = AlertEntry {
                        message: format!("Login: {} from {}", user, ip_display),
                        timestamp: now,
                        expires_at: now + alert_dur,
                    };

                    let mut state = store.write().await;
                    state.add_connection(conn);
                    state.push_alert(alert);
                }
                WinEvent::Logoff { logon_id } => {
                    store.write().await.remove_connection(&logon_id);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows Event Log subscription (blocking, runs on its own OS thread)
// ---------------------------------------------------------------------------

fn subscribe_security_events(
    tx: mpsc::UnboundedSender<WinEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use windows::core::w;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::EventLog::*;
    use windows::Win32::System::Threading::{
        CreateEventW, ResetEvent, WaitForSingleObject, INFINITE,
    };

    unsafe {
        // Create a Win32 event for signaling
        let signal: HANDLE = CreateEventW(None, true, false, None)?;
        if signal.is_invalid() {
            return Err("Failed to create signal event".into());
        }

        let channel = w!("Security");
        let query = w!("*[System[(EventID=4624 or EventID=4634)]]");

        let sub: EVT_HANDLE = match EvtSubscribe(
            EVT_HANDLE::default(),
            signal,
            channel,
            query,
            EVT_HANDLE::default(),
            None,
            None,
            EvtSubscribeToFutureEvents.0,
        ) {
            Ok(h) => h,
            Err(e) => {
                return Err(format!(
                    "EvtSubscribe failed (run as Administrator): {}",
                    e
                )
                .into());
            }
        };

        // Buffer for event handles — EvtNext in windows 0.57 takes &mut [isize]
        let mut events: [isize; 16] = [0isize; 16];

        loop {
            let _ = WaitForSingleObject(signal, INFINITE);
            let _ = ResetEvent(signal);

            let mut returned: u32 = 0;
            loop {
                let result = EvtNext(
                    sub,
                    &mut events,
                    1000, // timeout ms
                    0,    // flags
                    &mut returned,
                );

                if result.is_err() || returned == 0 {
                    break;
                }

                for event in events.iter_mut().take(returned as usize) {
                    let handle = EVT_HANDLE(*event);
                    if let Some(evt) = render_event(handle) {
                        let _ = tx.send(evt);
                    }
                    // Close the event handle to prevent resource leaks
                    let _ = EvtClose(handle);
                    *event = 0;
                }
                returned = 0;
            }
        }
    }
}

/// Render an event handle to XML and parse the relevant fields.
unsafe fn render_event(event_handle: EVT_HANDLE) -> Option<WinEvent> {
    use windows::Win32::System::EventLog::*;

    // First call: determine required buffer size
    let mut buf_size: u32 = 0;
    let mut prop_count: u32 = 0;
    let _ = EvtRender(
        EVT_HANDLE::default(),
        event_handle,
        EvtRenderEventXml.0,
        0,
        None,
        &mut buf_size,
        &mut prop_count,
    );

    if buf_size == 0 {
        return None;
    }

    // Second call: render into buffer
    let mut buffer: Vec<u16> = vec![0u16; (buf_size / 2 + 1) as usize];
    let render_result = EvtRender(
        EVT_HANDLE::default(),
        event_handle,
        EvtRenderEventXml.0,
        buf_size,
        Some(buffer.as_mut_ptr() as *mut _),
        &mut buf_size,
        &mut prop_count,
    );

    if render_result.is_err() {
        return None;
    }

    let xml = String::from_utf16_lossy(&buffer);
    parse_event_xml(&xml)
}

use windows::Win32::System::EventLog::EVT_HANDLE;

/// Minimal XML parser — extracts fields from Windows Event XML.
/// We use simple string searching instead of pulling in an XML crate
/// to keep dependencies minimal (zero-copy where possible).
fn parse_event_xml(xml: &str) -> Option<WinEvent> {
    let event_id = extract_xml_value(xml, "EventID")?;

    match event_id.as_str() {
        "4624" => {
            let user = extract_data_value(xml, "TargetUserName")
                .unwrap_or_default();
            let ip = extract_data_value(xml, "IpAddress")
                .unwrap_or_else(|| "-".into());
            let logon_type_str = extract_data_value(xml, "LogonType")
                .unwrap_or_else(|| "0".into());
            let logon_type: u32 = logon_type_str.parse().unwrap_or(0);
            let logon_id = extract_data_value(xml, "TargetLogonId")
                .unwrap_or_else(|| {
                    format!("win-{}", chrono::Utc::now().timestamp_millis())
                });

            // Filter out system/service logons
            if user.ends_with('$')
                || user == "SYSTEM"
                || user == "ANONYMOUS LOGON"
            {
                return None;
            }

            Some(WinEvent::Logon {
                user,
                source_ip: ip,
                logon_type,
                logon_id,
            })
        }
        "4634" => {
            let logon_id = extract_data_value(xml, "TargetLogonId")
                .unwrap_or_default();
            Some(WinEvent::Logoff { logon_id })
        }
        _ => None,
    }
}

/// Extract a value from `<Tag>value</Tag>` in XML.
fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

/// Extract a value from `<Data Name="name">value</Data>` in event XML.
fn extract_data_value(xml: &str, name: &str) -> Option<String> {
    let pattern = format!("Name=\"{}\"", name);
    let pos = xml.find(&pattern)?;
    let after = &xml[pos..];
    let gt = after.find('>')? + 1;
    let lt = after[gt..].find('<')? + gt;
    let val = after[gt..lt].trim();
    if val.is_empty() || val == "-" {
        None
    } else {
        Some(val.to_string())
    }
}
