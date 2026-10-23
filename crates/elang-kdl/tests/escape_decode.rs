#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "decoder tests assert exact escape decoding; a panic signals a regression"
)]
//! Escape-sequence **decoding** tests.
//!
//! `spec_fixtures.rs` exercises the canonical *encoder*; `coverage_gaps.rs`
//! covers the round-trip. This file targets the input side: every escape
//! the decoder accepts, and the error paths it raises for malformed ones.
//!
//! `decode_string` lives in `escape.rs` and is reached through the lexer,
//! so these tests drive it the way a consumer would — by parsing source
//! that contains the escape, then reading the decoded value back.

use elang_kdl::ast::KdlEntry;
use elang_kdl::{BudgetTracker, KdlValue};

/// Borrow the value out of an entry regardless of arg/prop shape.
fn entry_value(e: &KdlEntry) -> &KdlValue {
    match e {
        KdlEntry::Arg(v) => v,
        KdlEntry::Prop { value, .. } => value,
    }
}

/// Parse `src` and return the first node's single string argument.
fn first_string(src: &[u8]) -> String {
    let mut b = BudgetTracker::new();
    let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
    entry_value(&doc.nodes()[0].entries()[0])
        .as_str()
        .unwrap()
        .to_string()
}

/// Parse `src` and expect an error, returning its code.
fn expect_err_code(src: &[u8]) -> elang_kdl::ErrorCode {
    let mut b = BudgetTracker::new();
    elang_kdl::parse_with_budgets(src, &mut b)
        .expect_err("expected a parse error")
        .code()
}

// ---------------------------------------------------------------------------
// Every escape the decoder accepts
// ---------------------------------------------------------------------------

#[test]
fn decodes_quote_and_backslash() {
    assert_eq!(first_string(b"a \"x\\\"y\\\\z\"\n"), "x\"y\\z");
}

#[test]
fn decodes_control_character_escapes() {
    assert_eq!(first_string(b"a \"\\b\\f\\n\\r\\t\"\n"), "\u{8}\u{c}\n\r\t");
}

#[test]
fn rejects_solidus_escape() {
    // KDL 2.0 string escapes do NOT include `\/` (that is JSON, not KDL).
    // A `\` followed by `/` is an unknown escape and must be rejected with
    // `InvalidEscape`.
    let mut b = BudgetTracker::new();
    let Err(err) = elang_kdl::parse_with_budgets(b"a \"x\\/y\"\n", &mut b) else {
        panic!("`\\/` must be rejected");
    };
    assert_eq!(err.code, elang_kdl::ErrorCode::InvalidEscape);
}

#[test]
fn decodes_short_unicode_escape() {
    assert_eq!(first_string(b"a \"\\u{1}\"\n"), "\u{1}");
}

#[test]
fn decodes_max_length_unicode_escape() {
    // Six hex digits is the KDL 2.0 maximum.
    assert_eq!(first_string(b"a \"\\u{10FFFF}\"\n"), "\u{10FFFF}");
}

#[test]
fn decodes_mixed_escape_sequence() {
    assert_eq!(first_string(b"a \"\\u{48}\\u{49}\\u{4A}\"\n"), "HIJ");
}

#[test]
fn raw_string_does_not_decode_escapes() {
    // A raw string keeps its content verbatim — `\` and `u{...}` are
    // literal bytes, not escapes.
    assert_eq!(first_string(b"a #\"\\u{1}\"#\n"), "\\u{1}");
}

#[test]
fn decoded_string_is_reencoded_canonically() {
    let mut b = BudgetTracker::new();
    // `\/` is not a KDL 2.0 escape, so use `/` directly. The shortest `\u{1}`
    // round-trips through the canonical emitter.
    let doc = elang_kdl::parse_with_budgets(b"a \"x/y\\u{1}\"\n", &mut b).unwrap();
    match &doc.nodes()[0].entries()[0] {
        elang_kdl::KdlEntry::Arg(v) => {
            // `Display` for KdlValue delegates to the canonical emitter.
            assert_eq!(v.to_string(), "\"x/y\\u{1}\"");
        }
        _ => panic!("expected an argument"),
    }
}

