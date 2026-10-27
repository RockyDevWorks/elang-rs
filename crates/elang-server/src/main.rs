//! `elang-server` binary entry point.
//!
//! Reads configuration from environment variables and starts the HTTP service
//! on `0.0.0.0:${ELANG_SERVER_PORT}` (default `8080`). All runtime knobs are
//! env-var driven so the same binary works for local dev, Docker, and Fly.io
//! without per-target rebuilds.

use std::env;

use elang_server::{
    router, state::AppState, DEFAULT_MAX_BODY_BYTES, DEFAULT_TIMEOUT_MS, MAX_BODY_BYTES_CEILING,
};

/// Bind address default if `ELANG_SERVER_PORT` is unset or invalid.
const DEFAULT_PORT: u16 = 8080;

#[tokio::main]
async fn main() {
    let port: u16 = env::var("ELANG_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let max_body_bytes: usize = env::var("ELANG_SERVER_MAX_BODY_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_BYTES)
        .clamp(1, MAX_BODY_BYTES_CEILING);

    let timeout_ms: u64 = env::var("ELANG_SERVER_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    let state = AppState::new(timeout_ms);
    let app = router(state, max_body_bytes);
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("elang-server: failed to bind 0.0.0.0:{port}: {e}");
            std::process::exit(1);
        }
    };

    eprintln!(
        "elang-server listening on 0.0.0.0:{port} \
         (max body {max_body_bytes} bytes, timeout {timeout_ms} ms)"
    );

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("elang-server: serve error: {e}");
        std::process::exit(1);
    }
}
