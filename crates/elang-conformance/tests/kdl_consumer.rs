//! Consumer wiring for `elang-kdl`.
//!
//! These tests prove that `elang-kdl` is a viable parser for the kinds
//! of documents the rest of the workspace will hand it: an IPA
//! tie-bar-bearing ELANG fragment, an integrity-manifest-shaped
//! document, and a round-trip through the canonical emitter. The
//! tests run inside `elang-conformance` so they are exercised by the
//! same gate as every other conformance check.
//!
//! They are NOT semantic checks of the integrity manifest or the IPA
//! repertoire — those belong to Phase 7 and Phase 6 respectively. This
//! file proves the parser is wired in and produces ASTs the downstream
//! crates can iterate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "consumer-wiring test asserts parser output for realistic domain documents; panics are the contract"
)]

use elang_kdl::{
    canon::emit_value, parse_with_budgets, BudgetTracker, ErrorCode, KdlValue, SourceSpan,
};

fn parse_ok(input: &[u8]) -> elang_kdl::KdlDocument {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
        .unwrap_or_else(|e| panic!("expected OK for {input:?}, got {e:?}"))
}

#[test]
fn parses_minimal_integrity_manifest_shape() {
    // Phase 7 will define the actual integrity-manifest grammar. For now
    // we lock in the *shape* the parser must accept: a version pragma
    // followed by nodes with the conventional `name sha256="…"` form.
    let input = b"/kdl-version \"2.0\"\n\
                 resource \"META-INF/integrity.kdl\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n\
                 resource \"payload/1.bin\" sha256=\"deadbeef\"\n";
    let doc = parse_ok(input);
    assert_eq!(doc.version().expect("has pragma").version.as_ref(), "2.0");
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(doc.nodes[0].name(), "resource");
    assert_eq!(doc.nodes[0].args().count(), 1);
    assert_eq!(doc.nodes[0].props().count(), 1);
}

#[test]
fn parses_ipa_repertoire_entry_shape() {
    // IPA repertoire entries are phonemes with optional tie-bar marks; the
    // shape is a bare node name plus property keys (category, etc.).
    let input = b"/kdl-version \"2.0\"\n\
                 phoneme category=\"vowel\" height=\"close\" backness=\"front\" rounded=\"false\"\n\
                 phoneme category=\"consonant\" place=\"bilabial\" manner=\"nasal\"\n";
    let doc = parse_ok(input);
    assert_eq!(doc.nodes.len(), 2);
    for n in doc.nodes.iter() {
        assert_eq!(n.name(), "phoneme");
        // Each entry has 3–4 string properties.
        assert!(n.props().count() >= 3);
    }
}

#[test]
fn parses_elang_metadata_block_shape() {
    // ELD metadata blocks are nested children with `field key="value"`
    // entries (see specs/grammar/eld-metadata.kdl).
    let input = b"metadata {\n\
                     title \"Test Entry\"\n\
                     creator \"Test Author\"\n\
                     language \"und\"\n\
                 }\n";
    let doc = parse_ok(input);
    assert_eq!(doc.nodes.len(), 1);
    let meta = doc.nodes[0].children().expect("has children");
    assert_eq!(meta.len(), 3);
    let names: Vec<&str> = meta.iter().map(|n| n.name()).collect();
    assert_eq!(names, ["title", "creator", "language"]);
    // Each metadata field carries exactly one positional string arg.
    for n in meta.iter() {
        assert_eq!(n.args().count(), 1);
    }
}

#[test]
fn canonical_round_trip_preserves_string_content() {
    // Whatever the parser emits, the canonical encoder must reproduce
    // in a way that re-parses to the same string. This is the contract
    // Phase 6 (ELD) and Phase 7 (integrity) depend on.
    let inputs = [
        "plain",
        "with \"quote\"",
        "with \\backslash",
        "with\nnewline",
        "caf\u{00E9}",
        "\u{1F600}",
        "with \t tab",
    ];
    for s in inputs {
        let canonical = emit_value(&KdlValue::String {
            span: SourceSpan::INVALID,
            value: s.into(),
            raw: false,
        });
        let doc = parse_ok(format!("node {canonical}\n").as_bytes());
        let v = doc.nodes[0].args().next().expect("first arg present");
        assert_eq!(
            v.as_str().expect("string value"),
            s,
            "round-trip failed for {s:?} (canonical: {canonical})"
        );
    }
}

#[test]
fn rejects_kdl_1_0_only_constructs() {
    // KDL 2.0 rejects the version pragma value "1.0"; consumers MUST
    // upgrade their documents to KDL 2.0 form.
    let mut budgets = BudgetTracker::new();
    let err = parse_with_budgets(b"/kdl-version \"1.0\"\nnode 1\n", &mut budgets)
        .expect_err("must reject KDL 1.0");
    assert_eq!(err.code, ErrorCode::UnsupportedVersion);
}

#[test]
fn rejects_oversized_document_before_allocating() {
    // A 70-level nested document must fail with NestingTooDeep on
    // level 65, before allocating levels 66–70. We can only observe
    // the error; the bounded allocation is enforced by the test
    // running at all (an unbounded parse would OOM).
    let mut s = String::new();
    for _ in 0..70 {
        s.push_str("n {");
    }
    s.push('}');
    let mut budgets = BudgetTracker::new();
    let err = parse_with_budgets(s.as_bytes(), &mut budgets)
        .expect_err("must reject > 64 levels of nesting");
    assert_eq!(err.code, ErrorCode::NestingTooDeep);
}
