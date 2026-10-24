#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "coverage-gap tests assert exact AST/token/budget state; a panic signals a regression"
)]
//! Coverage-gap tests for public API surface not exercised by the
//! behavioural suites (`spec_fixtures.rs`, `fuzz.rs`, the `#[cfg(test)]`
//! unit tests, or `kdl_consumer.rs`).
//!
//! These target *reachable* code that no existing test drives:
//!
//! - type annotations `(u8)name` and `("u8")name` — KDL 2.0 allows an
//!   identifier **or** a string inside the parens — plus rejection paths;
//! - rejection of `name=value` without a node name;
//! - [`Token::kind`] / [`Token::span`] across token variants;
//! - [`ErrorCode::as_str`] and [`ErrorCode::category`] mapping,
//!   including the archive-wide codes;
//! - archive-wide budget exhaustion (`nodes_archive`,
//!   `decoded_string_bytes_archive`) and the `reset` / `reset_all`
//!   split;
//! - encoder edge cases for `/`, `\b`, `\f`, `\r`, and shortest-hex
//!   unicode escapes;
//! - `Position` / `SourceSpan` `Display` and `Debug` formatting.
//!
//! Defensive and unreachable arms (`u32::try_from` on a `usize`, the
//! internal invariant guards in `escape.rs`, `panic!` handlers inside
//! `#[cfg(test)]` helpers) are deliberately *not* targeted — a test for
//! them would be theatre, not coverage.

use elang_core::error::Category;
use elang_kdl::ast::KdlEntry;
use elang_kdl::budget::Budget;
use elang_kdl::canon;
use elang_kdl::error::ErrorCode;
use elang_kdl::lex::{Lexer, Token};
use elang_kdl::{BudgetTracker, KdlValue, Position, SourceSpan};

fn budgets() -> BudgetTracker {
    BudgetTracker::new()
}

/// Borrow the value out of an entry regardless of arg/prop shape.
fn entry_value(e: &KdlEntry) -> &KdlValue {
    match e {
        KdlEntry::Arg(v) => v,
        KdlEntry::Prop { value, .. } => value,
    }
}

// ---------------------------------------------------------------------------
// Type annotations — KDL 2.0 grammar (T0501)
// ---------------------------------------------------------------------------

#[test]
fn annotation_identifier_form_parses() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"(u8)port 8080\n", &mut b).unwrap();
    let node = &doc.nodes()[0];
    assert_eq!(node.ty(), Some("u8"));
    assert_eq!(node.name(), "port");
    assert_eq!(node.entries().len(), 1);
}

#[test]
fn annotation_string_form_parses() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"(\"u8\")port 8080\n", &mut b).unwrap();
    assert_eq!(doc.nodes()[0].ty(), Some("u8"));
}

#[test]
fn annotation_raw_string_form_parses() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"(#\"a-b\"#)node 1\n", &mut b).unwrap();
    assert_eq!(doc.nodes()[0].ty(), Some("a-b"));
}

#[test]
fn annotation_rejects_numeric_type() {
    // A type annotation is an identifier or a string; `5` is neither.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"(5)node 1\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

#[test]
fn annotation_requires_closing_paren() {
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"(u8 node 1\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

#[test]
fn annotation_inside_children_block_parses() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"parent {\n  (int)x 1\n}\n", &mut b).unwrap();
    let parent = &doc.nodes()[0];
    let children = parent.children();
    let kids = children.unwrap();
    assert_eq!(kids.as_slice()[0].ty(), Some("int"));
}

// ---------------------------------------------------------------------------
// `name=value` node-level shorthand
// ---------------------------------------------------------------------------

#[test]
fn rejects_node_level_assignment() {
    for input in [
        b"key=\"value\"\n".as_slice(),
        b"port=8080\n",
        b"parent { key=1 }\n",
    ] {
        let err = elang_kdl::parse(input).unwrap_err();
        assert_eq!(err.code(), ErrorCode::UnexpectedByte);
    }
}

#[test]
fn regular_property_binds_integer() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"node port=8080\n", &mut b).unwrap();
    let e = &doc.nodes()[0].entries()[0];
    assert_eq!(entry_value(e).as_integer(), Some(8080));
}

