//! KDL 2.0 spec fixtures + boundary tests (T0514, T0515).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "integration test asserts specific parser output for KDL 2.0 spec inputs; panics are the contract"
)]
//!
//! These tests exercise representative inputs drawn from the KDL 2.0
//! specification (https://github.com/kdl-org/kdl/blob/main/SPEC.md) and
//! the ELANG-specific boundary conditions listed in
//! `specs/registry/kdl-2.0-compatibility.md`:
//!
//! - Every documented KDL 2.0 value form (string / integer in all four
//!   bases / float / bool / null).
//! - Slashdash `/-` commenting out.
//! - Multi-line block comments.
//! - Multi-fence raw strings (`r#"…"#` / `r##"…"##`).
//! - Unicode escapes (`\u{…}`) and canonical encoder pass-through.
//! - Canonical double-quoted emitter round-trips through the parser.
//!
//! Byte-exact golden vectors live in `elang-conformance`; this file holds
//! the *grammar coverage* cases the goldens do not enumerate one by one.

use elang_kdl::{
    canon::emit_value, parse_with_budgets, BudgetTracker, ErrorCode, KdlValue, SourceSpan,
};

fn parse_ok(input: &[u8]) -> elang_kdl::KdlDocument {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets).unwrap_or_else(|e| {
        panic!(
            "expected OK for {:?}, got {e}",
            String::from_utf8_lossy(input)
        )
    })
}

fn parse_err(input: &[u8]) -> ErrorCode {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
        .err()
        .map(|e| e.code)
        .unwrap_or_else(|| panic!("expected Err for {:?}", String::from_utf8_lossy(input)))
}

/// First positional argument of the first node, as a string.
fn first_arg_str(doc: &elang_kdl::KdlDocument) -> String {
    let v = doc.nodes[0]
        .args()
        .next()
        .unwrap_or_else(|| panic!("node has no positional args"));
    v.as_str()
        .unwrap_or_else(|| panic!("first arg is not a string: {v:?}"))
        .to_string()
}

// --------------------------------------------------------------------
// KDL 2.0 value forms (spec §3)
// --------------------------------------------------------------------

#[test]
fn kdl2_value_string_double_quoted() {
    let doc = parse_ok(b"node \"hello world\"\n");
    assert_eq!(first_arg_str(&doc), "hello world");
}

