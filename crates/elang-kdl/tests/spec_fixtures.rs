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
//! - Multi-fence raw strings (`#"…"#` / `##"…"##`).
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
fn kdl2_numeric_keywords_preserve_values_and_lexemes() {
    let doc = parse_ok(b"node #inf #-inf #nan\n");
    let args = doc.nodes[0].args().collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    assert_eq!(args[0].as_float(), Some(f64::INFINITY));
    assert_eq!(args[1].as_float(), Some(f64::NEG_INFINITY));
    assert!(args[2].as_float().unwrap().is_nan());
    for (value, keyword) in args.iter().zip(["#inf", "#-inf", "#nan"]) {
        assert_eq!(value.lexeme(), Some(keyword));
        assert_eq!(emit_value(value), keyword);
        let reparsed = parse_ok(format!("node {}\n", emit_value(value)).as_bytes());
        assert_eq!(
            reparsed.nodes[0].args().next().unwrap().lexeme(),
            Some(keyword)
        );
    }
}

#[test]
fn kdl2_value_bool_true() {
    let doc = parse_ok(b"node #true\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_bool(), Some(true));
}

#[test]
fn kdl2_value_bool_false() {
    let doc = parse_ok(b"node #false\n");
    assert_eq!(doc.nodes[0].args().next().unwrap().as_bool(), Some(false));
}

#[test]
fn kdl2_value_null() {
    let doc = parse_ok(b"node #null\n");
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
fn kdl2_vertical_tab_is_a_line_terminator() {
    let doc = parse_ok(b"first\x0bsecond\n");
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(doc.nodes[0].name(), "first");
    assert_eq!(doc.nodes[0].args().count(), 0);
    assert_eq!(doc.nodes[1].name(), "second");
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
fn kdl2_raw_string_one_hash() {
    let doc = parse_ok(b"node #\"hello\"#\n");
    assert_eq!(first_arg_str(&doc), "hello");
}

#[test]
fn kdl2_raw_string_one_hash_allows_bare_quote() {
    // `#"a"b"#` is the literal `a"b`: one bare `"` is legal because the
    // closing fence is `"#`, not `"`.
    let doc = parse_ok(b"node #\"a\"b\"#\n");
    assert_eq!(first_arg_str(&doc), "a\"b");
}

#[test]
fn kdl2_raw_string_two_hash_fence_keeps_inner_hash() {
    let doc = parse_ok(b"node ##\"contains \"# inside\"##\n");
    assert_eq!(first_arg_str(&doc), "contains \"# inside");
}

#[test]
fn kdl2_raw_string_no_escape_processing() {
    // `\n` inside a raw string is two literal bytes, not a newline.
    let doc = parse_ok(b"node #\"a\\nb\"#\n");
    assert_eq!(first_arg_str(&doc), "a\\nb");
}

#[test]
fn kdl2_raw_string_unicode_preserved() {
    let doc = parse_ok("node #\"\u{1F600}\u{00E9}\"#\n".as_bytes());
    assert_eq!(first_arg_str(&doc), "\u{1F600}\u{00E9}");
}

#[test]
fn kdl2_raw_string_unterminated_rejected() {
    assert_eq!(
        parse_err(b"node #\"never closed\n"),
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
    assert_eq!(emit_value(&KdlValue::Null(SourceSpan::INVALID)), "#null");
    assert_eq!(
        emit_value(&KdlValue::Bool {
            span: SourceSpan::INVALID,
            value: true
        }),
        "#true"
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

#[test]
fn kdl2_bare_identifiers_are_arguments_until_a_terminator() {
    let doc = parse_ok(b"first one two; second three\n");
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(
        doc.nodes[0]
            .args()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_eq!(doc.nodes[1].name(), "second");
    assert_eq!(parse_err(b"first {} second\n"), ErrorCode::UnexpectedByte);
    assert_eq!(
        parse_err(b"parent { first {} second }\n"),
        ErrorCode::UnexpectedByte
    );
}

#[test]
fn kdl2_rejects_top_level_close_brace() {
    for input in [b"}".as_slice(), b"node\n}", b"node {} }"] {
        assert_eq!(parse_err(input), ErrorCode::UnexpectedByte);
    }
}

#[test]
fn kdl2_slashdash_discards_one_multiline_construct() {
    let doc = parse_ok(b"/-hidden {\n child 1\n}; shown 2\nnext 3\n");
    assert_eq!(
        doc.nodes.iter().map(|n| n.name()).collect::<Vec<_>>(),
        ["shown", "next"]
    );
    let doc =
        parse_ok(b"n /-\n\"ignored\\nargument\" kept /-key=\"ignored\\nvalue\" other=ok\nnext\n");
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(first_arg_str(&doc), "kept");
    assert_eq!(
        doc.nodes[0].props().map(|(key, _)| key).collect::<Vec<_>>(),
        ["other"]
    );
    let doc = parse_ok(b"n /-{ hidden { nested 1 }\n} { shown 2 } /-{ hidden 3 }\nnext\n");
    assert_eq!(doc.nodes.len(), 2);
    let children = doc.nodes[0].children().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name(), "shown");
    assert_eq!(
        parse_err(b"n /-{ hidden } argument\n"),
        ErrorCode::UnexpectedByte
    );
}

#[test]
fn kdl2_slashdash_still_charges_budgets() {
    let mut budgets = BudgetTracker::new();
    let doc = parse_with_budgets(
        b"/-hidden \"ab\" { child #\"c\"# }\nn /-\"de\" /-{ child \"f\" }\n",
        &mut budgets,
    )
    .unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert_eq!(budgets.nodes_resource().current(), 4);
    assert_eq!(budgets.nodes_archive().current(), 4);
    assert_eq!(budgets.decoded_string_bytes_resource().current(), 6);
    assert_eq!(budgets.decoded_string_bytes_archive().current(), 6);
}

#[test]
fn kdl2_invalid_utf8_is_rejected_in_all_regions_and_entry_points() {
    for input in [
        b"// \xff\nn".as_slice(),
        b"/* \xff */n",
        b"n \xc2\n",
        b"/-n \xff\nnext",
    ] {
        assert_eq!(parse_err(input), ErrorCode::InvalidUtf8);
        assert_eq!(
            elang_kdl::parse(input).unwrap_err().code,
            ErrorCode::InvalidUtf8
        );
    }
}

#[test]
fn kdl2_unicode_whitespace_and_identifiers() {
    let doc = parse_ok("\u{feff}café\u{a0}東京\u{2003}clé=étoile\nⅣ\u{3000}你好\n".as_bytes());
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(doc.nodes[0].name(), "café");
    assert_eq!(first_arg_str(&doc), "東京");
    assert_eq!(doc.nodes[0].props().next().unwrap().0, "clé");
    assert_eq!(doc.nodes[1].name(), "Ⅳ");
    for c in [
        '\u{0}', '\u{7f}', '\u{200e}', '\u{202e}', '\u{2066}', '\u{feff}',
    ] {
        assert_eq!(
            parse_err(format!("n a{c}b\n").as_bytes()),
            ErrorCode::InvalidIdentifier
        );
    }
}

#[test]
fn kdl2_space_and_whitespace_escapes() {
    let doc = parse_ok("n \"a\\sb\\ \t\r\n\u{a0}\u{2003}c\\n\\\\d\"\n".as_bytes());
    assert_eq!(first_arg_str(&doc), "a bc\n\\d");
    assert_eq!(parse_err(b"n \"\\/\"\n"), ErrorCode::InvalidEscape);
}

#[test]
fn kdl2_node_spans_exclude_trivia_and_lookahead() {
    let input = b"  first bare key=\"v\"  \n (type)second {\n child 1\n}  \nthird\n";
    let doc = parse_ok(input);
    for (node, expected) in doc.nodes.iter().zip([
        b"first bare key=\"v\"".as_slice(),
        b"(type)second {\n child 1\n}",
        b"third",
    ]) {
        assert_eq!(node.span.slice(input), Some(expected));
        assert_eq!(
            node.span,
            SourceSpan::from_range(input, node.span.start.byte, node.span.end.byte)
        );
    }
    let child = &doc.nodes[1].children().unwrap()[0];
    assert_eq!(child.span.slice(input), Some(b"child 1".as_slice()));
}

#[test]
fn kdl2_quoted_and_raw_names_enforce_byte_limit() {
    for len in [256, 257] {
        let name = "é".repeat(128) + &"a".repeat(len - 256);
        for quoted in [format!("\"{name}\""), format!("#\"{name}\"#")] {
            for input in [format!("{quoted}\n"), format!("n {quoted}=1\n")] {
                if len == 256 {
                    let doc = parse_ok(input.as_bytes());
                    assert_eq!(doc.nodes.len(), 1);
                } else {
                    assert_eq!(parse_err(input.as_bytes()), ErrorCode::NameTooLong);
                }
            }
        }
    }
}

#[test]
fn kdl2_257_nodes_with_one_argument_each() {
    let doc = parse_ok("n bare\n".repeat(257).as_bytes());
    assert_eq!(doc.nodes.len(), 257);
    assert!(doc.nodes.iter().all(|n| n.args().count() == 1));
    let doc = parse_ok(format!("parent {{\n{} }}", "n \"arg\" k=1\n".repeat(257)).as_bytes());
    assert_eq!(doc.nodes[0].children().unwrap().len(), 257);
}

#[test]
fn kdl2_accepts_exactly_256_props_and_rejects_257() {
    let props = (0..256).map(|i| format!(" k{i}=1")).collect::<String>();
    assert_eq!(
        parse_ok(format!("n{props}\n").as_bytes()).nodes[0]
            .props()
            .count(),
        256
    );
    assert_eq!(
        parse_err(format!("n{props} extra=1\n").as_bytes()),
        ErrorCode::TooManyProps
    );
}

#[test]
fn kdl2_node_count_limits_reject_resource_and_archive_overflow() {
    for archive in [false, true] {
        for input in [b"n\n".as_slice(), b"/-n\n", b"parent { child }\n"] {
            let mut budgets = BudgetTracker::new();
            let budget = if archive {
                budgets.nodes_archive()
            } else {
                budgets.nodes_resource()
            };
            // Leave exactly one node available at the real configured ceiling.
            budget.try_add(budget.ceiling() - 1).unwrap();
            if input == b"parent { child }\n" {
                let err = parse_with_budgets(input, &mut budgets).unwrap_err();
                assert_eq!(
                    err.code,
                    if archive {
                        ErrorCode::TooManyNodesArchive
                    } else {
                        ErrorCode::TooManyNodesResource
                    }
                );
            } else {
                parse_with_budgets(input, &mut budgets).unwrap();
                let err = parse_with_budgets(b"n\n", &mut budgets).unwrap_err();
                assert_eq!(
                    err.code,
                    if archive {
                        ErrorCode::TooManyNodesArchive
                    } else {
                        ErrorCode::TooManyNodesResource
                    }
                );
            }
        }
    }
}

#[test]
fn kdl2_comment_limits_reject_individual_and_cumulative_overflow() {
    use elang_core::limits::MAX_COMMENT_BYTES;
    for line in [false, true] {
        let exact = if line {
            format!("//{}", "x".repeat(MAX_COMMENT_BYTES - 2))
        } else {
            format!("/*{}*/", "x".repeat(MAX_COMMENT_BYTES - 4))
        };
        parse_ok(exact.as_bytes());
        let over = exact.replacen('x', "xx", 1);
        assert_eq!(parse_err(over.as_bytes()), ErrorCode::CommentTooLong);
    }
    let mut budgets = BudgetTracker::new();
    let budget = budgets.comment_bytes_resource();
    budget.try_add(budget.ceiling() - 4).unwrap();
    parse_with_budgets(b"/**/", &mut budgets).unwrap();
    assert_eq!(
        parse_with_budgets(b"/**/", &mut budgets).unwrap_err().code,
        ErrorCode::CommentBudgetExceeded
    );
    budgets.reset();
    parse_with_budgets(b"/**/", &mut budgets).unwrap();
}

#[test]
fn kdl2_decoded_string_budgets_charge_decoded_bytes_and_reject_overflow() {
    for archive in [false, true] {
        for literal in ["\"\\u{e9}\"", "#\"é\"#", "é"] {
            let mut budgets = BudgetTracker::new();
            let budget = if archive {
                budgets.decoded_string_bytes_archive()
            } else {
                budgets.decoded_string_bytes_resource()
            };
            budget.try_add(budget.ceiling() - 2).unwrap();
            let input = format!("n {literal}\n");
            parse_with_budgets(input.as_bytes(), &mut budgets).unwrap();
            let err = parse_with_budgets(input.as_bytes(), &mut budgets).unwrap_err();
            assert_eq!(
                err.code,
                if archive {
                    ErrorCode::StringBudgetArchiveExceeded
                } else {
                    ErrorCode::StringBudgetResourceExceeded
                }
            );
            budgets.reset();
            if archive {
                assert_eq!(
                    parse_with_budgets(input.as_bytes(), &mut budgets)
                        .unwrap_err()
                        .code,
                    ErrorCode::StringBudgetArchiveExceeded
                );
            } else {
                parse_with_budgets(input.as_bytes(), &mut budgets).unwrap();
            }
            budgets.reset_all();
            parse_with_budgets(input.as_bytes(), &mut budgets).unwrap();
        }
    }
}

#[test]
fn kdl2_keyword_values_round_trip() {
    let doc = parse_ok(b"n #true #false #null\n");
    for value in doc.nodes[0].args() {
        let reparsed = parse_ok(format!("n {}\n", emit_value(value)).as_bytes());
        let actual = reparsed.nodes[0].args().next().unwrap();
        assert_eq!(emit_value(actual), emit_value(value));
    }
    // Keyword prefixes must not be accepted as complete keywords.
    assert_eq!(parse_err(b"n #truex\n"), ErrorCode::InvalidIdentifier);
}

#[test]
fn kdl2_line_comment_preserves_node_terminator() {
    for newline in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let doc = parse_ok(format!("first arg // comment{newline}second arg{newline}").as_bytes());
        assert_eq!(doc.nodes.len(), 2);
        assert!(doc.nodes.iter().all(|n| n.args().count() == 1));
    }
}

// ------------------------------------------------------------------------
// KDL 2.0 keyword regression guards
// ------------------------------------------------------------------------

#[test]
fn kdl2_rejects_bare_reserved_words() {
    for word in ["true", "false", "null", "inf", "nan"] {
        for input in [
            format!("n {word}\n"),
            format!("{word}\n"),
            format!("n {word}=1\n"),
        ] {
            assert_eq!(parse_err(input.as_bytes()), ErrorCode::InvalidIdentifier);
        }
        assert_eq!(
            first_arg_str(&parse_ok(format!("n \"{word}\"\n").as_bytes())),
            word
        );
    }
}

#[test]
fn kdl2_rejects_kdl1_prefixed_raw_strings() {
    assert!(elang_kdl::parse(b"n r#\"hello\"#\n").is_err());
}

#[test]
fn kdl2_keyword_prefix_is_not_a_keyword() {
    // `nullable`, `trueish` are ordinary identifiers, not malformed keywords.
    let doc = parse_ok(b"n nullable trueish falseness\n");
    let values: Vec<_> = doc.nodes[0].args().map(emit_value).collect();
    assert_eq!(values, vec!["\"nullable\"", "\"trueish\"", "\"falseness\""]);
}

#[test]
fn kdl2_keywords_round_trip_through_canonical_emitter() {
    // emit() must produce KDL 2.0 keywords that reparse identically.
    for (input, expected) in [
        (&b"n #true\n"[..], "#true"),
        (&b"n #false\n"[..], "#false"),
        (&b"n #null\n"[..], "#null"),
    ] {
        let doc = parse_ok(input);
        let emitted = emit_value(doc.nodes[0].args().next().unwrap());
        assert_eq!(emitted, expected);
        let reparsed = parse_ok(format!("n {emitted}\n").as_bytes());
        assert_eq!(
            emit_value(reparsed.nodes[0].args().next().unwrap()),
            emitted
        );
    }
}

#[test]
fn kdl2_rejects_kdl1_json_style_solidus_escape() {
    // `\/` is JSON, not KDL 2.0. Rejected as an unknown escape.
    let mut b = BudgetTracker::new();
    let Err(err) = elang_kdl::parse_with_budgets(b"n \"a\\/b\"\n", &mut b) else {
        panic!("`\\/` must be rejected under KDL 2.0");
    };
    assert_eq!(err.code, ErrorCode::InvalidEscape);
}

#[test]
fn kdl2_raw_fences_follow_published_grammar() {
    for literal in [r##"#"hello"#"##, r###"##"hello"##"###] {
        assert_eq!(
            first_arg_str(&parse_ok(format!("node {literal}\n").as_bytes())),
            "hello"
        );
    }
    assert!(elang_kdl::parse(b"node r#\"hello\"#\n").is_err());
}

#[test]
fn kdl2_positions_follow_every_newline_form() {
    for newline in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let input = format!("first{newline}  second{newline}  third");
        let doc = parse_ok(input.as_bytes());
        for (node, line) in doc.nodes.iter().zip(1..=3) {
            assert_eq!(node.span.start.line, line, "{newline:?}");
            assert_eq!(node.span.start.column, if line == 1 { 1 } else { 3 });
            assert_eq!(
                node.span.start,
                elang_kdl::Position::from_byte(input.as_bytes(), node.span.start.byte)
            );
        }
        let err = elang_kdl::parse(format!("first{newline}}}").as_bytes()).unwrap_err();
        assert_eq!(err.span.start.line, 2, "{newline:?}");
    }
}

#[test]
fn kdl2_single_line_raw_strings_reject_literal_newlines() {
    for newline in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        assert_eq!(
            parse_err(format!("n #\"a{newline}b\"#").as_bytes()),
            ErrorCode::UnterminatedRawString
        );
    }
    assert_eq!(
        parse_err(b"n #\"\"\"\ntext\n\"\"\"#"),
        ErrorCode::UnexpectedByte
    );
}
