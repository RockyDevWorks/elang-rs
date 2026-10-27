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
async fn body_above_axum_default_limit_reaches_handler() {
    // Axum's `Bytes` extractor applies a 2 MB default cap that shadows the
    // configured body cap unless `DefaultBodyLimit::disable()` is layered on
    // the router. This posts a 3 MB body (above the 2 MB default, below the
    // 10 MB configured cap) and asserts the request is NOT rejected with
    // 413 — i.e. it reaches the handler. Without the fix this test fails
    // with `413 Failed to buffer the request body: length limit exceeded`.
    let app = build_app();
    let payload: Vec<u8> = vec![0xAB; 3 * 1024 * 1024];
    let (status, _headers, _body) = post_octet_stream(app, "/inspect", payload).await;
    assert_ne!(
        status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "body between Axum's 2 MB default and the configured 10 MB cap must not be 413",
    );
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

/// Build a structurally valid archive around supplied frames. Keeping the
/// manifest absent proves the service rejects expansion before SDK decoding.
fn archive_from_frames(frames: &[(&str, Vec<u8>, u64)]) -> Vec<u8> {
    use elang_container::{
        Footer, Header, Index, IndexEntry, COMPRESSION_ZSTD, FOOTER_LEN, HEADER_LEN,
    };

    let mut archive = vec![0; HEADER_LEN];
    Header::current().encode(&mut archive, 0).unwrap();
    let mut entries = Vec::new();
    for (path, frame, declared_size) in frames {
        entries.push(
            IndexEntry::new(
                path,
                archive.len() as u64,
                frame.len() as u64,
                *declared_size,
                COMPRESSION_ZSTD,
            )
            .unwrap(),
        );
        archive.extend_from_slice(frame);
    }
    let index_offset = archive.len();
    let (index, hash) = Index::with_entries(entries)
        .unwrap()
        .encode_and_hash()
        .unwrap();
    archive.extend_from_slice(&index);
    let footer_offset = archive.len();
    archive.resize(footer_offset + FOOTER_LEN, 0);
    Footer::new(index_offset as u64, index.len() as u64, hash)
        .unwrap()
        .encode(&mut archive, footer_offset)
        .unwrap();
    elang_reader::Package::open(&archive).expect("valid structural framing");
    archive
}

fn compressed_zeros(size: usize) -> Vec<u8> {
    elang_container::frame::encode(&vec![0; size], elang_container::frame::Profile::Fast)
        .expect("compress synthetic resource")
}

async fn assert_decode_limit_rejected(bytes: Vec<u8>) {
    assert!(
        bytes.len() < DEFAULT_MAX_BODY_BYTES,
        "compressed body must fit"
    );
    for path in ["/verify", "/validate"] {
        let (status, _, body) = post_octet_stream(build_app(), path, bytes.clone()).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{path}");
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], false);
        assert_eq!(json["code"], "request_too_large");
    }
    // Inspect remains a listing operation and never decompresses these frames.
    let (status, _, body) = post_octet_stream(build_app(), "/inspect", bytes).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
}

#[tokio::test]
async fn compressed_resource_over_decoded_cap_is_rejected_before_decoding() {
    let size = 8 * 1024 * 1024 + 1;
    let archive = archive_from_frames(&[("payload", compressed_zeros(size), size as u64)]);
    assert_decode_limit_rejected(archive).await;
}

#[tokio::test]
async fn aggregate_decoded_cap_applies_across_resources() {
    let size = 4 * 1024 * 1024 + 1;
    let frame = compressed_zeros(size);
    let archive =
        archive_from_frames(&[("a", frame.clone(), size as u64), ("b", frame, size as u64)]);
    assert_decode_limit_rejected(archive).await;
}

#[tokio::test]
async fn understated_index_cannot_bypass_decoded_cap() {
    let frame = compressed_zeros(8 * 1024 * 1024 + 1);
    let archive = archive_from_frames(&[("payload", frame, 1)]);
    assert_decode_limit_rejected(archive).await;
}

#[tokio::test]
async fn decoder_window_is_bounded_independently_of_decoded_size() {
    // A standard non-single-segment frame with a 16 MiB history window,
    // one raw byte and checksum. Frame inspection accepts it without decoding.
    let frame = vec![
        0x28, 0xb5, 0x2f, 0xfd, 0x84, 0x70, 1, 0, 0, 0, 0x09, 0, 0, 0, 0, 0, 0, 0,
    ];
    let header = elang_container::frame::inspect(&frame).unwrap();
    assert_eq!(header.content_size, 1);
    assert_eq!(header.window_size, 16 * 1024 * 1024);
    let archive = archive_from_frames(&[("payload", frame, 1)]);
    assert_decode_limit_rejected(archive).await;
}

