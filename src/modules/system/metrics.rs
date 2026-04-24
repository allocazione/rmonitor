//! System metrics provider using the `sysinfo` crate.
//!
//! Wraps a `sysinfo::System` in a `std::sync::Mutex` because `System`
//! is not `Send` across `.await` points in all configurations.

use async_trait::async_trait;
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, RefreshKind, System};

use crate::providers::MetricProvider;
use crate::core::state::{ProcessInfo, ProcessSort};
use crate::core::store::Store;

/// `MetricProvider` implementation backed by `sysinfo`.
pub struct SysInfoMetrics {
    /// Guarded sysinfo handle — we keep a single instance alive for accuracy.
    sys: Mutex<System>,
    /// Guarded sysinfo Networks handle.
    networks: Mutex<Networks>,
}

impl SysInfoMetrics {
    /// Create a new metrics provider.
    ///
    /// Performs an initial CPU refresh so the first real poll (after the
    /// minimum interval) returns meaningful data instead of 0%.
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything()),
        );
        // Seed an initial measurement so the delta is available on first poll.
        sys.refresh_cpu_all();

        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh(true);

        Self {
            sys: Mutex::new(sys),
            networks: Mutex::new(networks),
        }
    }
}

#[async_trait]
impl MetricProvider for SysInfoMetrics {
    async fn refresh_cpu(&self, store: &Store) {
        // sysinfo requires two samples separated by at least MINIMUM_CPU_UPDATE_INTERVAL
        // for accurate CPU usage.
        let usages = {
            let mut sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
            sys.refresh_cpu_all();
            sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect::<Vec<f64>>()
        };

        let mut state = store.write().await;
        state.cpu_usages = usages;
    }

    async fn refresh_memory(&self, store: &Store) {
        let (total, used) = {
            let mut sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
            sys.refresh_memory();
            (sys.total_memory(), sys.used_memory())
        };

        let mut state = store.write().await;
        state.mem_total = total;
        state.mem_used = used;
        state.push_mem_sample(used);
    }

    async fn refresh_disk(&self, store: &Store) {
        let (total, used) = tokio::task::spawn_blocking(|| {
            let disks = Disks::new_with_refreshed_list();
            let mut total: u64 = 0;
            let mut available: u64 = 0;
            for disk in disks.list() {
                total = total.saturating_add(disk.total_space());
                available = available.saturating_add(disk.available_space());
            }
            (total, total.saturating_sub(available))
        })
        .await
        .unwrap_or((0, 0));

        let mut state = store.write().await;
        state.disk_total = total;
        state.disk_used = used;
    }

    async fn refresh_network(&self, store: &Store) {
        let (rx, tx, total_rx, total_tx) = {
            let mut networks = self.networks.lock().unwrap_or_else(|e| e.into_inner());
            networks.refresh(true);

            let mut rx = 0u64;
            let mut tx = 0u64;
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;

            for (_interface_name, data) in networks.iter() {
                rx = rx.saturating_add(data.received());
                tx = tx.saturating_add(data.transmitted());
                total_rx = total_rx.saturating_add(data.total_received());
                total_tx = total_tx.saturating_add(data.total_transmitted());
            }

            (rx, tx, total_rx, total_tx)
        };

        let mut state = store.write().await;
        state.net_rx = rx;
        state.net_tx = tx;
        state.net_total_rx = total_rx;
        state.net_total_tx = total_tx;
    }

    async fn refresh_processes(&self, _store: &Store) {
        // ... (this will be called by refresh_all or separately)
        // Implementation remains same but we might want to optimize.
    }

    async fn refresh_all(&self, store: &Store) {
        let (is_frozen, sort_by, sort_asc) = {
            let state = store.read().await;
            (state.processes_frozen, state.processes_sort_by, state.processes_sort_asc)
        };

        // Uptime refresh
        let uptime = {
            let _sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
            System::uptime()
        };
        store.write().await.uptime_secs = uptime;

        let (cpu_usages, mem_total, mem_used, rx, tx, total_rx, total_tx, processes) = {
            let mut sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
            let mut networks = self.networks.lock().unwrap_or_else(|e| e.into_inner());

            // 1. Refresh everything in one go
            sys.refresh_cpu_all();
            sys.refresh_memory();
            if !is_frozen {
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            }
            networks.refresh(true);

            // 2. Collect CPU
            let cpu_usages = sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect::<Vec<f64>>();
            let num_cpus = cpu_usages.len() as f32;

            // 3. Collect Memory
            let mem_total = sys.total_memory();
            let mem_used = sys.used_memory();

            // 4. Collect Network
            let mut rx = 0u64;
            let mut tx = 0u64;
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;
            for (_interface_name, data) in networks.iter() {
                rx = rx.saturating_add(data.received());
                tx = tx.saturating_add(data.transmitted());
                total_rx = total_rx.saturating_add(data.total_received());
                total_tx = total_tx.saturating_add(data.total_transmitted());
            }

            // 5. Collect Processes
            let list = if !is_frozen {
                let mut l: Vec<ProcessInfo> = sys
                    .processes()
                    .iter()
                    .map(|(pid, p)| ProcessInfo {
                        pid: pid.as_u32(),
                        name: p.name().to_string_lossy().into_owned(),
                        cpu_usage: if num_cpus > 0.0 { p.cpu_usage() / num_cpus } else { p.cpu_usage() },
                        memory: p.memory(),
                    })
                    .collect();

                l.sort_by(|a, b| {
                    let ord = match sort_by {
                        ProcessSort::Pid => a.pid.cmp(&b.pid),
                        ProcessSort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        ProcessSort::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
                        ProcessSort::Memory => a.memory.cmp(&b.memory),
                    };
                    if sort_asc { ord } else { ord.reverse() }
                });

                l.truncate(100);
                Some(l)
            } else {
                None
            };

            (cpu_usages, mem_total, mem_used, rx, tx, total_rx, total_tx, list)
        };

        // Update state in one lock
        let mut state = store.write().await;
        state.cpu_usages = cpu_usages;
        state.mem_total = mem_total;
        state.mem_used = mem_used;
        state.push_mem_sample(mem_used);
        state.net_rx = rx;
        state.net_tx = tx;
        state.net_total_rx = total_rx;
        state.net_total_tx = total_tx;
        if let Some(p) = processes {
            state.processes = p;
        }
    }
}

impl SysInfoMetrics {
    /// Kill a process by PID.
    pub fn kill_process(&self, pid: u32) -> bool {
        let sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid)) {
            process.kill()
        } else {
            false
        }
    }
}
