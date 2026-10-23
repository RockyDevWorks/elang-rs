//! KDL 2.0 value types (T0501).
//!
//! KDL 2.0 has six value shapes: null, bool, integer, float, string and raw
//! string. Raw strings differ from strings only in *how they are lexed* — the
//! decoded content is identical — so this crate stores them as one
//! [`KdlValue::String`] variant and keeps the source span to distinguish
//! them for diagnostics. Round-tripping to canonical form always produces a
//! standard double-quoted string (T0513), never a raw string.
//!
//! Integers and floats are kept as their **source text** alongside the
//! parsed value. KDL 2.0 permits integer literals in several bases
//! (`0x`, `0o`, `0b`, decimal, and `_` digit separators) and floats in
//! several formats, none of which are canonical. Retaining the lexeme lets
//! the canonical emitter reproduce the exact original spelling when the
//! caller asks for it, and lets validation layers reject non-canonical
//! spellings without the parser having to own that policy.
//!
//! No float type is parsed eagerly into `f64`: KDL floats can spell values
//! outside the `f64` range and the spec requires the reader to preserve the
//! literal, not to evaluate it. [`KdlValue::Float`] therefore carries the
//! validated literal text plus a `f64` computed best-effort for callers that
//! want numeric ordering.

use crate::span::SourceSpan;
use core::fmt;

/// A parsed KDL value.
///
/// Every variant carries a [`SourceSpan`] pointing at the **full source text**
/// of the value including its delimiters (quotes, base prefix, etc.), so a
/// diagnostic can highlight exactly what the user wrote.
#[derive(Debug, Clone, PartialEq)]
pub enum KdlValue {
    /// The `null` literal.
    Null(SourceSpan),
    /// `true` or `false`.
    Bool {
        /// Source span of the literal.
        span: SourceSpan,
        /// The boolean value.
        value: bool,
    },
    /// An integer literal in any base KDL 2.0 permits.
    Integer {
        /// Source span of the literal.
        span: SourceSpan,
        /// Exact source lexeme, including base prefix and `_` separators.
        lexeme: Box<str>,
        /// Parsed value, when it fits in `i128`.
        value: Option<i128>,
    },
    /// A float literal. The lexeme is preserved verbatim; `approx` is a
    /// best-effort `f64` for callers that need numeric comparison and is
    /// `None` when the literal does not round-trip through `f64`.
    Float {
        /// Source span of the literal.
        span: SourceSpan,
        /// Exact source lexeme, including base prefix and `_` separators.
        lexeme: Box<str>,
        /// Best-effort `f64` value; `None` if unrepresentable.
        approx: Option<f64>,
    },
    /// A string literal. `raw` records whether the source used `r#"…"#`
    /// form so diagnostics and schema layers can enforce a policy about raw
    /// strings; the canonical emitter ignores it and always emits a standard
    /// double-quoted string.
    String {
        /// Source span of the literal, including quotes.
        span: SourceSpan,
        /// Decoded string content (escapes already processed).
        value: Box<str>,
        /// True when the source spelling was a raw string.
        raw: bool,
    },
}

impl KdlValue {
    /// Source span of this value.
    pub fn span(&self) -> SourceSpan {
        match self {
            KdlValue::Null(s)
            | KdlValue::Bool { span: s, .. }
            | KdlValue::Integer { span: s, .. }
            | KdlValue::Float { span: s, .. }
            | KdlValue::String { span: s, .. } => *s,
        }
    }

    /// Returns the string content, or `None` for non-string values.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            KdlValue::String { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Returns the boolean value, or `None` for non-bool values.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            KdlValue::Bool { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Returns the integer value, or `None` for non-integers or for integers
    /// too large for `i128`.
    pub fn as_integer(&self) -> Option<i128> {
        match self {
            KdlValue::Integer { value, .. } => *value,
            _ => None,
        }
    }

    /// Returns the best-effort float value, or `None` for non-floats or
    /// unrepresentable literals.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            KdlValue::Float { approx, .. } => *approx,
            _ => None,
        }
    }

    /// True when this value is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, KdlValue::Null(_))
    }

    /// True when this value is a string (including raw strings).
    pub fn is_string(&self) -> bool {
        matches!(self, KdlValue::String { .. })
    }

    /// The exact source lexeme for integer and float values; `None` for
    /// other shapes.
    pub fn lexeme(&self) -> Option<&str> {
        match self {
            KdlValue::Integer { lexeme, .. } | KdlValue::Float { lexeme, .. } => Some(lexeme),
            _ => None,
        }
    }
}

impl fmt::Display for KdlValue {
    /// Renders the value in canonical KDL 2.0 form (T0513). Equivalent to
    /// [`crate::canon::emit_value`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::canon::emit_value(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp() -> SourceSpan {
        SourceSpan::from_range(b"x", 0, 1)
    }

    #[test]
    fn accessors_return_none_for_wrong_variant() {
        let v = KdlValue::Null(sp());
        assert!(v.is_null());
        assert!(!v.is_string());
        assert_eq!(v.as_str(), None);
        assert_eq!(v.as_bool(), None);
        assert_eq!(v.as_integer(), None);
        assert_eq!(v.as_float(), None);
        assert_eq!(v.lexeme(), None);
    }

    #[test]
    fn string_accessor_returns_content() {
        let v = KdlValue::String {
            span: sp(),
            value: "hi".into(),
            raw: false,
        };
        assert!(v.is_string());
        assert_eq!(v.as_str(), Some("hi"));
    }

    #[test]
    fn integer_lexeme_is_preserved() {
        let v = KdlValue::Integer {
            span: sp(),
            lexeme: "0xFF".into(),
            value: Some(255),
        };
        assert_eq!(v.lexeme(), Some("0xFF"));
        assert_eq!(v.as_integer(), Some(255));
    }

    #[test]
    fn display_uses_canonical_form() {
        let v = KdlValue::String {
            span: sp(),
            value: "a\"b".into(),
            raw: false,
        };
        assert_eq!(v.to_string(), r#""a\"b""#);
    }
}
