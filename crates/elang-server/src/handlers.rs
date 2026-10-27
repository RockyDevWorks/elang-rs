//! HTTP handlers for the SDK's three modes plus the `healthz` probe.
//!
//! All three SDK handlers share the same shape:
//!
//! 1. Validate `Content-Type: application/octet-stream`.
//! 2. Acquire a [`tokio::sync::Semaphore`] permit from [`AppState`] so
//!    the blocking work cannot orphan past the configured concurrency
//!    cap, even after a handler timeout.
//! 3. Move the body bytes (as `Bytes`, no extra copy) into a
//!    [`tokio::task::spawn_blocking`] closure that calls the matching
//!    [`elang_reader::Package`] method.
//! 4. Wrap the work in [`tokio::time::timeout`] so a hung request cannot
//!    pin a worker indefinitely.
//! 5. Map the SDK's [`elang_core::error::Error`] to a Hub-shaped
//!    [`crate::response::SdkFailure`] via [`crate::error::ServerError`].
//!
//! CPU-bound work (binary container parsing, SHA-256 verification, ELD
//! semantic validation) must not run on a tokio worker thread — that would
//! stall every concurrent request on the same worker. The `spawn_blocking`
//! hop moves it to tokio's blocking thread pool, which by default can grow
//! up to 512 threads; the `Semaphore` permit acquired in step 2 caps the
//! number of concurrent jobs to the configured limit so the pool cannot be
//! exhausted by a client that repeatedly triggers timeouts.

use std::time::Duration;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use elang_reader::{Inspected, Package, Validated, Verified};

use crate::{
    error::{internal, ServerError},
    limits::open_for_decode,
    response::{SdkInspectSuccess, SdkValidateSuccess, SdkVerifySuccess},
    state::AppState,
};

/// Content-Type expected on every SDK endpoint.
const REQUIRED_CONTENT_TYPE: &str = "application/octet-stream";

/// Liveness probe. Not part of the SDK contract.
///
/// Returns `200 OK` with a static body so a Hub version mismatch can never
/// mask a service outage from a Fly.io health check.
pub async fn healthz() -> Response {
    (StatusCode::OK, "ok\n").into_response()
}

/// Run the supplied blocking work on tokio's blocking pool, with a
/// per-request wall-clock timeout from [`AppState::timeout_ms`] and a
/// bounded-concurrency permit from [`AppState::permits`].
///
/// The permit is acquired *inside* the timeout budget, so a saturated
/// server queues rather than immediately rejecting. The permit is then
/// moved into the blocking closure, which means it is released only when
/// the SDK work genuinely finishes — not when the timeout fires. A request
/// that times out therefore leaves its orphaned job counted against the
/// concurrency cap, which is exactly what prevents an attacker from
/// pinning every blocking-pool thread by repeatedly triggering timeouts.
async fn run_with_timeout<F, T>(state: &AppState, work: F) -> Result<T, ServerError>
where
    F: FnOnce() -> Result<T, ServerError> + Send + 'static,
    T: Send + 'static,
{
    let budget = Duration::from_millis(state.timeout_ms());
    let permits = state.permits();

    let outcome = tokio::time::timeout(budget, async move {
        let permit = permits
            .acquire_owned()
            .await
            .map_err(|_| internal("concurrency semaphore closed unexpectedly"))?;
        let joined = tokio::task::spawn_blocking(move || {
            // Hold the permit for the entire blocking job.
            let _permit = permit;
            work()
        })
        .await;
        joined.map_err(|je| internal(&format!("worker join failed: {je}")))
    })
    .await;

    match outcome {
        Err(_elapsed) => Err(ServerError::Timeout),
        Ok(Err(e)) => Err(e),
        Ok(Ok(r)) => r,
    }
}

/// Validate `Content-Type: application/octet-stream`. Other content types
/// are refused with `415`.
fn validate_content_type(headers: &HeaderMap) -> Result<(), ServerError> {
    match headers.get(axum::http::header::CONTENT_TYPE) {
        Some(value) if is_octet_stream(value) => Ok(()),
        _ => Err(ServerError::UnsupportedMediaType),
    }
}