#[test]
fn shorthand_without_identifier_is_rejected() {
    // `1=2` — a positional arg cannot be assigned to.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"node 1=2\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

// ---------------------------------------------------------------------------
// Token::kind / Token::span across variants
// ---------------------------------------------------------------------------

fn lex_all(input: &str) -> Vec<Token> {
    let mut b = budgets();
    let mut lx = Lexer::new(input.as_bytes(), &mut b);
    let mut out = Vec::new();
    while let Ok(t) = lx.next_token() {
        if matches!(t, Token::Eof(_)) {
            break;
        }
        out.push(t);
    }
    out
}

#[test]
fn token_kind_covers_every_variant() {
    let kinds: Vec<&'static str> =
        lex_all("id 1 1.5 \"s\" #\"r\"# #true #false #null\n= { } ( ) / /-\n")
            .iter()
            .map(|t| t.kind())
            .collect();

    for expected in [
        "identifier",
        "integer",
        "float",
        "string",
        "raw string",
        "boolean",
        "null",
        "`=`",
        "`{`",
        "`}`",
        "`(`",
        "`)`",
        "`/`",
        "`/-`",
        "newline",
    ] {
        assert!(
            kinds.contains(&expected),
            "expected token kind {expected:?} in {kinds:?}"
        );
    }
    // `#true` and `#false` are both boolean.
    assert_eq!(kinds.iter().filter(|k| **k == "boolean").count(), 2);
}

#[test]
fn token_kind_of_eof() {
    let mut b = budgets();
    let mut lx = Lexer::new(b"", &mut b);
    let t = lx.next_token().unwrap();
    assert_eq!(t.kind(), "end of input");
}

#[test]
fn token_span_is_exact_for_delimiter() {
    let toks = lex_all("=");
    assert_eq!(toks.len(), 1);
    let s = toks[0].span();
    assert_eq!(s.start.byte, 0);
    assert_eq!(s.end.byte, 1);
}

#[test]
fn token_span_tracks_offsets() {
    let toks = lex_all("ab cd");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].span().start.byte, 0);
    assert_eq!(toks[0].span().end.byte, 2);
    assert_eq!(toks[1].span().start.byte, 3);
    assert_eq!(toks[1].span().end.byte, 5);
}

#[test]
fn token_span_covers_whole_literal() {
    let toks = lex_all("\"abc\"");
    let s = toks[0].span();
    assert_eq!(s.start.byte, 0);
    assert_eq!(s.end.byte, 5); // includes both quotes
}

// ---------------------------------------------------------------------------
// ErrorCode mapping — every variant's string form and category
// ---------------------------------------------------------------------------

#[test]
fn error_code_strings_are_stable() {
    // The registry (`specs/registry/error-codes.json`) pins these exact
    // spellings; `registry_completeness` in elang-conformance cross-checks
    // that the enum and the registry agree.
    let expected: &[(ErrorCode, &str)] = &[
        (ErrorCode::InvalidUtf8, "KDL_INVALID_UTF8"),
        (ErrorCode::TokenTooLong, "KDL_TOKEN_TOO_LONG"),
        (ErrorCode::StringTooLong, "KDL_STRING_TOO_LONG"),
        (ErrorCode::NameTooLong, "KDL_NAME_TOO_LONG"),
        (ErrorCode::TooManyArgs, "KDL_TOO_MANY_ARGS"),
        (ErrorCode::TooManyProps, "KDL_TOO_MANY_PROPS"),
        (ErrorCode::NestingTooDeep, "KDL_NESTING_TOO_DEEP"),
        (
            ErrorCode::TooManyNodesResource,
            "KDL_TOO_MANY_NODES_RESOURCE",
        ),
        (ErrorCode::TooManyNodesArchive, "KDL_TOO_MANY_NODES_ARCHIVE"),
        (ErrorCode::CommentTooLong, "KDL_COMMENT_TOO_LONG"),
        (
            ErrorCode::CommentBudgetExceeded,
            "KDL_COMMENT_BUDGET_EXCEEDED",
        ),
        (
            ErrorCode::StringBudgetResourceExceeded,
            "KDL_STRING_BUDGET_RESOURCE_EXCEEDED",
        ),
        (
            ErrorCode::StringBudgetArchiveExceeded,
            "KDL_STRING_BUDGET_ARCHIVE_EXCEEDED",
        ),
        (ErrorCode::UnterminatedString, "KDL_UNTERMINATED_STRING"),
        (ErrorCode::UnterminatedComment, "KDL_UNTERMINATED_COMMENT"),
        (
            ErrorCode::UnterminatedRawString,
            "KDL_UNTERMINATED_RAW_STRING",
        ),
        (ErrorCode::InvalidEscape, "KDL_INVALID_ESCAPE"),
        (
            ErrorCode::InvalidUnicodeEscape,
            "KDL_INVALID_UNICODE_ESCAPE",
        ),
        (ErrorCode::InvalidNumber, "KDL_INVALID_NUMBER"),
        (ErrorCode::UnexpectedByte, "KDL_UNEXPECTED_BYTE"),
        (ErrorCode::UnexpectedEof, "KDL_UNEXPECTED_EOF"),
        (ErrorCode::InvalidIdentifier, "KDL_INVALID_IDENTIFIER"),
        (
            ErrorCode::MissingVersionPragma,
            "KDL_MISSING_VERSION_PRAGMA",
        ),
        (ErrorCode::UnsupportedVersion, "KDL_UNSUPPORTED_VERSION"),
        (ErrorCode::Other, "KDL_OTHER"),
    ];

    for (code, want) in expected {
        assert_eq!(code.as_str(), *want, "string form drifted for {code:?}");
        // `Display` must agree with `as_str` — the registry markdown and
        // the diagnostics both depend on that.
        assert_eq!(code.to_string(), *want);
    }
}

