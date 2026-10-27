//! End-to-end contract tests for the `elang-server` HTTP surface.
//!
//! Drives the real [`router`](elang_server::router) via
//! [`tower::ServiceExt::oneshot`] and asserts the responses match the
//! wire contract that `RockyDevWorks/elang-hub`'s
//! [`SdkClient`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/client.ts)
//! expects. Every assertion here is a guard against a silent shape drift
//! between this crate and the Hub.
//!
//! # Fixtures
//!
//! Tests reuse the committed `examples/hello-en/hello-en.elang` reference
//! archive. It passes `inspect`, `verify`, and `validate` end-to-end and is
//! the smallest known-valid package in the repo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "integration tests for the HTTP contract; panics on regression"
)]

use std::path::PathBuf;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use elang_server::{
    router, state::AppState, DEFAULT_MAX_BODY_BYTES, DEFAULT_MAX_CONCURRENT_REQUESTS,
};
use tower::ServiceExt;

/// Repo-root path resolved from this crate's manifest.
fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

/// Load the committed `hello-en` example archive as bytes.
fn fixture_archive() -> Vec<u8> {
    let path = repo_root().join("examples/hello-en/hello-en.elang");
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

/// Build a router with a generous body cap and 30 s timeout.
fn build_app() -> axum::Router {
    router(
        AppState::with_concurrency(30_000, DEFAULT_MAX_CONCURRENT_REQUESTS),
        DEFAULT_MAX_BODY_BYTES,
    )
}

/// Build a router with a deliberately tiny body cap for size-limit tests.
fn build_app_tiny_limit(max_bytes: usize) -> axum::Router {
    router(
        AppState::with_concurrency(30_000, DEFAULT_MAX_CONCURRENT_REQUESTS),
        max_bytes,
    )
}

/// Send a `POST` against the supplied router with raw octet-stream bytes.
async fn post_octet_stream(
    app: axum::Router,
    path: &str,
    bytes: Vec<u8>,
) -> (StatusCode, axum::http::HeaderMap, axum::body::Bytes) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes))
        .expect("build request");
    let response = app.oneshot(req).await.expect("router responded");
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("read response body");
    (parts.status, parts.headers, body)
}

#[tokio::test]
async fn healthz_returns_200() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("build healthz request"),
        )
        .await
        .expect("healthz responded");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn inspect_returns_discovered_resources_for_valid_archive() {
    let app = build_app();
    let bytes = fixture_archive();
    let (status, _headers, body) = post_octet_stream(app, "/inspect", bytes).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
    assert_eq!(json["ok"], serde_json::Value::Bool(true));
    assert_eq!(json["mode"], "inspect");
    assert_eq!(json["specVersion"], "0.1.0");
    assert_eq!(json["binaryContainerVersion"], "0.1.0");
    // The hello-en fixture ships exactly five resources.
    assert_eq!(json["discoveredResources"], 5);
    assert!(json["diagnostics"].is_array());
}

#[tokio::test]
async fn verify_returns_verified_resources_for_valid_archive() {
    let app = build_app();
    let bytes = fixture_archive();
    let (status, _headers, body) = post_octet_stream(app, "/verify", bytes).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
    assert_eq!(json["ok"], serde_json::Value::Bool(true));
    assert_eq!(json["mode"], "verify");
    assert_eq!(json["specVersion"], "0.1.0");
    assert_eq!(json["binaryContainerVersion"], "0.1.0");
    assert_eq!(json["verifiedResources"], 5);
}

#[tokio::test]
async fn validate_returns_phoneme_resource_path_and_inventory_for_valid_archive() {
    let app = build_app();
    let bytes = fixture_archive();
    let (status, _headers, body) = post_octet_stream(app, "/validate", bytes).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
    assert_eq!(json["ok"], serde_json::Value::Bool(true));
    assert_eq!(json["mode"], "validate");
    assert_eq!(json["specVersion"], "0.1.0");
    assert_eq!(json["binaryContainerVersion"], "0.1.0");
    assert_eq!(json["phonemeResourcePath"], "Linguistics/phonemes.eld");
    let inventory = &json["inventory"];
    assert!(inventory.is_object(), "inventory must be a JSON object");
    assert_eq!(inventory["specVersion"], "0.1.0");
    assert!(inventory["language"].is_object());
    assert!(inventory["language"]["identifier"].is_string());
    assert_eq!(inventory["phonemeResourcePath"], "Linguistics/phonemes.eld");
    assert!(inventory["canonical"].is_string());
    assert!(inventory["groupCount"].as_u64().expect("groupCount number") >= 1);
}

#[tokio::test]
async fn inspect_returns_200_with_ok_false_for_malformed_archive() {
    let app = build_app();
    let bytes = vec![0x00u8; 16]; // Not a valid ELANG archive.
    let (status, _headers, body) = post_octet_stream(app, "/inspect", bytes).await;
    // The Hub's SdkClient short-circuits on 4xx/5xx; SDK-content failures
    // must therefore come back with `200 OK` + `ok: false` so the Hub's
    // `parseSdkResult` path picks them up.
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
    assert_eq!(json["ok"], serde_json::Value::Bool(false));
    let code = json["code"].as_str().expect("string code");
    assert!(
        !code.is_empty(),
        "failure responses must carry a stable SdkErrorCode"
    );
    assert!(json["message"].is_string());
    assert!(json["diagnostics"].is_array());
}

#[tokio::test]
async fn wrong_content_type_returns_415() {
    let app = build_app();
    let bytes = fixture_archive();
    let req = Request::builder()
        .method("POST")
        .uri("/inspect")
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .expect("build request");
    let response = app.oneshot(req).await.expect("router responded");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn body_size_cap_returns_413() {
    // Cap of 16 bytes; send a 32-byte body.
    let app = build_app_tiny_limit(16);
    let bytes = vec![0u8; 32];
    let (status, _headers, _body) = post_octet_stream(app, "/inspect", bytes).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn saturated_sdk_routes_return_503_and_recover() {
    let state = AppState::with_concurrency(30_000, 1);
    let permit = state
        .permits()
        .try_acquire_owned()
        .expect("reserve capacity");
    let app = router(state, DEFAULT_MAX_BODY_BYTES);

    for path in ["/inspect", "/verify", "/validate"] {
        let (status, _, body) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            post_octet_stream(app.clone(), path, fixture_archive()),
        )
        .await
        .expect("saturated route rejects without waiting for capacity");
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(json["ok"], false);
        assert_eq!(json["code"], "service_unavailable");
    }

    drop(permit);
    let (status, _, _) = post_octet_stream(app, "/validate", fixture_archive()).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = build_app();
    let req = Request::builder()
        .uri("/no-such-route")
        .body(Body::empty())
        .expect("build request");
    let response = app.oneshot(req).await.expect("router responded");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
