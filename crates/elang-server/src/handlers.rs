//! HTTP handlers for the SDK's three modes plus the `healthz` probe.
//!
//! All three SDK handlers share the same shape:
//!
//! 1. Acquire a [`tokio::sync::Semaphore`] permit from [`AppState`] before
//!    buffering the body. Return `503` immediately if full.
//! 2. Extract `Bytes` and validate `Content-Type: application/octet-stream`.
//! 3. Move the body bytes (as `Bytes`, no extra copy) and permit into a
//!    [`tokio::task::spawn_blocking`] closure that calls the matching
//!    [`elang_reader::Package`] method.
//! 4. Use [`tokio::time::timeout`] to limit the response wait. Blocking
//!    work can continue after a timeout, retaining its permit until done.
//! 5. Map the SDK's [`elang_core::error::Error`] to a Hub-shaped
//!    [`crate::response::SdkFailure`] via [`crate::error::ServerError`].
//!
//! CPU-bound work (binary container parsing, SHA-256 verification, ELD
//! semantic validation) must not run on a tokio worker thread — that would
//! stall every concurrent request on the same worker. The `spawn_blocking`
//! hop moves it to tokio's blocking thread pool, which by default can grow
//! up to 512 threads; the `Semaphore` permit acquired in step 1 caps the
//! number of concurrent jobs to the configured limit so the pool cannot be
//! exhausted by a client that repeatedly triggers timeouts.

use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use elang_reader::{Inspected, Package, Validated, Verified};
use tokio::sync::OwnedSemaphorePermit;

use crate::{
    error::{internal, ServerError},
    limits::open_for_decode,
    response::{SdkInspectSuccess, SdkValidateSuccess, SdkVerifySuccess},
    state::AppState,
};

/// Content-Type expected on every SDK endpoint.
const REQUIRED_CONTENT_TYPE: &str = "application/octet-stream";

/// Admission permit extracted before Axum buffers the SDK request body.
///
/// The permit is released if a later extractor rejects the request, or when
/// the blocking SDK job completes, including after a response timeout.
#[derive(Debug)]
pub struct SdkPermit(OwnedSemaphorePermit);

impl SdkPermit {
    fn acquire(state: &AppState) -> Result<Self, ServerError> {
        state
            .permits()
            .try_acquire_owned()
            .map(Self)
            .map_err(|_| internal("SDK concurrency limit reached or semaphore closed"))
    }
}

impl FromRequestParts<AppState> for SdkPermit {
    type Rejection = ServerError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Self::acquire(state)
    }
}

/// Liveness probe. Not part of the SDK contract.
///
/// Returns `200 OK` with a static body so a Hub version mismatch can never
/// mask a service outage from a Fly.io health check.
pub async fn healthz() -> Response {
    (StatusCode::OK, "ok\n").into_response()
}