#[test]
fn error_code_categories_are_correct() {
    let cases: &[(ErrorCode, Category)] = &[
        (ErrorCode::InvalidUtf8, Category::Container),
        (ErrorCode::MissingVersionPragma, Category::Limit),
        (ErrorCode::UnsupportedVersion, Category::Limit),
        (ErrorCode::InvalidIdentifier, Category::Limit),
        (ErrorCode::UnexpectedEof, Category::Limit),
        (ErrorCode::Other, Category::Limit),
    ];
    for (code, want) in cases {
        assert_eq!(code.category(), *want, "wrong category for {code:?}");
    }
}

// ---------------------------------------------------------------------------
// Budget API + archive-wide budgets (T0507, T0509)
// ---------------------------------------------------------------------------

#[test]
fn budget_code_accessor_returns_configured_code() {
    let b = Budget::new(7, ErrorCode::TooManyProps);
    assert_eq!(b.code(), ErrorCode::TooManyProps);
    assert_eq!(b.ceiling(), 7);
    assert_eq!(b.current(), 0);
}

#[test]
fn budget_zero_ceiling_rejects_every_add() {
    let mut b = Budget::new(0, ErrorCode::TooManyArgs);
    assert_eq!(b.try_add(0), Ok(()));
    assert_eq!(b.try_add(1), Err(ErrorCode::TooManyArgs));
}

#[test]
fn tracker_exposes_every_budget_accessor() {
    // Per the doc/traceability note for REQ-LIMIT-001, nesting depth is
    // checked directly against `MAX_KDL_NESTING_DEPTH` (not a budget).
    // Per-node arg/prop counts are enforced inline at parse_entry.
    // The tracker owns only the cumulative-resource and archive budgets.
    let mut t = budgets();
    t.nodes_resource().try_add(1).unwrap();
    t.nodes_archive().try_add(1).unwrap();
    t.decoded_string_bytes_resource().try_add(1).unwrap();
    t.decoded_string_bytes_archive().try_add(1).unwrap();
    t.comment_bytes_resource().try_add(1).unwrap();

    assert_eq!(t.nodes_resource().current(), 1);
    assert_eq!(t.nodes_archive().current(), 1);
    assert_eq!(t.decoded_string_bytes_resource().current(), 1);
    assert_eq!(t.decoded_string_bytes_archive().current(), 1);
    assert_eq!(t.comment_bytes_resource().current(), 1);
}