/// Whether the `Content-Type` header value matches `application/octet-stream`,
/// ignoring parameters like `; charset=…`.
fn is_octet_stream(value: &HeaderValue) -> bool {
    let Ok(s) = value.to_str() else {
        return false;
    };
    let primary = s.split(';').next().unwrap_or("");
    primary.trim().eq_ignore_ascii_case(REQUIRED_CONTENT_TYPE)
}

/// Handler for `POST /inspect`. Runs `Package::open` (stages 1–7) and returns
/// the listing without integrity verification.
pub async fn inspect(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SdkInspectSuccess>, ServerError> {
    validate_content_type(&headers)?;
    let inspected: Inspected = run_with_timeout(&state, move || {
        let pkg = Package::open(&body).map_err(ServerError::from)?;
        Ok::<_, ServerError>(pkg.inspect())
    })
    .await?;

    Ok(Json(SdkInspectSuccess {
        ok: true,
        mode: "inspect",
        spec_version: elang_core::version_domains::SPEC_RELEASE,
        binary_container_version: elang_core::version_domains::CONTAINER_VERSION,
        discovered_resources: inspected.len(),
        diagnostics: Vec::new(),
    }))
}

/// Handler for `POST /verify`. Runs `Package::open` + `verify` (stages 1–11)
/// and reports how many resources had their SHA-256 verified.
pub async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SdkVerifySuccess>, ServerError> {
    validate_content_type(&headers)?;
    let verified: Verified = run_with_timeout(&state, move || {
        open_for_decode(&body)?.verify().map_err(ServerError::from)
    })
    .await?;

    Ok(Json(SdkVerifySuccess {
        ok: true,
        mode: "verify",
        spec_version: elang_core::version_domains::SPEC_RELEASE,
        binary_container_version: elang_core::version_domains::CONTAINER_VERSION,
        verified_resources: verified.resource_count(),
        diagnostics: Vec::new(),
    }))
}

/// Handler for `POST /validate`. Runs `Package::open` + `validate` (stages
/// 1–12) and serializes the validated inventory as the opaque
/// `inventory` JSON payload.
pub async fn validate(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SdkValidateSuccess>, ServerError> {
    validate_content_type(&headers)?;
    let validated: Validated = run_with_timeout(&state, move || {
        open_for_decode(&body)?
            .validate()
            .map_err(ServerError::from)
    })
    .await?;

    let phoneme_resource_path = validated.epf().manifest().phonemes().as_str().to_owned();

    let inventory = inventory_to_json(&validated);

    Ok(Json(SdkValidateSuccess {
        ok: true,
        mode: "validate",
        spec_version: elang_core::version_domains::SPEC_RELEASE,
        binary_container_version: elang_core::version_domains::CONTAINER_VERSION,
        phoneme_resource_path,
        inventory,
        diagnostics: Vec::new(),
    }))
}

/// Serialize the validated inventory as a JSON value. v0.1.0 of the Hub
/// treats this as `unknown`, but the structure is shaped so a future Hub
/// view-model can narrow into it without a wire-contract change.
fn inventory_to_json(validated: &Validated) -> serde_json::Value {
    let metadata = validated.metadata();
    let canonical_bytes = validated.canonical_phoneme_bytes();
    let canonical_hex = bytes_to_hex(&canonical_bytes);

    serde_json::json!({
        "specVersion": elang_core::version_domains::SPEC_RELEASE,
        "language": {
            "identifier": metadata.identifier().as_str(),
            "name": metadata.name(),
            "description": metadata.description(),
        },
        "phonemeResourcePath": validated.epf().manifest().phonemes().as_str(),
        "groupCount": validated.groups().len(),
        "rootCount": validated.root_count(),
        "canonical": canonical_hex,
    })
}

/// Hex-encode a byte slice without bringing in a new dependency.
///
/// Uses checked arithmetic for the capacity pre-allocation to satisfy
/// the workspace's `arithmetic_side_effects` lint.
fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        let high = HEX.get(usize::from(b >> 4)).copied().unwrap_or(b'0');
        let low = HEX.get(usize::from(b & 0x0f)).copied().unwrap_or(b'0');
        // Pushing two ASCII bytes is safe; any failure here would be an OOM.
        out.push(high as char);
        out.push(low as char);
    }
    out
}
