//! Shared application state for the [`router`](crate::router).
//!
//! Two runtime knobs:
//! - the per-request handler timeout, and
//! - the bounded-concurrency guard for in-flight SDK jobs.
//!
//! The state is a plain `Clone` struct — axum's `WithState` extractor
//! requires `Clone`.

use std::sync::Arc;

use tokio::sync::Semaphore;

/// Application-wide state injected into every Axum handler.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Per-request wall-clock budget. Handlers wrap the SDK call in
    /// `tokio::time::timeout` and map overrun to `504 GATEWAY_TIMEOUT` (the
    /// Hub maps that to `SdkErrorCode::timeout`).
    timeout_ms: u64,
    /// Bounded-concurrency guard for in-flight SDK jobs. An extractor
    /// acquires a permit before request body buffering; the permit moves into
    /// the closure and is released only when the SDK work completes —
    /// even if the outer timeout fired and the response went back as
    /// `504`. Without this guard, a client that repeatedly hits the
    /// timeout could pin every blocking thread (`tokio`'s default
    /// maximum is 512) and exhaust the VM.
    permits: Arc<Semaphore>,
}

impl AppState {
    /// Build a new state with the given per-request timeout in milliseconds
    /// and the supplied concurrency limit.
    #[must_use]
    pub fn with_concurrency(timeout_ms: u64, max_concurrent: usize) -> Self {
        // Saturate to 1 so a malformed env var never deadlocks the semaphore.
        let permits = max_concurrent.max(1);
        Self {
            timeout_ms,
            permits: Arc::new(Semaphore::new(permits)),
        }
    }

    /// Per-request timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Clone of the inner `Arc<Semaphore>` for a handler to acquire a permit.
    #[must_use]
    pub fn permits(&self) -> Arc<Semaphore> {
        Arc::clone(&self.permits)
    }
}