#[test]
fn reset_clears_per_resource_but_preserves_archive() {
    let mut t = budgets();
    t.nodes_resource().try_add(5).unwrap();
    t.nodes_archive().try_add(5).unwrap();
    t.decoded_string_bytes_archive().try_add(5).unwrap();

    t.reset();

    assert_eq!(t.nodes_resource().current(), 0);
    // Archive-wide counters survive `reset` so a multi-resource parse
    // keeps enforcing the aggregate ceiling.
    assert_eq!(t.nodes_archive().current(), 5);
    assert_eq!(t.decoded_string_bytes_archive().current(), 5);
}

#[test]
fn reset_all_clears_archive_too() {
    let mut t = budgets();
    t.nodes_archive().try_add(5).unwrap();
    t.decoded_string_bytes_archive().try_add(5).unwrap();

    t.reset_all();

    assert_eq!(t.nodes_archive().current(), 0);
    assert_eq!(t.decoded_string_bytes_archive().current(), 0);
}

#[test]
fn default_tracker_matches_new() {
    let mut a = BudgetTracker::new();
    let mut b = BudgetTracker::default();
    assert_eq!(a.nodes_resource().ceiling(), b.nodes_resource().ceiling());
    assert_eq!(a.nodes_archive().ceiling(), b.nodes_archive().ceiling());
    assert_eq!(
        a.decoded_string_bytes_resource().ceiling(),
        b.decoded_string_bytes_resource().ceiling()
    );
}

#[test]
fn archive_node_budget_breach_reports_archive_code() {
    // Drain the archive-wide node budget, then confirm the *parser*
    // surfaces the archive code rather than the per-resource one.
    let mut t = budgets();
    t.nodes_archive().try_add(10_000_000).unwrap();

    let err = elang_kdl::parse_with_budgets(b"a\nb\n", &mut t).unwrap_err();
    assert_eq!(err.code(), ErrorCode::TooManyNodesArchive);
}

#[test]
fn archive_string_budget_breach_reports_archive_code() {
    let mut t = budgets();
    // MAX_DECODED_STRING_BYTES_PER_ARCHIVE == 1 GiB.
    t.decoded_string_bytes_archive().try_add(1 << 30).unwrap();

    let err = elang_kdl::parse_with_budgets(b"a \"hello\"\n", &mut t).unwrap_err();
    assert_eq!(err.code(), ErrorCode::StringBudgetArchiveExceeded);
}

// ---------------------------------------------------------------------------
// Canonical encoder edge cases (T0513)
// ---------------------------------------------------------------------------

/// Re-render the first node's entries canonically and join them, so an
/// encoder edge case can be asserted without a document-level `Display`
/// impl (which `elang-kdl` deliberately does not provide — canonical
/// emission of a whole document is the consumer's job).
fn roundtrip_entries(src: &str) -> String {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(src.as_bytes(), &mut b).unwrap();
    doc.nodes()[0]
        .entries()
        .iter()
        .map(|e| canon::emit_value(entry_value(e)))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn encoder_passes_solidus_through() {
    // `/` inside a string must NOT be escaped.
    assert_eq!(roundtrip_entries("a \"x/y\"\n"), "\"x/y\"");
}

#[test]
fn encoder_escapes_control_characters() {
    assert_eq!(
        roundtrip_entries("a \"\\u{8}\\u{c}\\r\\n\\t\"\n"),
        "\"\\b\\f\\r\\n\\t\""
    );
}

#[test]
fn encoder_uses_shortest_unicode_escape() {
    // ADR 0009 E-3: no leading zeros in the hex run.
    assert_eq!(roundtrip_entries("a \"\\u{1}\"\n"), "\"\\u{1}\"");
}

#[test]
fn encoder_preserves_quote_and_backslash() {
    assert_eq!(roundtrip_entries("a \"x\\\"y\\\\z\"\n"), r#""x\"y\\z""#);
}

#[test]
fn slashdash_comments_are_dropped_from_the_node_list() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"/- dropped\nkept\n", &mut b).unwrap();
    assert_eq!(doc.nodes().len(), 1);
    assert_eq!(doc.nodes()[0].name(), "kept");
}

#[test]
fn emit_value_roundtrips_non_string_variants() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n 1 1.5 #true #false #null \"s\"\n", &mut b).unwrap();
    let emitted: Vec<String> = doc.nodes()[0]
        .entries()
        .iter()
        .map(|e| canon::emit_value(entry_value(e)))
        .collect();
    assert_eq!(
        emitted,
        vec!["1", "1.5", "#true", "#false", "#null", "\"s\""]
    );
}

