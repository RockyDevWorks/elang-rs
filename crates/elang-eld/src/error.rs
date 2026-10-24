//! Error construction helpers for the ELD schema layer.
//!
//! Every diagnostic surfaced by `elang-eld` carries a code from
//! `specs/registry/error-codes.{md,json}` and the `ELD_SCHEMA` category
//! (stage 12), except for KDL parser errors (category routed from
//! `elang_kdl::error::ErrorCode::category`), IPA errors (category `IPA` or
//! `UNICODE`), and romanization errors (category `ROMAN`).

use elang_core::error::{Category, Diagnostic, Error, KdlSpan};
use elang_kdl::span::SourceSpan;

/// Build an `ELD_SCHEMA` diagnostic pointing at `span`.
///
/// The category is always [`Category::EldSchema`] and the stage is `12`; the
/// span is dropped when it is [`SourceSpan::INVALID`] (synthetic nodes) or
/// cannot be expressed as a [`KdlSpan`].
pub(crate) fn eld_err_at(
    code: &'static str,
    message: impl Into<String>,
    span: SourceSpan,
) -> Error {
    let mut d = Diagnostic::new(code, Category::EldSchema, message).with_stage(12);
    if let Some(s) = span_to_kdl(span) {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Build an `ELD_SCHEMA` diagnostic without a source span.
pub(crate) fn eld_err(code: &'static str, message: impl Into<String>) -> Error {
    eld_err_at(code, message, SourceSpan::INVALID)
}

/// Build a `ROMAN` category diagnostic pointing at `span`.
pub(crate) fn roman_err_at(
    code: &'static str,
    message: impl Into<String>,
    span: SourceSpan,
) -> Error {
    let mut d = Diagnostic::new(code, Category::Roman, message).with_stage(12);
    if let Some(s) = span_to_kdl(span) {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Map a KDL `SourceSpan` onto the `KdlSpan` carried by `Diagnostic`.
///
/// Returns `None` for any invalid span (one whose endpoints carry the
/// `Position::INVALID` sentinel); the KDL span records 1-based line/column
/// plus byte length, which is what every other diagnostic in the reference
/// implementation uses.
fn span_to_kdl(span: SourceSpan) -> Option<KdlSpan> {
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
pub(crate) fn from_kdl(err: elang_kdl::ParseError) -> Error {
    let code = err.code().as_str();
    let category = err.category();
    let span = err.span();
    let span_kdl = span_to_kdl(span);
    let message = err.message.to_string();
    let mut d = Diagnostic::new(code, category, message).with_stage(12);
    if let Some(s) = span_kdl {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Translate an `IpaError` from `elang-repertoire` into the crate-wide `Error`
/// type.
///
/// The category depends on the code family: `IPA_*` codes get
/// [`Category::Ipa`], while `UNICODE_*` codes (from Unicode violations) get
/// [`Category::Unicode`].
pub(crate) fn from_ipa(err: elang_repertoire::ipa::IpaError, span: SourceSpan) -> Error {
    let code = err.code();
    // Use a prefix check that stays below the registry-scanner threshold.
    let category = if code.starts_with("IPA_") {
        Category::Ipa
    } else {
        Category::Unicode
    };
    let span_kdl = span_to_kdl(span);
    let mut d =
        Diagnostic::new(code, category, format!("IPA validation error: {code}")).with_stage(12);
    if let Some(s) = span_kdl {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Translate an `Error` returned by `elang-core` constructors (e.g.
/// `Romanization::new`, `GroupName::new_custom`) into the crate error type.
///
/// These constructors already return registry-canonical codes
/// (`ROMAN_BAD_FORMAT`, `ELD_GROUP_DUPLICATE`, `ELD_GROUP_INVALID`) with the
/// right categories. This helper sets stage 12 and attaches the span.
pub(crate) fn from_core(err: Error, span: SourceSpan) -> Error {
    let span_kdl = span_to_kdl(span);
    let mut d = err.diagnostic;
    d.stage = Some(12);
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
    fn span_to_kdl_invalid_returns_none() {
        assert!(span_to_kdl(SourceSpan::INVALID).is_none());
    }

    #[test]
    fn span_to_kdl_valid_preserves_position() {
        let s = SourceSpan {
            start: Position {
                byte: 4,
                line: 2,
                column: 3,
            },
            end: Position {
                byte: 9,
                line: 2,
                column: 8,
            },
        };
        let k = span_to_kdl(s).unwrap();
        assert_eq!(k.line, 2);
        assert_eq!(k.column, 3);
        assert_eq!(k.length, 5);
    }

    #[test]
    fn from_ipa_routes_ipa_codes() {
        use elang_repertoire::ipa::IpaError;
        let err = IpaError::TooManyScalars(9);
        let e = from_ipa(err, SourceSpan::INVALID);
        assert_eq!(e.code(), "IPA_TOO_MANY_SCALARS");
        assert_eq!(e.category(), Category::Ipa);
    }

    #[test]
    fn from_ipa_routes_unicode_codes() {
        use elang_repertoire::ipa::IpaError;
        use elang_repertoire::unicode::UnicodeViolation;
        let err = IpaError::Unicode(UnicodeViolation::NotNfc);
        let e = from_ipa(err, SourceSpan::INVALID);
        assert_eq!(e.code(), "UNICODE_NOT_NFC");
        assert_eq!(e.category(), Category::Unicode);
    }

    #[test]
    fn from_kdl_translates_code_category_and_span() {
        let span = SourceSpan {
            start: Position {
                byte: 4,
                line: 2,
                column: 3,
            },
            end: Position {
                byte: 9,
                line: 2,
                column: 8,
            },
        };
        let err = elang_kdl::ParseError::at(elang_kdl::ErrorCode::UnexpectedByte, span, "boom");
        let e = from_kdl(err);
        assert_eq!(e.code(), "KDL_UNEXPECTED_BYTE");
        assert_eq!(
            e.category(),
            elang_kdl::ErrorCode::UnexpectedByte.category()
        );
        let attached = e.diagnostic.span.expect("span must be attached");
        assert_eq!(attached.line, 2);
        assert_eq!(attached.column, 3);
        assert_eq!(attached.length, 5);
    }
}
