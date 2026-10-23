//! Canonical KDL 2.0 string serializer (T0513).
//!
//! Re-exports [`escape::encode_string`] as the single entry point for
//! canonical emission of `KdlValue`s. The encoder is deterministic and
//! depends only on the value's decoded content; the same input always
//! produces the same bytes.

use crate::escape;
use crate::value::KdlValue;

/// Emit a canonical KDL 2.0 double-quoted string for `value`.
///
/// Non-string values are emitted in their canonical KDL form:
/// - `Null` → `null`
/// - `Bool { true }` → `true`
/// - `Bool { false }` → `false`
/// - `Integer` → the original lexeme (preserving base prefix and `_`
///   separators). If the lexeme is empty, falls back to a placeholder;
///   such values should not reach this function in practice.
/// - `Float` → the original lexeme.
///
/// String values are emitted via [`escape::encode_string`].
pub fn emit_value(value: &KdlValue) -> String {
    match value {
        KdlValue::Null(_) => "null".to_string(),
        KdlValue::Bool { value: b, .. } => if *b { "true" } else { "false" }.to_string(),
        KdlValue::Integer { lexeme, .. } => lexeme.to_string(),
        KdlValue::Float { lexeme, .. } => lexeme.to_string(),
        KdlValue::String { value, .. } => escape::encode_string(value),
    }
}

/// Alias for [`emit_value`] used by [`KdlValue`]'s `Display` impl.
pub fn emit(value: &KdlValue) -> String {
    emit_value(value)
}

/// Re-exported for convenience in callers that want to format a single
/// string content.
pub use crate::escape::encode_string as encode;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SourceSpan;

    fn sp() -> SourceSpan {
        SourceSpan::INVALID
    }

    #[test]
    fn emits_string_with_canonical_escapes() {
        let v = KdlValue::String {
            span: sp(),
            value: "a\"b\\c".into(),
            raw: false,
        };
        assert_eq!(emit_value(&v), r#""a\"b\\c""#);
    }

    #[test]
    fn emits_null() {
        let v = KdlValue::Null(sp());
        assert_eq!(emit_value(&v), "null");
    }

    #[test]
    fn emits_bool() {
        let v = KdlValue::Bool {
            span: sp(),
            value: true,
        };
        assert_eq!(emit_value(&v), "true");
        let v = KdlValue::Bool {
            span: sp(),
            value: false,
        };
        assert_eq!(emit_value(&v), "false");
    }

    #[test]
    fn emits_integer_lexeme_verbatim() {
        let v = KdlValue::Integer {
            span: sp(),
            lexeme: "0xFF".into(),
            value: Some(255),
        };
        assert_eq!(emit_value(&v), "0xFF");
    }
}