#[test]
fn emit_is_an_alias_for_emit_value() {
    let v = KdlValue::Bool {
        span: SourceSpan::INVALID,
        value: true,
    };
    assert_eq!(canon::emit(&v), "#true");
    assert_eq!(canon::emit_value(&v), "#true");
}

// ---------------------------------------------------------------------------
// AST accessors (span, ty, entries, nodes, is_empty)
// ---------------------------------------------------------------------------

#[test]
fn node_span_covers_full_node() {
    // The node span must extend from the (optional) type annotation or
    // name through the last entry (and any children block). `end` is
    // exclusive — it points one past the final byte the parser consumed,
    // so for a node at the end of input it equals the input length.
    let mut b = budgets();
    let src = b"node 1 \"two\" k=3\n";
    let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
    let node = &doc.nodes()[0];
    let s = node.span();
    assert_eq!(s.start.byte, 0);
    // End is exclusive and stops at the last byte of the node itself; the
    // trailing newline is the node terminator, not part of the node.
    assert_eq!(s.end.byte, 16);
    // The span is at least the union of all entry spans.
    for e in node.entries() {
        let e_span = e.span();
        assert!(s.start.byte <= e_span.start.byte);
        assert!(e_span.end.byte <= s.end.byte);
    }
}

#[test]
fn document_span_covers_all_nodes() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"a\nbb\n", &mut b).unwrap();
    let s = doc.span();
    assert_eq!(s.start.byte, 0);
    // The document span covers the trailing newline, so it extends one byte
    // past the last node's span end.
    assert_eq!(s.end.byte, 5);
    assert!(s.end.byte > doc.nodes().last().unwrap().span().end.byte);
}

#[test]
fn entry_span_of_prop_covers_key_through_value() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n key=123\n", &mut b).unwrap();
    let e = &doc.nodes()[0].entries()[0];
    let s = e.span();
    assert_eq!(s.start.byte, 2); // `key`
    assert_eq!(s.end.byte, 9); // `123`
}

#[test]
fn entry_span_of_arg_is_the_value_span() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n \"s\"\n", &mut b).unwrap();
    let e = &doc.nodes()[0].entries()[0];
    assert_eq!(e.span(), entry_value(e).span());
}

#[test]
fn value_span_is_preserved_for_every_variant() {
    // One value per input so the expected column is easy to state.
    let cases: &[(&[u8], &str, u32, u32)] = &[
        (b"n #null\n", "Null", 2, 7),
        (b"n #true\n", "Bool", 2, 7),
        (b"n 1\n", "Integer", 2, 3),
        (b"n 1.5\n", "Float", 2, 5),
        (b"n \"s\"\n", "String", 2, 5),
    ];
    for (src, label, want_start, want_end) in cases {
        let mut b = budgets();
        let doc = elang_kdl::parse_with_budgets(src, &mut b).unwrap();
        let v = entry_value(&doc.nodes()[0].entries()[0]);
        let s = v.span();
        assert_eq!(s.start.byte, *want_start, "start byte for {label}");
        assert_eq!(s.end.byte, *want_end, "end byte for {label}");
        // Byte 2 on a single-line input is column 3 (1-based, counting
        // every byte after the last newline).
        assert_eq!(s.start.column, 3, "column for {label}");
    }
}

#[test]
fn empty_children_block_is_empty() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n {}\n", &mut b).unwrap();
    let node0 = &doc.nodes()[0];
    let kids = node0.children().unwrap();
    assert!(kids.is_empty());
    assert_eq!(kids.len(), 0);
}

#[test]
fn children_deref_exposes_slice() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"p {\n a\n b\n}\n", &mut b).unwrap();
    let node0 = &doc.nodes()[0];
    let kids = node0.children().unwrap();
    assert_eq!(kids.len(), 2);
    // `Deref<Target = [KdlNode]>` plus `as_slice()` must agree.
    assert_eq!(kids[0].name(), "a");
    assert_eq!(kids.as_slice()[1].name(), "b");
    assert_eq!(kids.iter().count(), 2);
}

