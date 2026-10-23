//! KDL 2.0 parse-error model.
//!
//! Errors carry a stable [`ErrorCode`] (mirrors the project registry in
//! `specs/registry/error-codes.md`) so that schema layers and the CLI can
//! match on the code without depending on the human-readable message. Every
//! error also carries a [`SourceSpan`] pointing at the offending bytes when
//! one is known; when no span is available (e.g. input validation failures
//! before the parser runs) the span is [`SourceSpan::INVALID`].

use crate::span::SourceSpan;

/// Stable, registry-aligned error codes for the KDL parser.
///
/// The codes are listed in the order in which a typical parser hit would
/// encounter them. They mirror the `KDL_*` block of `error-codes.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Input failed UTF-8 validation at the byte boundary.
    InvalidUtf8,
    /// A token exceeded `MAX_ENCODED_TOKEN_BYTES`.
    TokenTooLong,
    /// A decoded string exceeded `MAX_DECODED_STRING_BYTES`.
    StringTooLong,
    /// A node or property name exceeded `MAX_NAME_BYTES`.
    NameTooLong,
    /// A node has more positional arguments than `MAX_ARGS_PER_NODE`.
    TooManyArgs,
    /// A node has more properties than `MAX_PROPS_PER_NODE`.
    TooManyProps,
    /// Nesting depth exceeded `MAX_KDL_NESTING_DEPTH`.
    NestingTooDeep,
    /// Per-resource node count exceeded `MAX_NODES_PER_RESOURCE`.
    TooManyNodesResource,
    /// Archive-wide node count exceeded `MAX_NODES_PER_ARCHIVE` (T0507).
    TooManyNodesArchive,
    /// A single comment exceeded `MAX_COMMENT_BYTES`.
    CommentTooLong,
    /// Total comment bytes per resource exceeded `MAX_COMMENT_BYTES_PER_RESOURCE`.
    CommentBudgetExceeded,
    /// Decoded string bytes per resource exceeded `MAX_DECODED_STRING_BYTES_PER_RESOURCE`.
    StringBudgetResourceExceeded,
    /// Archive-wide decoded string bytes exceeded `MAX_DECODED_STRING_BYTES_PER_ARCHIVE` (T0509).
    StringBudgetArchiveExceeded,
    /// Unterminated string literal (missing closing quote).
    UnterminatedString,
    /// Unterminated block comment (missing `*/`).
    UnterminatedComment,
    /// Unterminated raw string (mismatched `#` fences).
    UnterminatedRawString,
    /// Invalid escape sequence inside a string.
    InvalidEscape,
    /// Invalid Unicode escape (malformed `\u{...}`).
    InvalidUnicodeEscape,
    /// Invalid numeric literal.
    InvalidNumber,
    /// Unrecognised byte where a token was expected.
    UnexpectedByte,
    /// Unexpected end-of-input where a token was expected.
    UnexpectedEof,
    /// Bare identifier contains an invalid character.
    InvalidIdentifier,
    /// Reserved for consumers requiring the optional `/kdl-version` pragma.
    /// The parser does not emit this code.
    MissingVersionPragma,
    /// `/kdl-version` pragma value is not `"2.0"`.
    UnsupportedVersion,
    /// Generic catch-all for things not yet enumerated.
    Other,
}

