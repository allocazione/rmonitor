//! Thread-safe wrapper around `AppState`.
//!
//! `Store` provides ergonomic access to the shared application state
//! via `Arc<tokio::sync::RwLock<AppState>>`. Background tasks use
//! `.write().await` while the UI thread uses non-blocking `.try_snapshot()`.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::AppState;

/// Thread-safe state store wrapping `Arc<RwLock<AppState>>`.
///
/// Cheap to clone (just an `Arc::clone`).
#[derive(Debug, Clone)]
pub struct Store {
    inner: Arc<RwLock<AppState>>,
}

impl Store {
    /// Create a new `Store` wrapping the given initial state.
    pub fn new(state: AppState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }

    /// Acquire an async write lock (for background tasks).
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, AppState> {
        self.inner.write().await
    }

    /// Acquire an async read lock (for background tasks that only read).
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AppState> {
        self.inner.read().await
    }

    /// Take a full clone of the current state.
    /// This is slightly more expensive but guarantees the UI
    /// holds a consistent snapshot with no lock contention.
    pub async fn snapshot(&self) -> AppState {
        self.inner.read().await.clone()
    }

    /// Non-blocking snapshot: returns the last known state or a clone
    /// if the lock can be acquired without waiting.
    pub fn try_snapshot(&self) -> Option<AppState> {
        self.inner.try_read().ok().map(|guard| guard.clone())
    }
}