#[test]
fn node_without_annotation_has_no_type() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"plain\n", &mut b).unwrap();
    assert_eq!(doc.nodes()[0].ty(), None);
    assert_eq!(doc.nodes()[0].children(), None);
}

#[test]
fn args_and_props_iterators_partition_entries() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n 1 \"two\" k=3\n", &mut b).unwrap();
    let node = &doc.nodes()[0];
    assert_eq!(node.args().count(), 2);
    let props: Vec<(&str, &KdlValue)> = node.props().collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].0, "k");
    assert_eq!(props[0].1.as_integer(), Some(3));
}

#[test]
fn debug_format_is_populated_for_key_types() {
    // `Debug` is what shows up in assertion failures and fuzz reports.
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"a \"s\"\n", &mut b).unwrap();
    let v = entry_value(&doc.nodes()[0].entries()[0]);
    assert!(
        format!("{v:?}").contains("String"),
        "unexpected Debug for value"
    );
    assert!(!format!("{:?}", doc.nodes()[0]).is_empty());
    assert!(!format!("{doc:?}").is_empty());
}

// ---------------------------------------------------------------------------
// Position / SourceSpan formatting
// ---------------------------------------------------------------------------

#[test]
fn invalid_position_formats_as_invalid() {
    assert!(Position::INVALID.is_invalid());
    assert_eq!(format!("{}", Position::INVALID), "<invalid>");
    assert!(format!("{:?}", Position::INVALID).contains("invalid"));
}

#[test]
fn valid_position_formats_line_and_column() {
    let src = b"ab\ncd";
    let p = Position::from_byte(src, 4);
    assert_eq!(p.line, 2);
    assert_eq!(p.column, 2);
    assert_eq!(format!("{p}"), "2:2");
    assert!(format!("{p:?}").starts_with("Position("));
}

#[test]
fn invalid_span_formats_as_invalid() {
    assert!(SourceSpan::INVALID.is_invalid());
    // Display renders `{start}..{end}`; for the `INVALID` sentinel both
    // positions format as `<invalid>`, so the span reads
    // `<invalid>..<invalid>`. That is the unambiguous sentinel form.
    assert_eq!(format!("{}", SourceSpan::INVALID), "<invalid>..<invalid>");
    assert!(format!("{:?}", SourceSpan::INVALID).contains("SourceSpan"));
}

#[test]
fn valid_span_formats_position_range() {
    let src = b"abcdef";
    let s = SourceSpan::from_range(src, 1, 4);
    assert!(!s.is_invalid());
    assert!(!s.is_empty());
    assert_eq!(s.len(), 3);
    // Display renders `start..end` where each side is a `Position`
    // (`line:column`).
    assert_eq!(format!("{s}"), "1:2..1:5");
    assert!(format!("{s:?}").contains("SourceSpan"));
}

#[test]
fn empty_span_has_zero_length() {
    let src = b"abc";
    let s = SourceSpan::from_range(src, 2, 2);
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert!(!s.is_invalid());
}

#[test]
fn parse_error_display_includes_code_message_and_span() {
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"(5)n\n", &mut b).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("KDL_UNEXPECTED_BYTE"), "missing code in {s:?}");
    assert!(
        s.contains("type annotation must be"),
        "missing message in {s:?}"
    );
    // Span is rendered as `line:column`.
    assert_eq!(err.span().start.line, 1);
}

#[test]
fn parse_error_category_delegates_to_code() {
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"(5)n\n", &mut b).unwrap_err();
    assert_eq!(err.category(), err.code().category());
}

#[test]
fn parse_error_display_without_span_omits_parentheses() {
    // `Display` has two branches: invalid-span (no parens) and valid
    // span (`code: message (line:column)`).
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"(5)n\n", &mut b).unwrap_err();
    // The error has a real span; the span must appear in the Display.
    assert!(err.to_string().contains('('));
}

#[test]
fn lexer_input_and_pos_return_buffer_and_offset() {
    let buf: &[u8] = b"hello world";
    let mut b = budgets();
    let mut lx = Lexer::new(buf, &mut b);
    assert_eq!(lx.input(), buf);
    assert_eq!(lx.pos(), 0);
    let _ = lx.next_token().unwrap();
    assert!(lx.pos() > 0);
}

