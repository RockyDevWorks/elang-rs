//! Error construction helpers for the EPF schema layer.
//!
//! Every diagnostic surfaced by `elang-epf` carries a code from
//! `specs/registry/error-codes.{md,json}` and the `EPF_SCHEMA` category
//! (stage 10), except for KDL parser errors (category routed from
//! `elang_kdl::error::ErrorCode::category`) and underlying `ResourcePath`
//! errors (propagated with their `RESOURCE_PATH` category).
//!
//! Codes emitted by `elang-core::model::epf` (e.g. `EPF_IDENTIFIER_BAD_URN`,
//! `EPF_NAME_EMPTY`) are remapped onto registry-canonical EPF_SCHEMA codes
//! (`EPF_BAD_URN`, `EPF_BAD_METADATA_FIELD`) at this layer so a consumer
//! filtering on the registry always matches. The underlying reason survives
//! in the diagnostic message.

use elang_core::error::{Category, Diagnostic, Error, KdlSpan};
use elang_kdl::span::SourceSpan;

/// Build an `EPF_SCHEMA` diagnostic pointing at `span`.
///
/// The category is always [`Category::EpfSchema`] and the stage is `10`; the
/// span is dropped when it is [`SourceSpan::INVALID`] (synthetic nodes) or
/// cannot be expressed as a [`KdlSpan`].
pub(crate) fn epf_err_at(
    code: &'static str,
    message: impl Into<String>,
    span: SourceSpan,
) -> Error {
    let mut d = Diagnostic::new(code, Category::EpfSchema, message).with_stage(10);
    if let Some(s) = span_to_kdl(span) {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Build an `EPF_SCHEMA` diagnostic without a source span.
pub(crate) fn epf_err(code: &'static str, message: impl Into<String>) -> Error {
    epf_err_at(code, message, SourceSpan::INVALID)
}

/// Build a `LIMIT` diagnostic. Used when an input-level ceiling is breached
/// before the EPF schema rules can fire.
#[allow(dead_code, reason = "kept for future input-size guards")]
pub(crate) fn limit_err(code: &'static str, message: impl Into<String>) -> Error {
    Error::new(code, Category::Limit, message)
}

/// Map a KDL `SourceSpan` onto the `KdlSpan` carried by `Diagnostic`.
///
/// The KDL span records 1-based line/column plus byte length, which is what
/// every other diagnostic in the reference implementation uses. The EPF layer
/// never receives a span past EOF, so any `Position::INVALID` becomes `None`.
fn span_to_kdl(span: SourceSpan) -> Option<KdlSpan> {
    if span.is_invalid() {
        return None;
    }
    if span.start.is_invalid() || span.end.is_invalid() {
        return None;
    }
    let length = span.len();
    // `SourceSpan::len` already saturates to `u32::MAX` when the endpoints
    // are invalid, so the value is always in range.
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
    let mut d = Diagnostic::new(code, category, message).with_stage(10);
    if let Some(s) = span_kdl {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

/// Translate an `Error` returned by `elang-core::model::epf` constructors into
/// an EPF_SCHEMA error with a registry-canonical code.
///
/// `elang-core`'s model uses fine-grained codes (`EPF_IDENTIFIER_BAD_URN`,
/// `EPF_NAME_EMPTY`, ...) that predate the registry. The EPF schema layer
/// owns the diagnostic surface and remaps them onto the coarser registry
/// codes (`EPF_BAD_URN`, `EPF_BAD_METADATA_FIELD`) so consumers filtering on
/// the registry always match.
pub(crate) fn from_core(err: Error) -> Error {
    use Category::*;
    let code = err.code();
    let mapped = match (err.category(), code) {
        (
            EpfSchema,
            "EPF_IDENTIFIER_EMPTY"
            | "EPF_IDENTIFIER_TOO_LONG"
            | "EPF_IDENTIFIER_BAD_URN"
            | "EPF_IDENTIFIER_BAD_SLUG",
        ) => "EPF_BAD_URN",
        (EpfSchema, "EPF_NAME_EMPTY" | "EPF_NAME_TOO_LONG" | "EPF_DESCRIPTION_TOO_LONG") => {
            "EPF_BAD_METADATA_FIELD"
        }
        (EpfSchema, "EPF_BAD_VERSION") => "EPF_BAD_VERSION",
        (EpfSchema, "EPF_VERSION_MISMATCH") => "EPF_VERSION_MISMATCH",
        _ => return err,
    };
    Error::new(mapped, Category::EpfSchema, err.diagnostic.message)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
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
    fn from_core_remaps_identifier_codes() {
        let e = Error::new(
            "EPF_IDENTIFIER_BAD_URN",
            Category::EpfSchema,
            "urn scheme wrong",
        );
        let r = from_core(e);
        assert_eq!(r.code(), "EPF_BAD_URN");
        assert_eq!(r.category(), Category::EpfSchema);
        assert_eq!(r.diagnostic.message, "urn scheme wrong");
    }

    #[test]
    fn from_core_remaps_metadata_codes() {
        let e = Error::new("EPF_NAME_EMPTY", Category::EpfSchema, "name empty");
        assert_eq!(from_core(e).code(), "EPF_BAD_METADATA_FIELD");
        let e = Error::new(
            "EPF_DESCRIPTION_TOO_LONG",
            Category::EpfSchema,
            "desc too long",
        );
        assert_eq!(from_core(e).code(), "EPF_BAD_METADATA_FIELD");
    }

    #[test]
    fn from_core_passes_through_path_errors() {
        let e = Error::new("PATH_ABSOLUTE", Category::ResourcePath, "absolute path");
        let r = from_core(e);
        assert_eq!(r.code(), "PATH_ABSOLUTE");
        assert_eq!(r.category(), Category::ResourcePath);
    }

    #[test]
    fn from_core_passes_through_epf_version_mismatch() {
        let e = Error::new(
            "EPF_VERSION_MISMATCH",
            Category::EpfSchema,
            "version mismatch",
        );
        assert_eq!(from_core(e).code(), "EPF_VERSION_MISMATCH");
    }
}
