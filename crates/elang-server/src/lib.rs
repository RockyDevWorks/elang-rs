//! # elang-server
//!
//! HTTP service exposing the ELANG SDK over `POST /{inspect,verify,validate}`.
//! Implements the wire contract from `RockyDevWorks/elang-hub`
//! `src/lib/sdk/contract.ts` so the Hub's [`SdkClient`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/client.ts)
//! can validate packages by POSTing raw archive bytes.
//!
//! ## Endpoints
//!
//! - `GET /healthz` — liveness probe. Not part of the SDK contract; returns a
//!   static `200 OK` so a Hub version mismatch can never mask a service outage.
//! - `POST /inspect` — runs `Package::open` (stages 1–7), returns the listing
//!   without integrity verification.
//! - `POST /verify` — runs `Package::open` + `verify` (stages 1–11), checks
//!   the SHA-256 of every indexed resource against the integrity manifest.
//! - `POST /validate` — runs `Package::open` + `validate` (stages 1–12),
//!   full ELD semantic validation.
//!
//! All SDK endpoints expect `Content-Type: application/octet-stream` and
//! return `application/json`. The Hub's SDK contract is the source of truth
//! for the response shape.
//!
//! ## Wire contract
//!
//! See [`response`] for the typed response shapes and [`error::ServerError`]
//! for the failure mode. Success and failure share the top-level `ok: bool`
//! discriminator.
//!
//! ## Why a separate crate
//!
//! `elang-core` is intentionally dependency-light (no `serde`); adding an
//! HTTP framework there would force every consumer to link axum/hyper. The
//! service is a thin wrapper that adds `serde` + `axum` only at the leaf
//! boundary where JSON is needed.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod handlers;
mod limits;
pub mod response;
pub mod state;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tower_http::limit::RequestBodyLimitLayer;

pub use error::ServerError;
pub use limits::MAX_DECODED_BYTES;
pub use response::{
    SdkDiagnostic, SdkFailure, SdkInspectSuccess, SdkValidateSuccess, SdkVerifySuccess,
};

/// Default request body size cap (10 MiB).
///
/// Matches the Hub's [`MAX_FILE_BYTES`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/bounds.ts)
/// default. Override with the `ELANG_SERVER_MAX_BODY_BYTES` env var; values
/// outside `[1, 256 MiB]` are clamped to that range to keep the limit sane.
pub const DEFAULT_MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Hard upper bound on the body-size cap (256 MiB).
///
/// Deployments that raise the body cap must also provision memory for
/// buffered requests, archive copies and concurrent SDK jobs.
pub const MAX_BODY_BYTES_CEILING: usize = 256 * 1024 * 1024;

/// Default per-request handler timeout (30 s).
///
/// Returns `504 GATEWAY_TIMEOUT` on overrun; the Hub maps that to the
/// `timeout` SDK code. Override with `ELANG_SERVER_TIMEOUT_MS`.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Default max concurrent in-flight SDK requests (4).
///
/// Caps the number of `Package::open` / `verify` / `validate` jobs that
/// can run at once. A `Semaphore` permit moves into the closure and is
/// released only when the work ends — so a handler timeout cannot orphan
/// the underlying blocking job past the permit limit. Provision memory
/// for each job's decoded bytes, decompressor window and parser state,
/// as well as buffered request bodies and archive copies. The Fly.io
/// configuration uses one job on a 1 GB VM. Raising body size or concurrency
/// requires reviewing available memory. Override with
/// `ELANG_SERVER_MAX_CONCURRENT`.
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 4;

/// Build the Axum router with all routes wired and the body-size cap
/// applied as a layer.
///
/// The router is `Clone`; clone it freely per-connection. State is `Arc`-sharable
/// via [`state::AppState`] so the same router can serve any number of
/// concurrent requests.
pub fn router(state: state::AppState, max_body_bytes: usize) -> Router {
    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/inspect", post(handlers::inspect))
        .route("/verify", post(handlers::verify))
        .route("/validate", post(handlers::validate))
        // Axum's `Bytes` extractor applies a 2 MB default cap that shadows
        // the configured body cap. Disable it so `RequestBodyLimitLayer`
        // is the sole gate. See
        // <https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html>.
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .with_state(state)
}

