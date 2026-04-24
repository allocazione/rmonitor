//! System metrics provider using the `sysinfo` crate.
//!
//! Wraps a `sysinfo::System` in a `std::sync::Mutex` because `System`
//! is not `Send` across `.await` points in all configurations.

use async_trait::async_trait;
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};

use crate::providers::MetricProvider;
use crate::store::Store;

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
                .with_memory(MemoryRefreshKind::everything()),
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
        // for accurate CPU usage. We sleep inside a blocking task to avoid
        // starving the tokio runtime.
        let usages = tokio::task::spawn_blocking({
            let sys_mutex = &self.sys;
            let ptr = sys_mutex as *const Mutex<System>;
            let ptr_val = ptr as usize;

            move || {
                // SAFETY: ptr_val points to a field of SysInfoMetrics which lives
                // for the duration of the program. The Mutex ensures exclusive access.
                let sys_mutex = unsafe { &*(ptr_val as *const Mutex<System>) };
                let mut sys = sys_mutex.lock().unwrap_or_else(|e| e.into_inner());
                sys.refresh_cpu_all();
                sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect::<Vec<f64>>()
            }
        })
        .await
        .unwrap_or_default();

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
}