#[test]
fn kdl2_value_integer_decimal() {
    let doc = parse_ok(b"node 42\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_integer(), Some(42));
}

#[test]
fn kdl2_value_integer_hex_preserves_lexeme() {
    let doc = parse_ok(b"node 0xDEAD_BEEF\n");
    let v = doc.nodes[0].args().next().unwrap();
    assert_eq!(v.as_integer(), Some(0xDEAD_BEEF));
    // The exact source lexeme must survive for canonical re-emission.
    assert_eq!(v.lexeme(), Some("0xDEAD_BEEF"));
}

#[test]
fn kdl2_value_integer_octal() {
    let doc = parse_ok(b"node 0o755\n");
    assert_eq!(
        doc.nodes[0].args().next().unwrap().as_integer(),
        Some(0o755)
    );
}

#[test]
fn kdl2_value_integer_binary() {
    let doc = parse_ok(b"node 0b1010_1010\n");
    assert_eq!(
        doc.nodes[0].args().next().unwrap().as_integer(),
        Some(0b1010_1010)
    );
}

#[test]
fn kdl2_value_integer_negative() {
    let doc = parse_ok(b"node -17\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_integer(), Some(-17));
}

#[test]
fn kdl2_value_float() {
    let doc = parse_ok(b"node 1.5e10\n");
    let v = doc.nodes[0].args().next().unwrap();
    assert_eq!(v.as_float(), Some(1.5e10));
    assert_eq!(v.lexeme(), Some("1.5e10"));
}

#[test]
fn kdl2_value_bool_true() {
    let doc = parse_ok(b"node true\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_bool(), Some(true));
}

#[test]
fn kdl2_value_bool_false() {
    let doc = parse_ok(b"node false\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_bool(), Some(false));
}

#[test]
fn kdl2_value_null() {
    let doc = parse_ok(b"node null\n");
    assert!(doc.nodes[0].args().next().unwrap().is_null());
}

// --------------------------------------------------------------------
// Comments and whitespace (T0512)
// --------------------------------------------------------------------

#[test]
fn kdl2_slashdash_comments_out_node() {
    // `/-node "a"` is a KDL 2.0 slashdash comment; the next line is the
    // only real node.
    let doc = parse_ok(b"/-node \"a\"\nnode \"b\"\n");
    assert_eq!(doc.nodes.len(), 1);
    assert_eq!(first_arg_str(&doc), "b");
}

#[test]
fn kdl2_slashdash_comments_out_child() {
    let doc = parse_ok(b"parent {\n    /-hidden 1\n    shown 2\n}\n");
    let children = doc.nodes[0].children().expect("has children");
    assert_eq!(children.len(), 1);
    assert_eq!(children.as_slice()[0].name(), "shown");
}

#[test]
fn kdl2_block_comment_with_inner_stars() {
    let doc = parse_ok(b"/*** foo ***/ node 1\n");
    assert_eq!(doc.nodes.len(), 1);
    assert_eq!(doc.nodes[0].name(), "node");
}

#[test]
fn kdl2_multi_line_block_comment() {
    let doc = parse_ok(b"/* line1\n   line2 */ node 1\n");
    assert_eq!(doc.nodes.len(), 1);
}

#[test]
fn kdl2_crlf_line_endings() {
    let doc = parse_ok(b"node 1\r\nnode 2\r\n");
    assert_eq!(doc.nodes.len(), 2);
}

#[test]
fn kdl2_vertical_tab_is_not_a_line_terminator() {
    // KDL 2.0 treats U+000B as whitespace, not as a line ending; both
    // nodes must still land on separate statements because the `\n`
    // separates them.
    let doc = parse_ok(b"node 1\nnode 2\n");
    assert_eq!(doc.nodes.len(), 2);
}

#[test]
fn kdl2_unterminated_block_comment_rejected() {
    assert_eq!(
        parse_err(b"node 1 /* unclosed\n"),
        ErrorCode::UnterminatedComment
    );
}

// --------------------------------------------------------------------
// Raw strings (multi-fence)
// --------------------------------------------------------------------

#[test]
fn kdl2_raw_string_no_hash() {
    let doc = parse_ok(b"node r\"hello\"\n");
    assert_eq!(first_arg_str(&doc), "hello");
}

#[test]
fn kdl2_raw_string_one_hash_allows_bare_quote() {
    // `r#"a"b"#` is the literal `a"b`: one bare `"` is legal because the
    // closing fence is `"#`, not `"`.
    let doc = parse_ok(b"node r#\"a\"b\"#\n");
    assert_eq!(first_arg_str(&doc), "a\"b");
}

#[test]
fn kdl2_raw_string_two_hash_fence_keeps_inner_hash() {
    let doc = parse_ok(b"node r##\"contains \"# inside\"##\n");
    assert_eq!(first_arg_str(&doc), "contains \"# inside");
}

#[test]
fn kdl2_raw_string_no_escape_processing() {
    // `\n` inside a raw string is two literal bytes, not a newline.
    let doc = parse_ok(b"node r\"a\\nb\"\n");
    assert_eq!(first_arg_str(&doc), "a\\nb");
}

#[test]
fn kdl2_raw_string_unicode_preserved() {
    let doc = parse_ok("node r#\"\u{1F600}\u{00E9}\"#\n".as_bytes());
    assert_eq!(first_arg_str(&doc), "\u{1F600}\u{00E9}");
}

#[test]
fn kdl2_raw_string_unterminated_rejected() {
    assert_eq!(
        parse_err(b"node r#\"never closed\n"),
        ErrorCode::UnterminatedRawString
    );
}

// --------------------------------------------------------------------
// Children blocks
// --------------------------------------------------------------------

#[test]
fn kdl2_nested_children() {
    let doc = parse_ok(b"outer { inner 1; inner 2 }\n");
    assert_eq!(doc.nodes.len(), 1);
    let children = doc.nodes[0].children().expect("has children");
    assert_eq!(children.len(), 2);
    assert_eq!(children.as_slice()[0].name(), "inner");
}

#[test]
fn kdl2_deeply_nested_children() {
    // Well inside MAX_KDL_NESTING_DEPTH (64): 40 levels must parse.
    let mut s = String::new();
    for _ in 0..40 {
        s.push_str("n {");
    }
    s.push_str("leaf 1");
    for _ in 0..40 {
        s.push('}');
    }
    s.push('\n');
    let doc = parse_ok(s.as_bytes());
    assert_eq!(doc.nodes.len(), 1);
}

#[test]
fn kdl2_children_with_args_and_props() {
    let doc = parse_ok(b"tag \"x\" key=\"v\" { child 1 }\n");
    let n = &doc.nodes[0];
    assert_eq!(n.name(), "tag");
    assert_eq!(n.args().count(), 1);
    assert_eq!(n.props().count(), 1);
    assert_eq!(n.children().expect("has children").len(), 1);
}

// --------------------------------------------------------------------
// Version pragma
// --------------------------------------------------------------------

#[test]
fn kdl2_version_pragma_accepted() {
    let doc = parse_ok(b"/kdl-version \"2.0\"\nnode 1\n");
    assert_eq!(doc.version().expect("has version").version.as_ref(), "2.0");
    assert_eq!(doc.nodes.len(), 1);
}

#[test]
fn kdl2_version_1_0_rejected() {
    // ELANG pins KDL 2.0 exclusively.
    assert_eq!(
        parse_err(b"/kdl-version \"1.0\"\nnode 1\n"),
        ErrorCode::UnsupportedVersion
    );
}

// --------------------------------------------------------------------
// String escapes (T0514)
// --------------------------------------------------------------------

#[test]
fn kdl2_string_unicode_escape() {
    let doc = parse_ok(b"node \"a\\u{1F600}b\"\n");
    assert_eq!(first_arg_str(&doc), "a\u{1F600}b");
}

#[test]
fn kdl2_string_named_escapes() {
    let doc = parse_ok(b"node \"a\\nb\\tc\\\"d\\\\e\"\n");
    assert_eq!(first_arg_str(&doc), "a\nb\tc\"d\\e");
}

#[test]
fn kdl2_string_c0_control_escape() {
    let doc = parse_ok(b"node \"a\\u{01}b\"\n");
    assert_eq!(first_arg_str(&doc), "a\x01b");
}

#[test]
fn kdl2_string_non_ascii_passes_through_decoding() {
    // The decoder must treat UTF-8 continuation bytes as text, not as C1
    // controls. This is the regression guard for the `encode_string`
    // byte-vs-char bug.
    let doc = parse_ok("node \"caf\u{00E9} \u{1F600}\"\n".as_bytes());
    assert_eq!(first_arg_str(&doc), "caf\u{00E9} \u{1F600}");
}

#[test]
fn kdl2_string_rejects_surrogate_escape() {
    assert_eq!(
        parse_err(b"node \"a\\u{D800}b\"\n"),
        ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn kdl2_string_rejects_empty_unicode_escape() {
    assert_eq!(
        parse_err(b"node \"a\\u{}b\"\n"),
        ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn kdl2_string_rejects_out_of_range_escape() {
    assert_eq!(
        parse_err(b"node \"\\u{110000}\"\n"),
        ErrorCode::InvalidUnicodeEscape
    );
}

#[test]
fn kdl2_string_rejects_unknown_escape() {
    assert_eq!(parse_err(b"node \"a\\qb\"\n"), ErrorCode::InvalidEscape);
}

#[test]
fn kdl2_string_unterminated_rejected() {
    assert_eq!(
        parse_err(b"node \"never closed\n"),
        ErrorCode::UnterminatedString
    );
}

// --------------------------------------------------------------------
// Invalid UTF-8 (T0502)
// --------------------------------------------------------------------

#[test]
fn kdl2_invalid_utf8_rejected_at_byte_boundary() {
    // 0xFF is never valid in UTF-8.
    assert_eq!(parse_err(b"node \"\xff\xfe\"\n"), ErrorCode::InvalidUtf8);
}

#[test]
fn kdl2_valid_multibyte_utf8_accepted() {
    let doc = parse_ok("node \"\u{00E9}\u{4E2D}\u{1F600}\"\n".as_bytes());
    assert_eq!(first_arg_str(&doc), "\u{00E9}\u{4E2D}\u{1F600}");
}

// --------------------------------------------------------------------
// Canonical emitter round-trip (T0513)
// --------------------------------------------------------------------

fn str_value(content: &str) -> KdlValue {
    KdlValue::String {
        span: SourceSpan::INVALID,
        value: content.into(),
        raw: false,
    }
}

#[test]
fn kdl2_canonical_emit_quoted_string_includes_quotes() {
    assert_eq!(emit_value(&str_value("a")), r#""a""#);
}

#[test]
fn kdl2_canonical_emit_escapes_backslash_and_quote() {
    assert_eq!(emit_value(&str_value("a\"b\\c")), r#""a\"b\\c""#);
}

#[test]
fn kdl2_canonical_emit_escapes_c0_controls() {
    // ADR 0009 E-3 + steering §10 require *no leading zeros* in \u{…}
    // escapes; the encoder emits the shortest valid hex run.
    assert_eq!(emit_value(&str_value("a\x01b")), r#""a\u{1}b""#);
}

#[test]
fn kdl2_canonical_emit_round_trips_through_parser() {
    for content in [
        "plain",
        "a\"b",
        "a\\b",
        "a\nb",
        "a\x01b",
        "caf\u{00E9}",
        "\u{1F600}",
    ] {
        let canonical = emit_value(&str_value(content));
        let input = format!("node {canonical}\n");
        let doc = parse_ok(input.as_bytes());
        assert_eq!(
            first_arg_str(&doc),
            content,
            "round-trip failed for {content:?} (canonical: {canonical})"
        );
    }
}

#[test]
fn kdl2_canonical_emit_is_idempotent() {
    // canon(value) re-parsed and re-emitted must produce identical bytes.
    for content in [
        "plain",
        "a\"b",
        "a\\b",
        "a\nb",
        "a\x01b",
        "caf\u{00E9}",
        "\u{1F600}",
    ] {
        let first = emit_value(&str_value(content));
        let reparsed = parse_ok(format!("node {first}\n").as_bytes());
        let value = reparsed.nodes[0].args().next().unwrap();
        assert_eq!(
            emit_value(value),
            first,
            "canon not idempotent for {content:?}"
        );
    }
}

#[test]
fn kdl2_canonical_emit_non_string_values() {
    assert_eq!(emit_value(&KdlValue::Null(SourceSpan::INVALID)), "null");
    assert_eq!(
        emit_value(&KdlValue::Bool {
            span: SourceSpan::INVALID,
            value: true
        }),
        "true"
    );
    assert_eq!(
        emit_value(&KdlValue::Integer {
            span: SourceSpan::INVALID,
            lexeme: "0xFF".into(),
            value: Some(255),
        }),
        "0xFF"
    );
}

// --------------------------------------------------------------------
// Budget enforcement (T0503–T0509)
// --------------------------------------------------------------------

#[test]
fn kdl2_rejects_nesting_beyond_64() {
    let mut s = String::new();
    for _ in 0..70 {
        s.push_str("n {");
    }
    s.push('}');
    assert_eq!(parse_err(s.as_bytes()), ErrorCode::NestingTooDeep);
}

#[test]
fn kdl2_rejects_more_than_256_args() {
    let mut s = String::from("n");
    for _ in 0..257 {
        s.push_str(" 1");
    }
    s.push('\n');
    assert_eq!(parse_err(s.as_bytes()), ErrorCode::TooManyArgs);
}

#[test]
fn kdl2_accepts_exactly_256_args() {
    let mut s = String::from("n");
    for _ in 0..256 {
        s.push_str(" 1");
    }
    s.push('\n');
    let doc = parse_ok(s.as_bytes());
    assert_eq!(doc.nodes[0].args().count(), 256);
}

#[test]
fn kdl2_rejects_more_than_256_props() {
    let mut s = String::from("n");
    for i in 0..257 {
        s.push_str(&format!(" k{i}=1"));
    }
    s.push('\n');
    assert_eq!(parse_err(s.as_bytes()), ErrorCode::TooManyProps);
}

#[test]
fn kdl2_rejects_name_longer_than_256_bytes() {
    let name = "a".repeat(257);
    let input = format!("{name} 1\n");
    assert_eq!(parse_err(input.as_bytes()), ErrorCode::NameTooLong);
}

#[test]
fn kdl2_rejects_multibyte_name_over_256_bytes() {
    // 86 × 3-byte chars = 258 bytes > 256, even though it is only 86 chars.
    let name = "\u{4E2D}".repeat(86);
    let input = format!("{name} 1\n");
    assert_eq!(parse_err(input.as_bytes()), ErrorCode::NameTooLong);
}

// --------------------------------------------------------------------
// Source spans (T0502)
// --------------------------------------------------------------------

#[test]
fn kdl2_error_span_points_at_offending_token() {
    let input = b"node 1\nnode 2\n\"unterminated\n";
    let mut budgets = BudgetTracker::new();
    let err = parse_with_budgets(input, &mut budgets).expect_err("must fail");
    assert_eq!(err.code, ErrorCode::UnterminatedString);
    // The failure is on line 3, so the reported line must be 3, not 1.
    assert_eq!(err.span.start.line, 3, "span: {:?}", err.span);
}

#[test]
fn kdl2_document_span_covers_whole_input() {
    let input = b"node 1\nnode 2\n";
    let doc = parse_ok(input);
    assert_eq!(doc.span.start.byte, 0);
    assert_eq!(doc.span.end.byte as usize, input.len());
}