/// Bind to `0.0.0.0:config.port` and serve forever.
///
/// Equivalent to [`serve_with_shutdown`] with `shutdown =
/// std::future::pending()` — the server runs until the runtime is
/// dropped. Use [`serve_with_shutdown`] from tests (or from `main` if you
/// ever want SIGTERM-aware behaviour) so the success path is reachable.
pub async fn serve(config: crate::config::Config) -> Result<(), ServeError> {
    serve_with_shutdown(config, std::future::pending()).await
}

/// Bind to `0.0.0.0:config.port` and serve until `shutdown` resolves.
///
/// Returns:
/// - `Ok(())` after a graceful shutdown triggered by the supplied future.
/// - `Err(ServeError::Bind { .. })` if the socket cannot be bound.
/// - `Err(ServeError::Axum { .. })` if axum's serve loop fails.
///
/// The shutdown future is `Send + 'static` because axum spawns the
/// listener-accept loop on its own task; it must outlive the bind.
pub async fn serve_with_shutdown<F>(
    config: crate::config::Config,
    shutdown: F,
) -> Result<(), ServeError>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let state = crate::config::build_state(&config);
    let app = router(state, config.max_body_bytes);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .map_err(|e| ServeError::Bind {
            port: config.port,
            source: e,
        })?;

    eprintln!(
        "elang-server listening on 0.0.0.0:{} \
         (max body {} bytes, timeout {} ms, max concurrent {})",
        config.port, config.max_body_bytes, config.timeout_ms, config.max_concurrent
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(ServeError::Axum)?;

    Ok(())
}

/// Failure modes for [`serve`].
#[derive(Debug)]
pub enum ServeError {
    /// TCP bind on `port` failed (port in use, permission denied, etc.).
    Bind {
        /// Port that could not be bound.
        port: u16,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// axum's serve loop returned an error.
    Axum(
        /// Underlying axum error.
        std::io::Error,
    ),
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "unit tests for the server entry point; panics on regression"
)]
mod tests {
    use super::*;
    use crate::config::{Config, DEFAULT_PORT};

    #[tokio::test]
    async fn serve_with_shutdown_returns_ok_when_signal_fires() {
        // Pre-build a shutdown future that resolves on a oneshot. Spawning
        // `serve_with_shutdown` lets it bind to an ephemeral port; we then
        // fire the signal and confirm the success path returns `Ok(())`
        // — the one line `run_with_timeout`'s existing tests cannot hit.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let cfg = Config {
            port: 0,
            max_body_bytes: 1024,
            timeout_ms: 30_000,
            max_concurrent: 1,
        };
        let handle = tokio::spawn(serve_with_shutdown(cfg, async move {
            let _ = rx.await;
        }));
        // Yield so the bind lands before we fire the signal.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tx.send(()).expect("shutdown signal sent");
        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("serve task did not hang")
            .expect("serve task joined")
            .expect("Ok(()) on graceful shutdown");
    }

    #[tokio::test]
    async fn serve_returns_bind_error_when_port_already_taken() {
        // Reserve an ephemeral port, then ask `serve` to bind to it —
        // the second bind must fail and the function must return
        // `ServeError::Bind` carrying the offending port.
        let blocker = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("ephemeral bind");
        let port = blocker.local_addr().expect("local_addr").port();

        let cfg = Config {
            port,
            max_body_bytes: 1024,
            timeout_ms: 30_000,
            max_concurrent: 1,
        };

        let err = serve(cfg).await.expect_err("second bind must fail");
        match err {
            ServeError::Bind {
                port: returned_port,
                source,
            } => {
                assert_eq!(returned_port, port);
                assert_eq!(source.kind(), std::io::ErrorKind::AddrInUse);
            }
            other => panic!("expected ServeError::Bind, got {other:?}"),
        }
    }

    #[test]
    fn default_port_constant() {
        assert_eq!(DEFAULT_PORT, 8080);
    }
}