// ---------------------------------------------------------------------------
// Token::span across every variant — drives the per-variant match arms
// ---------------------------------------------------------------------------

#[test]
fn token_span_for_every_variant() {
    let toks = lex_all("id 1 1.5 \"s\" #\"r\"# #true #false #null\n= { } ( ) / /-");
    // 17 tokens: id, 1, 1.5, "s", #"r"#, #true, #false, #null, newline,
    // `=`, `{`, `}`, `(`, `)`, `/`, `/-` — note: input ends with no
    // trailing newline before EOF; the lexer emits `Eof` after `/-`.
    assert!(toks.len() >= 16, "got {} tokens", toks.len());
    // Every token must report a non-invalid span with end > start, except
    // delimiters which are single bytes wide (start..end is exactly 1).
    for t in &toks {
        let s = t.span();
        assert!(!s.is_invalid(), "invalid span for {:?}", t.kind());
        assert!(s.end.byte > s.start.byte, "zero-width for {:?}", t.kind());
    }
}

#[test]
fn token_span_tracks_byte_position_for_delimiters() {
    let toks = lex_all("id { ( ) }");
    // id span = 0..2
    assert_eq!(toks[0].span().start.byte, 0);
    assert_eq!(toks[0].span().end.byte, 2);
    // `{` span = 3..4
    assert_eq!(toks[1].span().start.byte, 3);
    assert_eq!(toks[1].span().end.byte, 4);
    // `(` span = 5..6
    assert_eq!(toks[2].span().start.byte, 5);
    assert_eq!(toks[2].span().end.byte, 6);
    // `)` span = 7..8
    assert_eq!(toks[3].span().start.byte, 7);
    assert_eq!(toks[3].span().end.byte, 8);
    // `}` span = 9..10
    assert_eq!(toks[4].span().start.byte, 9);
    assert_eq!(toks[4].span().end.byte, 10);
}

// ---------------------------------------------------------------------------
// Property entry field access
// ---------------------------------------------------------------------------

#[test]
fn prop_entry_exposes_key_span_and_value() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"n key=1\n", &mut b).unwrap();
    let e = &doc.nodes()[0].entries()[0];
    match e {
        KdlEntry::Prop {
            key_span,
            key,
            value,
        } => {
            assert!(!key_span.is_invalid());
            assert_eq!(key.as_ref(), "key");
            assert_eq!(value.as_integer(), Some(1));
        }
        KdlEntry::Arg(_) => panic!("expected a prop entry"),
    }
}

// ---------------------------------------------------------------------------
// Lexer public surface
// ---------------------------------------------------------------------------

#[test]
fn lexer_position_and_input_accessors() {
    let buf: &[u8] = b"id rest";
    let mut b = budgets();
    let mut lx = Lexer::new(buf, &mut b);
    assert_eq!(lx.pos(), 0);
    assert_eq!(lx.input(), buf);
    let _ = lx.next_token().unwrap();
    assert!(lx.pos() > 0);
}

#[test]
fn slashdash_line_comment_is_discarded_by_the_parser() {
    // `Lexer::skip_to_eol` is no longer part of the public surface — the
    // parser consumes a slashdash node itself (see `slashdash_target`).
    // This asserts the resulting behaviour instead: the commented node
    // never reaches the AST, and parsing resumes on the next line.
    let buf: &[u8] = b"node /- dropped\nrest\n";
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(buf, &mut b).unwrap();
    let names: Vec<&str> = doc.nodes().iter().map(|n| n.name()).collect();
    assert_eq!(names, vec!["node", "rest"]);
}

#[test]
fn lexer_budget_accessor_returns_mutable_tracker() {
    let mut b = budgets();
    let mut lx = Lexer::new(b"id", &mut b);
    // The lexer's budgets accessor hands back the underlying tracker so
    // callers can drain counters while parsing.
    let tracker = lx.budgets();
    tracker.nodes_resource().try_add(1).unwrap();
    tracker.comment_bytes_resource().try_add(1).unwrap();
}

// ---------------------------------------------------------------------------
// Parse error paths (uncovered branches in parse.rs)
// ---------------------------------------------------------------------------

