//! Server error type, status-code mapping, and `IntoResponse` adapter.
//!
//! The Hub's [`SdkClient`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/client.ts)
//! short-circuits on HTTP status: `413 → request_too_large`,
//! `504/408 → timeout`, `503 → service_unavailable`, anything else `>= 500 →
//! service_unavailable`, anything else `→ unknown`. SDK *content* failures
//! (the package was well-formed but failed validation) therefore must be
//! returned with `200 OK` and `ok: false` in the JSON body — that is the
//! only way the Hub's [`parseSdkResult`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/client.ts)
//! will pick up the structured `SdkErrorCode`.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use elang_core::error::{Category, Error as SdkError};

use crate::response::{stage_to_label, SdkDiagnostic, SdkFailure};

/// Errors that can be returned from a request handler.
///
/// All variants map to a deliberate HTTP status + JSON body. See
/// [`IntoResponse for ServerError`](impl-IntoResponse).
#[derive(Debug)]
pub enum ServerError {
    /// `413` — request body exceeded the configured cap.
    PayloadTooLarge,
    /// `415` — `Content-Type` was not `application/octet-stream`.
    UnsupportedMediaType,
    /// `400` — request was malformed in a way the SDK cannot answer
    /// (e.g. empty body, missing path).
    BadRequest(String),
    /// `200 OK` with `ok: false` body — the SDK returned a structured
    /// failure. This is the success-status shape because the Hub's
    /// `parseSdkResult` is the only path that surfaces `SdkErrorCode`.
    Sdk(SdkFailure),
    /// `504` — handler exceeded the per-request timeout budget.
    Timeout,
    /// `503` — internal server error. Maps to `service_unavailable`
    /// on the Hub side.
    ///
    /// The variant carries no message: anything held in an `Internal`
    /// value would only be useful for server-side logging, but exposing
    /// it in the response could leak panic payloads, internal paths, or
    /// other operator-only data. Construct via [`ServerError::internal`],
    /// which logs the supplied context to stderr and returns the unit
    /// variant.
    Internal,
}

/// Construct an [`ServerError::Internal`] after logging the supplied
/// context to stderr. The context never reaches the client — only the
/// generic "Service is temporarily unavailable." message does.
#[must_use]
pub fn internal(context: &str) -> ServerError {
    eprintln!("elang-server: internal error: {context}");
    ServerError::Internal
}

impl ServerError {
    /// The Hub-friendly `SdkFailure` body for status-bearing variants.
    fn to_failure_body(&self) -> SdkFailure {
        match self {
            Self::PayloadTooLarge => SdkFailure {
                ok: false,
                stage: "inspect",
                code: "request_too_large",
                message: "Request body exceeded the configured size cap.".into(),
                path: None,
                diagnostics: Vec::new(),
            },
            Self::UnsupportedMediaType => SdkFailure {
                ok: false,
                stage: "inspect",
                code: "unknown",
                message: "Content-Type must be application/octet-stream.".into(),
                path: None,
                diagnostics: Vec::new(),
            },
            Self::BadRequest(msg) => SdkFailure {
                ok: false,
                stage: "inspect",
                code: "unknown",
                message: msg.clone(),
                path: None,
                diagnostics: Vec::new(),
            },
            Self::Timeout => SdkFailure {
                ok: false,
                stage: "inspect",
                code: "timeout",
                message: "Service timed out before responding.".into(),
                path: None,
                diagnostics: Vec::new(),
            },
            Self::Internal => SdkFailure {
                ok: false,
                stage: "inspect",
                code: "service_unavailable",
                message: "Service is temporarily unavailable.".into(),
                path: None,
                diagnostics: Vec::new(),
            },
            Self::Sdk(f) => f.clone(),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::Internal => StatusCode::SERVICE_UNAVAILABLE,
            // Deliberately 200 OK for SDK failures — see type docs.
            Self::Sdk(_) => StatusCode::OK,
        };
        (status, Json(self.to_failure_body())).into_response()
    }
}

impl From<SdkError> for ServerError {
    fn from(e: SdkError) -> Self {
        let stage_label = stage_to_label(e.diagnostic.stage.unwrap_or(1));
        let code = map_code(e.diagnostic.category, e.diagnostic.code);
        let message = e.diagnostic.message.clone();
        let path = e.diagnostic.resource_path.clone();
        let diagnostic = SdkDiagnostic {
            stage: stage_label,
            code,
            message: message.clone(),
            path: path.clone(),
        };
        Self::Sdk(SdkFailure {
            ok: false,
            stage: stage_label,
            code,
            message,
            path,
            diagnostics: vec![diagnostic],
        })
    }
}

/// Map an `elang_core::Diagnostic`'s `(category, code)` to the Hub's
/// stable [`SdkErrorCode`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/contract.ts)
/// namespace.
///
/// The mapping is deliberately coarse-grained: every distinct SDK error
/// collapses to one of the 14 Hub codes. The fine-grained `code` lives on
/// each [`SdkDiagnostic`] inside the `diagnostics` array, so a future Hub
/// view model can present a developer-facing stack without losing
/// precision.
#[must_use]
pub fn map_code(category: Category, code: &str) -> &'static str {
    match category {
        Category::Container | Category::Index | Category::Frame | Category::ResourcePath => {
            "malformed_container"
        }
        Category::Integrity => match code {
            // SHA-256 mismatch → the resource bytes disagree with the
            // manifest (i.e. the package is corrupted on disk).
            "INTEGRITY_HASH_MISMATCH" => "corrupted_resource",
            // Every other integrity code is a manifest-structure or
            // manifest-coverage problem (`INTEGRITY_STRUCTURE_INVALID`,
            // `INTEGRITY_MISSING_ENTRY`, `INTEGRITY_EXTRA_ENTRY`,
            // `INTEGRITY_DUPLICATE_ENTRY`, `INTEGRITY_BAD_ALGORITHM`,
            // `INTEGRITY_BAD_HEX`) → the package was *built* wrong.
            _ => "integrity_check_failed",
        },
        Category::EpfSchema => "descriptor_invalid",
        Category::EldSchema
        | Category::Unicode
        | Category::Ipa
        | Category::Roman
        | Category::When => "eld_invalid",
        // Size-limit failures (archive too big, resource too big, etc.)
        // → malformed_container. The Hub maps `request_too_large` only
        // when the HTTP layer rejects the request (413); an in-archive
        // size-cap is content-level, not transport-level.
        Category::Limit => "malformed_container",
        // Canonical serialization failures → eld_invalid (the inventory
        // failed to serialize to its canonical form).
        Category::Canonical => "eld_invalid",
    }
}