// ---------------------------------------------------------------------------
// Malformed escapes the decoder rejects
// ---------------------------------------------------------------------------

#[test]
fn rejects_trailing_backslash() {
    assert_eq!(
        expect_err_code(b"a \"x\\\n"),
        elang_kdl::ErrorCode::UnterminatedString
    );
}

#[test]
fn rejects_unicode_escape_without_brace() {
    assert_eq!(
        expect_err_code(b"a \"\\u41\"\n"),
        elang_kdl::ErrorCode::InvalidEscape
    );
}

#[test]
fn rejects_unicode_escape_with_bad_hex() {
    assert_eq!(
        expect_err_code(b"a \"\\u{ZZZ}\"\n"),
        elang_kdl::ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn rejects_unicode_escape_out_of_range() {
    // U+110000 is one past the Unicode maximum.
    assert_eq!(
        expect_err_code(b"a \"\\u{110000}\"\n"),
        elang_kdl::ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn rejects_unicode_escape_too_long() {
    // Seven hex digits exceeds the KDL 2.0 maximum of six.
    assert_eq!(
        expect_err_code(b"a \"\\u{0000041}\"\n"),
        elang_kdl::ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn rejects_unterminated_string() {
    assert_eq!(
        expect_err_code(b"a \"unclosed\n"),
        elang_kdl::ErrorCode::UnterminatedString
    );
}

#[test]
fn rejects_unterminated_raw_string() {
    assert_eq!(
        expect_err_code(b"a #\"unclosed\n"),
        elang_kdl::ErrorCode::UnterminatedRawString
    );
}

// ---------------------------------------------------------------------------
// Structural error paths in the lexer / parser
// ---------------------------------------------------------------------------

#[test]
fn rejects_unterminated_block_comment() {
    assert_eq!(
        expect_err_code(b"/* never closed\na\n"),
        elang_kdl::ErrorCode::UnterminatedComment
    );
}

#[test]
fn rejects_missing_version_pragma_with_mismatched_version() {
    assert_eq!(
        expect_err_code(b"/kdl-version \"1.0\"\na\n"),
        elang_kdl::ErrorCode::UnsupportedVersion
    );
}

#[test]
fn rejects_version_pragma_not_followed_by_newline() {
    assert_eq!(
        expect_err_code(b"/kdl-version \"2.0\" a\n"),
        elang_kdl::ErrorCode::UnexpectedByte
    );
}

#[test]
fn rejects_unknown_slash_directive() {
    assert_eq!(
        expect_err_code(b"/not-kdl-version\na\n"),
        elang_kdl::ErrorCode::UnexpectedByte
    );
}

#[test]
fn rejects_non_string_version_pragma() {
    assert_eq!(
        expect_err_code(b"/kdl-version 2\na\n"),
        elang_kdl::ErrorCode::UnexpectedByte
    );
}

#[test]
fn rejects_invalid_number_literals() {
    // A base prefix with no digits after it is a hard error.
    for src in [&b"a 0x\n"[..], &b"a 0b\n"[..], &b"a 0o\n"[..]] {
        assert_eq!(
            expect_err_code(src),
            elang_kdl::ErrorCode::InvalidNumber,
            "expected InvalidNumber for {:?}",
            String::from_utf8_lossy(src)
        );
    }
}

#[test]
fn unrepresentable_numeric_literals_are_preserved_not_rejected() {
    // KDL 2.0 permits literals the reader cannot evaluate — a float with
    // two decimal points, an exponent with no digits, a hex literal with
    // a bad digit. The parser preserves the lexeme verbatim and reports
    // `value: None` / `approx: None` rather than rejecting, so a
    // canonical-form validator downstream can make the policy call
    // (see `KdlValue`'s doc comment).
    for src in [&b"a 1.2.3\n"[..], &b"a 1e\n"[..], &b"a 0xg\n"[..]] {
        let mut b = BudgetTracker::new();
        let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
        let v = entry_value(&doc.nodes()[0].entries()[0]);
        // The exact source spelling survives.
        assert!(
            v.lexeme().is_some(),
            "expected a preserved lexeme for {:?}",
            String::from_utf8_lossy(src)
        );
    }
}

#[test]
fn identifiers_permit_kdl_punctuation() {
    // KDL 2.0 bare identifiers are permissive: `-`, `_`, `.` and `$` are
    // all legal identifier bytes. Only whitespace, KDL structural
    // punctuation and `/` terminate an identifier.
    for (src, want) in [
        (&b"a-b-c\n"[..], "a-b-c"),
        (&b"a_b\n"[..], "a_b"),
        (&b"a.b\n"[..], "a.b"),
        (&b"$a\n"[..], "$a"),
        (&b"a1\n"[..], "a1"),
        (&b"a-\n"[..], "a-"),
    ] {
        let mut b = BudgetTracker::new();
        let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
        assert_eq!(doc.nodes()[0].name(), want);
    }
}

#[test]
fn rejects_unexpected_end_of_input() {
    assert_eq!(
        expect_err_code(b"a {\n"),
        elang_kdl::ErrorCode::UnexpectedEof
    );
}

#[test]
fn rejects_nesting_beyond_the_limit() {
    // MAX_KDL_NESTING_DEPTH == 64; build 65 levels.
    let mut src = String::new();
    for _ in 0..65 {
        src.push_str("n {\n");
    }
    assert_eq!(
        expect_err_code(src.as_bytes()),
        elang_kdl::ErrorCode::NestingTooDeep
    );
}

// ---------------------------------------------------------------------------
// Value-shape guards
// ---------------------------------------------------------------------------

#[test]
fn null_and_bool_values_expose_their_accessors() {
    let mut b = BudgetTracker::new();
    let doc = elang_kdl::parse_with_budgets(b"n #null #true #false\n", &mut b).unwrap();
    let entries = doc.nodes()[0].entries();
    assert!(matches!(entry_value(&entries[0]), KdlValue::Null(_)));
    assert_eq!(entry_value(&entries[1]).as_bool(), Some(true));
    assert_eq!(entry_value(&entries[2]).as_bool(), Some(false));
    assert!(entry_value(&entries[0]).is_null());
    assert!(!entry_value(&entries[0]).is_string());
}

#[test]
fn integer_lexemes_are_preserved_for_every_base() {
    let cases: &[(&[u8], &str, Option<i128>)] = &[
        (b"n 42\n", "42", Some(42)),
        (b"n 0xFF\n", "0xFF", Some(255)),
        (b"n 0o17\n", "0o17", Some(15)),
        (b"n 0b1010\n", "0b1010", Some(10)),
        (b"n 1_000\n", "1_000", Some(1000)),
        (b"n -7\n", "-7", Some(-7)),
        (b"n +7\n", "+7", Some(7)),
    ];
    for (src, lexeme, value) in cases {
        let mut b = BudgetTracker::new();
        let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
        let v = entry_value(&doc.nodes()[0].entries()[0]);
        assert_eq!(v.lexeme(), Some(*lexeme), "lexeme for {:?}", *src);
        assert_eq!(v.as_integer(), *value, "value for {:?}", *src);
    }
}

#[test]
fn float_lexemes_are_preserved() {
    let cases: &[(&[u8], &str)] = &[
        (b"n 1.5\n", "1.5"),
        (b"n 1e10\n", "1e10"),
        (b"n -0.5\n", "-0.5"),
    ];
    for (src, lexeme) in cases {
        let mut b = BudgetTracker::new();
        let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
        let v = entry_value(&doc.nodes()[0].entries()[0]);
        assert_eq!(v.lexeme(), Some(*lexeme), "lexeme for {:?}", *src);
        assert!(matches!(v, KdlValue::Float { .. }), "not a float: {v:?}");
    }
}

#[test]
fn accessor_wrong_variant_returns_none() {
    let mut b = BudgetTracker::new();
    let doc = elang_kdl::parse_with_budgets(b"n 1\n", &mut b).unwrap();
    let v = entry_value(&doc.nodes()[0].entries()[0]);
    assert_eq!(v.as_str(), None);
    assert_eq!(v.as_bool(), None);
    assert_eq!(v.as_float(), None);
    assert!(!v.is_null());
    assert!(!v.is_string());
}