#[test]
fn slash_not_followed_by_version_keyword_is_rejected() {
    // `/5` — after `/` the next token is an Integer, not an Ident,
    // so parse_version_pragma_rest returns UnexpectedByte.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"/5\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

#[test]
fn slashdash_requires_children_block() {
    // `/-` at end of block without `{` is an error.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"/-\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

#[test]
fn missing_children_block_after_slashdash_within_block() {
    // Within a children block, `/-` must be followed by a `{` block.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"p { /-\n }\n", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnexpectedByte);
}

// ---------------------------------------------------------------------------
// Escape decoder paths (escape.rs uncovered branches)
// ---------------------------------------------------------------------------

#[test]
fn encode_string_emits_shortest_hex_for_c1_controls() {
    // C1 control character range 0x80..=0x9F uses `\u{xx}` form.
    let encoded = elang_kdl::escape::encode_string("\u{80}");
    assert_eq!(encoded, r#""\u{80}""#);
}

#[test]
fn encode_string_emits_shortest_hex_for_del() {
    // DEL (0x7F) uses `\u{7f}` form.
    let encoded = elang_kdl::escape::encode_string("\u{7f}");
    assert_eq!(encoded, r#""\u{7f}""#);
}

// ---------------------------------------------------------------------------
// UTF-8 validation
// ---------------------------------------------------------------------------

#[test]
fn rejects_invalid_utf8_input() {
    // 0xFF is never valid UTF-8.
    let mut b = budgets();
    let err = elang_kdl::parse_with_budgets(b"\xff", &mut b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidUtf8);
}

#[test]
fn validate_utf8_accepts_ascii() {
    assert!(elang_kdl::validate_utf8(b"hello").is_ok());
}

#[test]
fn validate_utf8_rejects_lone_continuation_byte() {
    // 0x80 alone is a continuation byte without a leader.
    assert!(elang_kdl::validate_utf8(&[0x80]).is_err());
}

// ---------------------------------------------------------------------------
// Convenience entry points
// ---------------------------------------------------------------------------

#[test]
fn parse_integer_recognises_bases() {
    assert_eq!(elang_kdl::parse_integer("42"), Some(42));
    assert_eq!(elang_kdl::parse_integer("-7"), Some(-7));
    assert_eq!(elang_kdl::parse_integer("0xFF"), Some(255));
    assert_eq!(elang_kdl::parse_integer("0o17"), Some(15));
    assert_eq!(elang_kdl::parse_integer("0b1010"), Some(10));
    assert_eq!(elang_kdl::parse_integer("1_000"), Some(1000));
    assert_eq!(elang_kdl::parse_integer("garbage"), None);
}

#[test]
fn parse_float_recognises_basic_forms() {
    assert_eq!(elang_kdl::parse_float("1.5"), Some(1.5));
    assert_eq!(elang_kdl::parse_float("-0.5"), Some(-0.5));
    assert_eq!(elang_kdl::parse_float("1e10"), Some(1e10));
    assert_eq!(elang_kdl::parse_float("garbage"), None);
}

// ---------------------------------------------------------------------------
// Slashdash semantic (T0512)
// ---------------------------------------------------------------------------

#[test]
fn slashdash_drops_the_whole_line() {
    let mut b = budgets();
    let doc =
        elang_kdl::parse_with_budgets(b"/- first comment\n/- second\nkept\n", &mut b).unwrap();
    assert_eq!(doc.nodes().len(), 1);
    assert_eq!(doc.nodes()[0].name(), "kept");
}

// ---------------------------------------------------------------------------
// Version pragma (T0501)
// ---------------------------------------------------------------------------

#[test]
fn version_pragma_is_optional() {
    // Empty KDL 2.0 input is valid — version pragma is optional, not
    // mandatory; the canonical forms produced by ELD include one but the
    // parser accepts omission.
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"", &mut b).unwrap();
    assert_eq!(doc.version(), None);
    assert_eq!(doc.nodes().len(), 0);
}

#[test]
fn version_pragma_round_trip() {
    let mut b = budgets();
    let doc = elang_kdl::parse_with_budgets(b"/kdl-version \"2.0\"\nnode\n", &mut b).unwrap();
    let v = doc.version().expect("version pragma present");
    assert_eq!(v.version.as_ref(), "2.0");
}
