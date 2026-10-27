//! JSON response shapes for the three SDK modes plus the shared diagnostic
//! type. Every struct here mirrors a member of
//! `RockyDevWorks/elang-hub` `src/lib/sdk/contract.ts` — when that file
//! changes, the matching shape here is the source of truth.
//!
//! The Hub's TypeScript shape is camelCase; `#[serde(rename_all = "camelCase")]`
//! on each struct handles the conversion.

use serde::{Deserialize, Serialize};

/// One diagnostic emitted by the SDK.
///
/// Matches [`SdkDiagnostic`](https://github.com/RockyDevWorks/elang-hub/blob/main/src/lib/sdk/contract.ts)
/// in the Hub. The Hub narrows unknown stage/code values to its own
/// fallbacks (`'unknown'` for code, the requested mode for stage); we send
/// only valid values so the narrowing is a no-op on the happy path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkDiagnostic {
    /// `'inspect' | 'verify' | 'validate'` — see [`stage_to_label`].
    pub stage: &'static str,
    /// Stable SDK error code (see [`super::error::map_code`]).
    pub code: &'static str,
    /// Human-readable, bounded message.
    pub message: String,
    /// Optional resource/field locator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl SdkDiagnostic {
    /// Build a single-diagnostic vector. Used by the failure path.
    #[must_use]
    pub fn singleton(stage: &'static str, code: &'static str, message: String) -> Vec<Self> {
        vec![Self {
            stage,
            code,
            message,
            path: None,
        }]
    }
}

/// Successful verdict for `inspect` (stages 1–7).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkInspectSuccess {
    /// Always `true`. Literal in the JSON so the Hub's `json.ok === true`
    /// narrowing works.
    pub ok: bool,
    /// Always `"inspect"`.
    pub mode: &'static str,
    /// SDK spec version this build implements.
    pub spec_version: &'static str,
    /// SDK binary-container version this build implements.
    pub binary_container_version: &'static str,
    /// Number of resources discovered in the container index.
    pub discovered_resources: usize,
    /// Diagnostics that did not block success. Empty for v0.1.0.
    pub diagnostics: Vec<SdkDiagnostic>,
}

/// Successful verdict for `verify` (stages 1–11).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkVerifySuccess {
    /// Always `true`.
    pub ok: bool,
    /// Always `"verify"`.
    pub mode: &'static str,
    /// SDK spec version this build implements.
    pub spec_version: &'static str,
    /// SDK binary-container version this build implements.
    pub binary_container_version: &'static str,
    /// Number of resources whose SHA-256 matched the integrity manifest.
    pub verified_resources: usize,
    /// Diagnostics that did not block success. Empty for v0.1.0.
    pub diagnostics: Vec<SdkDiagnostic>,
}

/// Successful verdict for `validate` (stages 1–12).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkValidateSuccess {
    /// Always `true`.
    pub ok: bool,
    /// Always `"validate"`.
    pub mode: &'static str,
    /// SDK spec version this build implements.
    pub spec_version: &'static str,
    /// SDK binary-container version this build implements.
    pub binary_container_version: &'static str,
    /// Path of the phoneme `.eld` resource inside the package.
    pub phoneme_resource_path: String,
    /// Opaque inventory payload. v0.1.0 of the Hub treats this as
    /// `unknown`; we serialize a structured summary plus the canonical
    /// phoneme bytes (hex-encoded) so future Hub view-model code can
    /// narrow into it.
    pub inventory: serde_json::Value,
    /// Diagnostics that did not block success. Empty for v0.1.0.
    pub diagnostics: Vec<SdkDiagnostic>,
}

/// Failed verdict, returned with HTTP status `4xx`/`5xx`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkFailure {
    /// Always `false`.
    pub ok: bool,
    /// Validation stage at which the SDK emitted the failure.
    pub stage: &'static str,
    /// Stable SDK error code.
    pub code: &'static str,
    /// Bounded, service-rendered message.
    pub message: String,
    /// Optional resource/field locator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Always present; carries the leading diagnostic. Empty for
    /// transport-level errors that never reached the SDK.
    pub diagnostics: Vec<SdkDiagnostic>,
}

/// Map an ELANG stage number (1–12) to the Hub's `ValidationStage` label.
///
/// Stages 1–7 are the `inspect` ceiling, 8–11 are `verify`, 12 is `validate`.
/// The Hub's `parseFailure` trusts the SDK's reported stage when it is one
/// of the three known labels, so this mapping is the authoritative stage
/// attribution when an error carries a numeric stage.
///
/// Stages outside 1–12 (which cannot occur in practice — every
/// `elang_core::Diagnostic` in a validated build has `stage: Some(n)` with
/// `1 ≤ n ≤ 12`) fall back to `"inspect"` for defensive defaulting.
#[must_use]
pub fn stage_to_label(stage: u8) -> &'static str {
    match stage {
        1..=7 => "inspect",
        8..=11 => "verify",
        12 => "validate",
        // Defensive: every SDK-attached stage is 1..=12. The fallback keeps
        // the response a valid JSON object even if a future diagnostic
        // breaks that contract.
        _ => "inspect",
    }
}
