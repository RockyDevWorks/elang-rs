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

pub mod error;
pub mod handlers;
pub mod response;
pub mod state;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::limit::RequestBodyLimitLayer;

pub use error::ServerError;
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
/// Above this, even a fully-validated package would not fit in the
/// `MAX_UNCOMPRESSED_AGGREGATE_BYTES` ceiling enforced by the reader, so
/// accepting larger requests would just defer the failure.
pub const MAX_BODY_BYTES_CEILING: usize = 256 * 1024 * 1024;

/// Default per-request handler timeout (30 s).
///
/// Returns `504 GATEWAY_TIMEOUT` on overrun; the Hub maps that to the
/// `timeout` SDK code. Override with `ELANG_SERVER_TIMEOUT_MS`.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

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
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .with_state(state)
}
