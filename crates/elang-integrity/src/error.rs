//! Error construction helpers for the integrity manifest layer.
//!
//! Every diagnostic surfaced by `elang-integrity` carries a code from
//! `specs/registry/error-codes.{md,json}` under the `INTEGRITY` category
//! (stage 9 for structural validation; stage 11 for per-resource hash
//! verification via `verify_one`), except for:
//!
//! - KDL parser errors, which retain their own category (`LIMIT` for budget
//!   failures, `CONTAINER` for `KDL_INVALID_UTF8`, etc.) and are routed
//!   through `from_kdl`.
//! - `ResourcePath` construction errors, which carry the `RESOURCE_PATH`
//!   category and propagate unchanged.
//! - NFC / forbidden-category violations on string values (grammar §1), which
//!   are routed through the `UNICODE` category — same as `elang-epf`.
//!
//! Codes emitted by `elang-core::ResourcePath::new` (`PATH_EMPTY_SEGMENT`,
//! `PATH_TOO_LONG`, `PATH_ABSOLUTE`, `PATH_BACKSLASH`, `PATH_NUL`) propagate
//! unchanged so a consumer filtering on those registry codes continues to
//! match.

use elang_core::error::{Category, Diagnostic, Error, KdlSpan};
use elang_kdl::span::SourceSpan;

/// Build an `INTEGRITY` diagnostic pointing at `span`.
///
/// The category is always [`Category::Integrity`] and the stage is `9` (the
/// manifest is parsed at stage 9 of the 12-stage reader pipeline). The span is
/// dropped when it is [`SourceSpan::INVALID`] (synthetic nodes) or cannot be
/// expressed as a [`KdlSpan`].
pub(crate) fn integrity_err_at(
    code: &'static str,
    message: impl Into<String>,
    span: SourceSpan,
) -> Error {
    let mut d = Diagnostic::new(code, Category::Integrity, message).with_stage(9);
    if let Some(s) = span_to_kdl(span) {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Build an `INTEGRITY` diagnostic without a source span.
///
/// Use for violations that originate from cross-field checks
/// (e.g. [`crate::manifest::verify_coverage`]) where there is no single KDL
/// span to point at.
pub(crate) fn integrity_err(code: &'static str, message: impl Into<String>) -> Error {
    integrity_err_at(code, message, SourceSpan::INVALID)
}

/// Map a [`SourceSpan`] onto a diagnostic [`KdlSpan`].
///
/// Returns `None` for any invalid span (one whose endpoints carry the
/// `Position::INVALID` sentinel); the KDL span records 1-based line/column
/// plus byte length, matching every other diagnostic in the reference
/// implementation.
pub(crate) fn span_to_kdl(span: SourceSpan) -> Option<KdlSpan> {
    if span.is_invalid() {
        return None;
    }
    let length = span.len();
    Some(KdlSpan {
        line: span.start.line,
        column: span.start.column,
        length,
    })
}

/// Translate a `ParseError` from `elang-kdl` into the crate-wide `Error` type
/// with its span attached.
///
/// The KDL parser owns its own category routing (budget failures → `LIMIT`,
/// UTF-8 → `CONTAINER`); we attach its diagnostic as-is and just normalize the
/// span.
pub(crate) fn from_kdl(err: elang_kdl::ParseError) -> Error {
    let code = err.code().as_str();
    let category = err.category();
    let span = err.span();
    let span_kdl = span_to_kdl(span);
    let message = err.message.to_string();
    let mut d = Diagnostic::new(code, category, message).with_stage(9);
    if let Some(s) = span_kdl {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact diagnostic codes/categories; a panic signals a regression"
)]
mod tests {
    use super::*;
    use elang_kdl::span::Position;

    #[test]
    fn integrity_err_helper_has_no_span() {
        let err = integrity_err("INTEGRITY_STRUCTURE_INVALID", "msg");
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
        assert_eq!(err.diagnostic.category, Category::Integrity);
        assert_eq!(err.diagnostic.stage, Some(9));
        assert!(err.diagnostic.span.is_none());
    }

    #[test]
    fn integrity_err_at_carries_valid_span() {
        let span = SourceSpan {
            start: Position {
                line: 3,
                column: 5,
                byte: 0,
            },
            end: Position {
                line: 3,
                column: 8,
                byte: 3,
            },
        };
        let err = integrity_err_at("INTEGRITY_BAD_HEX", "msg", span);
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
        let carried = err.diagnostic.span.expect("valid span should carry");
        assert_eq!(carried.line, 3);
        assert_eq!(carried.column, 5);
        assert_eq!(carried.length, 3);
    }

    #[test]
    fn integrity_err_at_drops_invalid_span() {
        let err = integrity_err_at("INTEGRITY_STRUCTURE_INVALID", "msg", SourceSpan::INVALID);
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
        assert_eq!(err.diagnostic.category, Category::Integrity);
        assert_eq!(err.diagnostic.stage, Some(9));
        assert!(err.diagnostic.span.is_none());
    }
}