/// Run the supplied blocking work on tokio's blocking pool, with a
/// per-request wall-clock timeout from [`AppState::timeout_ms`] and a
/// previously acquired [`SdkPermit`].
///
/// Admission has already rejected saturated requests before body buffering.
/// Move the permit into the blocking closure so it is released only when
/// the SDK work finishes, including after a timeout. A request
/// that times out therefore leaves its orphaned job counted against the
/// concurrency cap, which is exactly what prevents an attacker from
/// pinning every blocking-pool thread by repeatedly triggering timeouts.
async fn run_with_timeout<F, T>(
    state: &AppState,
    permit: SdkPermit,
    work: F,
) -> Result<T, ServerError>
where
    F: FnOnce() -> Result<T, ServerError> + Send + 'static,
    T: Send + 'static,
{
    let budget = Duration::from_millis(state.timeout_ms());

    let outcome = tokio::time::timeout(budget, async move {
        let joined = tokio::task::spawn_blocking(move || {
            // Hold the permit for the entire blocking job.
            let _permit = permit.0;
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
    permit: SdkPermit,
    body: Bytes,
) -> Result<Json<SdkInspectSuccess>, ServerError> {
    validate_content_type(&headers)?;
    let inspected: Inspected = run_with_timeout(&state, permit, move || {
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
    permit: SdkPermit,
    body: Bytes,
) -> Result<Json<SdkVerifySuccess>, ServerError> {
    validate_content_type(&headers)?;
    let verified: Verified = run_with_timeout(&state, permit, move || {
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
    permit: SdkPermit,
    body: Bytes,
) -> Result<Json<SdkValidateSuccess>, ServerError> {
    validate_content_type(&headers)?;
    let validated: Validated = run_with_timeout(&state, permit, move || {
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

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "tests fail on unexpected results"
)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::mpsc,
        task::{Context, Poll, Waker},
    };

    use super::*;
    use crate::MAX_DECODED_BYTES;

    /// Header-valid frames with synthetic content-size claims. No large
    /// buffers are allocated, and the empty frame bodies cannot satisfy the
    /// claims, so a 413 proves rejection happened before decoding.
    fn archive_with_frame_sizes(sizes: &[(u64, u64)]) -> Vec<u8> {
        use elang_container::{Footer, Header, Index, IndexEntry, FOOTER_LEN, HEADER_LEN};

        let mut bytes = vec![0u8; HEADER_LEN];
        Header::current().encode(&mut bytes, 0).expect("header");
        let mut entries = Vec::new();
        for (i, &(index_size, frame_size)) in sizes.iter().enumerate() {
            let offset = u64::try_from(bytes.len()).expect("offset");
            let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0xe4];
            frame.extend_from_slice(&frame_size.to_le_bytes());
            frame.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0]); // empty block + checksum
            entries.push(
                IndexEntry::new(
                    &format!("resource/{i:04}.bin"),
                    offset,
                    u64::try_from(frame.len()).expect("frame length"),
                    index_size,
                    1,
                )
                .expect("entry"),
            );
            bytes.extend_from_slice(&frame);
        }
        let index_offset = u64::try_from(bytes.len()).expect("index offset");
        let (index, digest) = Index::with_entries(entries)
            .expect("index")
            .encode_and_hash()
            .expect("index encoding");
        let footer = Footer::new(
            index_offset,
            u64::try_from(index.len()).expect("index length"),
            digest,
        )
        .expect("footer");
        bytes.extend_from_slice(&index);
        let footer_offset = bytes.len();
        bytes.resize(footer_offset.saturating_add(FOOTER_LEN), 0);
        footer
            .encode(&mut bytes, footer_offset)
            .expect("footer encoding");
        bytes
    }

    #[test]
    fn decoded_limit_accepts_the_exact_aggregate_boundary() {
        let half = MAX_DECODED_BYTES.checked_div(2).expect("half limit");
        let bytes = archive_with_frame_sizes(&[(half, half), (half, half)]);
        assert!(open_for_decode(&bytes).is_ok());
    }

    #[tokio::test]
    async fn decode_endpoints_reject_excessive_frame_sizes_before_decoding() {
        let half = MAX_DECODED_BYTES.checked_div(2).expect("half limit");
        let over = MAX_DECODED_BYTES.checked_add(1).expect("over limit");
        let half_over = half.checked_add(1).expect("over half limit");
        for sizes in [
            // One oversized resource.
            vec![(over, over)],
            // Aggregate overflow with individually acceptable frames.
            vec![(half, half), (half_over, half_over)],
            // Index understates frame size.
            vec![(1, over)],
        ] {
            let bytes = archive_with_frame_sizes(&sizes);
            assert!(bytes.len() < crate::DEFAULT_MAX_BODY_BYTES);
            // The SDK still accepts the headers under its format limits.
            assert!(Package::open(&bytes).is_ok());
            for path in ["/verify", "/validate"] {
                use axum::{body::Body, http::Request};
                use tower::ServiceExt;

                let app = crate::router(
                    AppState::with_concurrency(30_000, 1),
                    crate::DEFAULT_MAX_BODY_BYTES,
                );
                let response = app
                    .oneshot(
                        Request::post(path)
                            .header("content-type", REQUIRED_CONTENT_TYPE)
                            .body(Body::from(bytes.clone()))
                            .expect("request"),
                    )
                    .await
                    .expect("response");
                assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE, "{path}");
                let body = axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .expect("body");
                let json: serde_json::Value = serde_json::from_slice(&body).expect("JSON");
                assert_eq!(json.get("ok"), Some(&serde_json::Value::Bool(false)));
                assert_eq!(
                    json.get("code").and_then(serde_json::Value::as_str),
                    Some("request_too_large")
                );
            }
        }
    }

    async fn run_admitted_work<F, T>(state: &AppState, work: F) -> Result<T, ServerError>
    where
        F: FnOnce() -> Result<T, ServerError> + Send + 'static,
        T: Send + 'static,
    {
        let permit = SdkPermit::acquire(state)?;
        run_with_timeout(state, permit, work).await
    }

    #[test]
    fn saturated_work_is_rejected_on_first_poll() {
        let state = AppState::with_concurrency(30_000, 1);
        let _permit = state.permits().try_acquire_owned().expect("first permit");
        let mut work = pin!(run_admitted_work(&state, || Ok(())));
        let mut context = Context::from_waker(Waker::noop());

        assert!(matches!(
            work.as_mut().poll(&mut context),
            Poll::Ready(Err(ServerError::Internal))
        ));
    }

    #[tokio::test]
    async fn timed_out_work_holds_permit_until_completion() {
        let state = AppState::with_concurrency(20, 1);
        let worker_state = state.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = tokio::spawn(async move {
            run_admitted_work(&worker_state, move || {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("release blocking work");
                Ok(())
            })
            .await
        });

        started_rx.await.expect("blocking work started");
        assert!(matches!(
            worker.await.expect("handler joined"),
            Err(ServerError::Timeout)
        ));
        assert_eq!(state.permits().available_permits(), 0);
        assert!(matches!(
            run_admitted_work(&state, || Ok(())).await,
            Err(ServerError::Internal)
        ));

        release_tx.send(()).expect("unblock work");
        let permit = tokio::time::timeout(Duration::from_secs(5), state.permits().acquire_owned())
            .await
            .expect("work released permit")
            .expect("semaphore open");
        drop(permit);
        assert!(run_admitted_work(&state, || Ok(())).await.is_ok());
    }

    #[tokio::test]
    async fn panic_in_work_returns_internal_via_join_error() {
        // A panic inside the blocking closure is caught by
        // `spawn_blocking` and surfaced as a `JoinError`. `run_with_timeout`
        // must translate that into the `Internal` arm of the SDK
        // error contract — never panic out of the request path.
        let state = AppState::with_concurrency(30_000, 1);
        let permit = SdkPermit::acquire(&state).expect("permit acquired");
        let outcome = run_with_timeout(&state, permit, || -> Result<(), ServerError> {
            panic!("synthetic SDK panic")
        })
        .await;
        assert!(
            matches!(outcome, Err(ServerError::Internal)),
            "expected Internal from JoinError, got {outcome:?}"
        );
    }

    #[test]
    fn is_octet_stream_rejects_non_utf8_header_value() {
        // `HeaderValue::from_bytes` permits opaque bytes (0xFF is
        // outside the visible ASCII range but still valid per the
        // spec). The matcher must refuse such values rather than
        // panic on the `to_str()` failure.
        let value = axum::http::HeaderValue::from_bytes(&[0xFF]).expect("opaque byte");
        assert!(!is_octet_stream(&value));
    }
}
