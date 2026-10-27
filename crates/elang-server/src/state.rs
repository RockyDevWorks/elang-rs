//! Shared application state for the [`router`](crate::router).
//!
//! Today the only runtime knob is the per-request handler timeout (passed
//! through to `tokio::time::timeout` from each SDK handler). The state is a
//! plain `Clone` struct — axum's `WithState` extractor requires `Clone`.

/// Application-wide state injected into every Axum handler.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Per-request wall-clock budget. Handlers wrap the SDK call in
    /// `tokio::time::timeout` and map overrun to `504 GATEWAY_TIMEOUT` (the
    /// Hub maps that to `SdkErrorCode::timeout`).
    timeout_ms: u64,
}

impl AppState {
    /// Build a new state with the given per-request timeout in milliseconds.
    #[must_use]
    pub const fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    /// Per-request timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}
