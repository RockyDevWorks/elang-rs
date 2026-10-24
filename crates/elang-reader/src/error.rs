//! Error construction helpers for the integrated reader pipeline.
//!
//! Diagnostics emitted from this crate carry the registry category and stage
//! for the *stage the diagnostic surfaced from*. Stages 1–7 are owned by
//! `elang-container` (its diagnostics already carry the right stage), stages
//! 8–9 by this crate (bootstrap / integrity-structure cross-checks), and
//! stages 10–12 by `elang-epf` / `elang-eld` (their errors propagate with the
//! stage the upstream layer assigned). The helpers here only build the
//! stage-8 bootstrap diagnostics; everything else passes through unchanged.

use elang_core::error::{Category, Diagnostic, Error};

/// Stage 8 (bootstrap). Used for failures in the cross-check between the
/// binary header and the in-archive `Header/format.version` / `Header/mimetype`
/// resources.
const BOOTSTRAP_STAGE: u8 = 8;

/// Build a `CONTAINER`-category, stage-8 diagnostic tagged with `resource_path`.
pub(crate) fn bootstrap_err(
    code: &'static str,
    message: impl Into<String>,
    resource_path: impl Into<String>,
) -> Error {
    Error {
        diagnostic: Diagnostic::new(code, Category::Container, message)
            .with_stage(BOOTSTRAP_STAGE)
            .with_resource(resource_path),
    }
}

/// Build a `CONTAINER`-category, stage-8 diagnostic without a resource tag.
///
/// Used when the failure is the absence of a required bootstrap resource
/// (no path to attach).
pub(crate) fn bootstrap_err_no_path(code: &'static str, message: impl Into<String>) -> Error {
    Error {
        diagnostic: Diagnostic::new(code, Category::Container, message).with_stage(BOOTSTRAP_STAGE),
    }
}
