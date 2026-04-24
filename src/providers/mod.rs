//! Provider trait definitions and platform re-exports.
//!
//! The two core traits — `MetricProvider` and `ConnectionProvider` — define
//! the async interface between background data-gathering tasks and the store.

use async_trait::async_trait;
use crate::core::store::Store;

// ---------------------------------------------------------------------------
// MetricProvider — system metrics (CPU, memory, disk, network)
// ---------------------------------------------------------------------------

/// Trait for system metric collection.
///
/// Implementations must be `Send + Sync` so they can be moved into
/// `tokio::spawn` tasks.
#[async_trait]
pub trait MetricProvider: Send + Sync {
    /// Refresh CPU usage per core and write results to the store.
    async fn refresh_cpu(&self, store: &Store);

    /// Refresh memory usage and push a sample to the history ring buffer.
    async fn refresh_memory(&self, store: &Store);

    /// Refresh disk usage totals.
    async fn refresh_disk(&self, store: &Store);

    /// Refresh network inbound/outbound traffic.
    async fn refresh_network(&self, store: &Store);

    /// Refresh running processes (top by CPU).
    async fn refresh_processes(&self, store: &Store);

    /// Refresh all metrics in one go to ensure consistent timing.
    async fn refresh_all(&self, store: &Store) {
        self.refresh_cpu(store).await;
        self.refresh_memory(store).await;
        self.refresh_network(store).await;
        self.refresh_processes(store).await;
    }
}

// ---------------------------------------------------------------------------
// ConnectionProvider — SSH / RDP session monitoring
// ---------------------------------------------------------------------------

/// Trait for security connection monitoring.
///
/// `watch_connections` is a long-running method that tails logs or subscribes
/// to OS events indefinitely, updating the store on each new login/logoff.
#[async_trait]
pub trait ConnectionProvider: Send + Sync {
    /// Start watching for connection events. Runs indefinitely.
    async fn watch_connections(&self, store: &Store);
}