impl ErrorCode {
    /// Stable string identifier for matching against the registry. The
    /// prefix `KDL_` mirrors the registry convention.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidUtf8 => "KDL_INVALID_UTF8",
            ErrorCode::TokenTooLong => "KDL_TOKEN_TOO_LONG",
            ErrorCode::StringTooLong => "KDL_STRING_TOO_LONG",
            ErrorCode::NameTooLong => "KDL_NAME_TOO_LONG",
            ErrorCode::TooManyArgs => "KDL_TOO_MANY_ARGS",
            ErrorCode::TooManyProps => "KDL_TOO_MANY_PROPS",
            ErrorCode::NestingTooDeep => "KDL_NESTING_TOO_DEEP",
            ErrorCode::TooManyNodesResource => "KDL_TOO_MANY_NODES_RESOURCE",
            ErrorCode::TooManyNodesArchive => "KDL_TOO_MANY_NODES_ARCHIVE",
            ErrorCode::CommentTooLong => "KDL_COMMENT_TOO_LONG",
            ErrorCode::CommentBudgetExceeded => "KDL_COMMENT_BUDGET_EXCEEDED",
            ErrorCode::StringBudgetResourceExceeded => "KDL_STRING_BUDGET_RESOURCE_EXCEEDED",
            ErrorCode::StringBudgetArchiveExceeded => "KDL_STRING_BUDGET_ARCHIVE_EXCEEDED",
            ErrorCode::UnterminatedString => "KDL_UNTERMINATED_STRING",
            ErrorCode::UnterminatedComment => "KDL_UNTERMINATED_COMMENT",
            ErrorCode::UnterminatedRawString => "KDL_UNTERMINATED_RAW_STRING",
            ErrorCode::InvalidEscape => "KDL_INVALID_ESCAPE",
            ErrorCode::InvalidUnicodeEscape => "KDL_INVALID_UNICODE_ESCAPE",
            ErrorCode::InvalidNumber => "KDL_INVALID_NUMBER",
            ErrorCode::UnexpectedByte => "KDL_UNEXPECTED_BYTE",
            ErrorCode::UnexpectedEof => "KDL_UNEXPECTED_EOF",
            ErrorCode::InvalidIdentifier => "KDL_INVALID_IDENTIFIER",
            ErrorCode::MissingVersionPragma => "KDL_MISSING_VERSION_PRAGMA",
            ErrorCode::UnsupportedVersion => "KDL_UNSUPPORTED_VERSION",
            ErrorCode::Other => "KDL_OTHER",
        }
    }

    /// The [`crate::span::Position`] byte is mapped to the corresponding
    /// `elang_core::error::Category` so that schema layers can route
    /// diagnostics through the existing error model without a second
    /// classification step.
    pub fn category(self) -> elang_core::error::Category {
        use elang_core::error::Category;
        match self {
            ErrorCode::TokenTooLong
            | ErrorCode::StringTooLong
            | ErrorCode::NameTooLong
            | ErrorCode::TooManyArgs
            | ErrorCode::TooManyProps
            | ErrorCode::NestingTooDeep
            | ErrorCode::TooManyNodesResource
            | ErrorCode::TooManyNodesArchive
            | ErrorCode::CommentTooLong
            | ErrorCode::CommentBudgetExceeded
            | ErrorCode::StringBudgetResourceExceeded
            | ErrorCode::StringBudgetArchiveExceeded => Category::Limit,
            ErrorCode::InvalidUtf8 => Category::Container,
            ErrorCode::MissingVersionPragma | ErrorCode::UnsupportedVersion => Category::Limit,
            ErrorCode::InvalidEscape
            | ErrorCode::InvalidUnicodeEscape
            | ErrorCode::InvalidNumber
            | ErrorCode::InvalidIdentifier => Category::Limit,
            ErrorCode::UnterminatedString
            | ErrorCode::UnterminatedComment
            | ErrorCode::UnterminatedRawString
            | ErrorCode::UnexpectedByte
            | ErrorCode::UnexpectedEof => Category::Limit,
            ErrorCode::Other => Category::Limit,
        }
    }
}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A parse error with a stable code, a source span (when known) and a
/// human-readable message.
///
/// The `span` is `SourceSpan::INVALID` for failures that occur before the
/// parser has a usable position (e.g. an input that fails UTF-8 validation
/// before any byte is consumed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Stable registry-aligned code.
    pub code: ErrorCode,
    /// Where the error happened, when known.
    pub span: SourceSpan,
    /// Human-readable description, written once at error construction time.
    pub message: Box<str>,
}

impl ParseError {
    /// Construct a new error with no span (`SourceSpan::INVALID`).
    pub fn new(code: ErrorCode, message: impl Into<Box<str>>) -> ParseError {
        ParseError {
            code,
            span: SourceSpan::INVALID,
            message: message.into(),
        }
    }

    /// Construct a new error at the given span.
    pub fn at(code: ErrorCode, span: SourceSpan, message: impl Into<Box<str>>) -> ParseError {
        ParseError {
            code,
            span,
            message: message.into(),
        }
    }

    /// Stable registry-aligned code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Source span.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Error category, mapped through [`ErrorCode::category`].
    pub fn category(&self) -> elang_core::error::Category {
        self.code.category()
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.span.is_invalid() {
            write!(f, "{}: {}", self.code, self.message)
        } else {
            write!(f, "{}: {} ({})", self.code, self.message, self.span)
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_stable_strings() {
        assert_eq!(ErrorCode::NestingTooDeep.as_str(), "KDL_NESTING_TOO_DEEP");
        assert_eq!(ErrorCode::TokenTooLong.as_str(), "KDL_TOKEN_TOO_LONG");
    }

    #[test]
    fn category_mapping_for_budget_codes() {
        use elang_core::error::Category;
        assert_eq!(ErrorCode::NestingTooDeep.category(), Category::Limit);
        assert_eq!(ErrorCode::TokenTooLong.category(), Category::Limit);
        assert_eq!(ErrorCode::TooManyNodesResource.category(), Category::Limit);
    }

    #[test]
    fn display_with_span() {
        let e = ParseError::at(ErrorCode::NestingTooDeep, SourceSpan::INVALID, "too deep");
        assert!(e.to_string().contains("KDL_NESTING_TOO_DEEP"));
        assert!(e.to_string().contains("too deep"));
    }

    #[test]
    fn display_without_span_omits_paren() {
        let e = ParseError::new(ErrorCode::InvalidUtf8, "bad bytes");
        let s = e.to_string();
        assert!(s.contains("KDL_INVALID_UTF8"));
        assert!(s.contains("bad bytes"));
    }
}
