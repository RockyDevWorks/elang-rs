//! Error construction helpers for the integrated reader pipeline.
//!
//! Diagnostics emitted from this crate carry the registered category and the
//! stage where the failure surfaced. Bootstrap content checks use stage 8
//! and the `CONTAINER` category; resource lookup and frame errors retain
//! their own categories and are tagged with the caller's stage.

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
    resource_err(
        code,
        Category::Container,
        message,
        resource_path,
        BOOTSTRAP_STAGE,
    )
}

/// Build a resource diagnostic with its registered category and caller's stage.
pub(crate) fn resource_err(
    code: &'static str,
    category: Category,
    message: impl Into<String>,
    resource_path: impl Into<String>,
    stage: u8,
) -> Error {
    Error {
        diagnostic: Diagnostic::new(code, category, message)
            .with_stage(stage)
            .with_resource(resource_path),
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "unit tests for error builders; a panic signals a builder regression"
)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_err_attaches_stage_8_and_resource() {
        let err = bootstrap_err(
            "MIMETYPE_INVALID",
            "Header/mimetype content does not match",
            "Header/mimetype",
        );
        assert_eq!(err.diagnostic.code, "MIMETYPE_INVALID");
        assert_eq!(err.diagnostic.category, Category::Container);
        assert_eq!(err.diagnostic.stage, Some(8));
        assert_eq!(
            err.diagnostic.resource_path.as_deref(),
            Some("Header/mimetype")
        );
    }

    #[test]
    fn resource_err_uses_caller_category_and_stage() {
        let err = resource_err(
            "INTEGRITY_HASH_MISMATCH",
            Category::Integrity,
            "hash mismatch",
            "Linguistics/phonemes.eld",
            11,
        );
        assert_eq!(err.diagnostic.code, "INTEGRITY_HASH_MISMATCH");
        assert_eq!(err.diagnostic.category, Category::Integrity);
        assert_eq!(err.diagnostic.stage, Some(11));
        assert_eq!(
            err.diagnostic.resource_path.as_deref(),
            Some("Linguistics/phonemes.eld")
        );
    }
}