#[tokio::test]
async fn exact_decoded_cap_is_admitted_to_the_sdk() {
    let size = 8 * 1024 * 1024;
    let archive = archive_from_frames(&[("payload", compressed_zeros(size), size as u64)]);
    for path in ["/verify", "/validate"] {
        let (status, _, body) = post_octet_stream(build_app(), path, archive.clone()).await;
        // The service admits the boundary, then SDK stage 8 rejects the
        // intentionally absent bootstrap resources using the existing contract.
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], false);
        assert_eq!(json["stage"], "verify");
        assert_ne!(json["code"], "request_too_large");
    }
}

/// Send only the headers: the advertised bytes remain unsent so body
/// extraction must wait. A saturated route must still return a response.
fn send_incomplete_request(address: std::net::SocketAddr, path: &str) -> std::net::TcpStream {
    use std::io::Write;

    let mut stream = std::net::TcpStream::connect(address).expect("connect to test server");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("bound response wait");
    write!(stream,
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\nConnection: close\r\n\r\n"
    ).expect("send request headers");
    stream
}

fn read_status_line(stream: std::net::TcpStream) -> String {
    use std::io::BufRead;

    let mut line = String::new();
    std::io::BufReader::new(stream)
        .read_line(&mut line)
        .expect("response arrives before the missing request body");
    line
}

async fn wait_for_capacity(state: &AppState, expected: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.permits().available_permits() != expected {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("capacity changes within the test budget");
}

#[tokio::test]
async fn body_buffering_holds_capacity_and_saturated_requests_do_not_wait_for_bodies() {
    use std::io::Write;

    let state = AppState::with_concurrency(30_000, 1);
    let app = router(state.clone(), DEFAULT_MAX_BODY_BYTES);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("test listener address");
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    for path in ["/inspect", "/verify", "/validate"] {
        let mut buffering = send_incomplete_request(address, path);
        // The admitted request owns the only permit before its body arrives.
        wait_for_capacity(&state, 0).await;
        let status = tokio::task::spawn_blocking(move || {
            read_status_line(send_incomplete_request(address, path))
        })
        .await
        .expect("saturated request joined");
        assert!(status.starts_with("HTTP/1.1 503"), "{path}: {status}");

        buffering.write_all(b"xx").expect("finish admitted body");
        let status = tokio::task::spawn_blocking(move || read_status_line(buffering))
            .await
            .expect("admitted request joined");
        // The malformed SDK content still follows the established 200/ok:false contract.
        assert!(status.starts_with("HTTP/1.1 200"), "{path}: {status}");
        wait_for_capacity(&state, 1).await;
    }
    server.abort();
}

#[tokio::test]
async fn failed_body_extraction_releases_capacity() {
    let state = AppState::with_concurrency(30_000, 1);
    let app = router(state.clone(), 16);
    for path in ["/inspect", "/verify", "/validate"] {
        let (status, _, _) = post_octet_stream(app.clone(), path, vec![0; 32]).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(state.permits().available_permits(), 1);
    }
}

#[tokio::test]
async fn stalled_bodies_time_out_and_release_sdk_capacity() {
    use std::io::Write;

    let state = AppState::with_concurrency(200, 1);
    let app = router(state.clone(), DEFAULT_MAX_BODY_BYTES);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("test listener address");
    let server_app = app.clone();
    let server = tokio::spawn(async move { axum::serve(listener, server_app).await });

    for path in ["/inspect", "/verify", "/validate"] {
        for partial_body in [false, true] {
            let mut stalled = send_incomplete_request(address, path);
            if partial_body {
                stalled.write_all(b"x").expect("send partial body");
            }
            wait_for_capacity(&state, 0).await;
            let status = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio::task::spawn_blocking(move || read_status_line(stalled)),
            )
            .await
            .expect("body timeout follows the 200 ms handler budget")
            .expect("stalled request joined");
            // Axum maps a body-read error to 400, before SDK processing starts.
            assert!(status.starts_with("HTTP/1.1 400"), "{path}: {status}");
            wait_for_capacity(&state, 1).await;

            let (status, _, body) = post_octet_stream(app.clone(), path, fixture_archive()).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
            assert_eq!(json["ok"], true, "subsequent SDK request succeeds: {path}");
        }
    }
    server.abort();
}
