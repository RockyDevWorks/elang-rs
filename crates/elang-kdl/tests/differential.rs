//! Differential KDL 2.0 parsing tests (Phase 15 / Issue #25, T1518).
//!
//! Cross-check `elang-kdl` against the official `kdl` (kdl-rs) crate over a
//! corpus of KDL 2.0 spec inputs. Both parsers must agree on:
//!
//! 1. **Accept/reject verdict** for every input that does not exercise
//!    ELANG-only behaviour (per-resource budgets, the KDL 1.0 reject rule,
//!    KDL 2.0 strictness, etc.).
//!
//! 2. **Top-level node names and order** for inputs both accept. The
//!    canonical emitter is out of scope for this test — `kdl`'s formatter
//!    diverges from the ELANG canonical rules (ADR 0010) and that
//!    divergence is intentional.
//!
//! # Why differential testing
//!
//! `elang-kdl` is hand-rolled (ADR 0010) so that it can enforce
//! ELANG-specific budgets, fail closed on KDL 1.0 inputs, and emit the
//! canonical serializer specified in ADR 0009 E-3. None of those
//! constraints are checked by an independent implementation, so a
//! silent grammar drift would only be caught by the conformance corpus
//! (which is finite). Adding the `kdl` crate as a `[dev-dependency]`
//! gives us a live cross-check on accept/reject decisions and AST node
//! extraction — every test below runs both parsers and asserts they
//! agree.
//!
//! # Why this is not a tautology
//!
//! Both parsers target KDL 2.0 but with different feature sets. A bug
//! in either parser surfaces as a disagreement on the verdict or the
//! node sequence. The corpus here is deliberately drawn from KDL 2.0
//! spec shapes that *both* parsers should accept: bare nodes, type
//! annotations, multi-fence raw strings, slashdash, all four integer
//! bases, escape sequences, block comments, CRLF, and unicode
//! identifiers.
//!
//! Budget-exceeding inputs (deeply nested, very large token) are
//! deliberately **excluded** from the differential agreement
//! assertion: elang-kdl rejects them by design; `kdl` accepts them.
//! The asymmetry is the whole point of ADR 0010.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "differential harness; panics surface accept/reject disagreements with the reference parser"
)]

use std::str::FromStr;

/// A KDL 2.0 input fixture paired with its expected accept/reject verdict.
///
/// `both_accept = true` → both parsers must accept and produce the same
/// top-level node sequence (after `kdl-rs`'s quote stripping).
///
/// `both_accept = false, elang_only_accept = true` → elang-kdl accepts
/// (the KDL 2.0 spec is followed) but the reference parser rejects
/// (because the reference parser doesn't implement that surface, e.g.
/// `/kdl-version "2.0"` in `kdl-rs` v6). The verdict is *not* a
/// disagreement — it is documented expected behaviour.
///
/// `both_accept = false, elang_only_accept = false` → both parsers must
/// reject (KDL 1.0 construct, KDL 2.0 syntax error, etc.).
struct DifferentialCase {
    /// Human-readable case id (used in failure messages).
    id: &'static str,
    /// KDL 2.0 source text.
    source: &'static str,
    /// `true` iff both parsers must accept the input.
    both_accept: bool,
    /// `true` iff the input is a spec-conformant KDL 2.0 input that
    /// elang-kdl accepts but the reference parser does not. Implies
    /// `both_accept == false`.
    elang_only_accept: bool,
}

const DIFFERENTIAL_CASES: &[DifferentialCase] = &[
    // ───────── KDL 2.0 spec shapes both parsers accept ─────────
    DifferentialCase {
        id: "bare-node",
        source: "node",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "node-with-string-arg",
        source: r#"node "value""#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "node-with-multiple-args",
        source: r#"node 1 2 3 "four""#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "node-with-property",
        source: r#"node key="value""#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "node-with-args-and-props",
        source: r#"node 1 key="value" 2 other=42"#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "children-block",
        source: "node {\n    child 1\n    sibling 2\n}",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "nested-children",
        source: "a {\n  b {\n    c 1\n  }\n  d 2\n}",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "multi-fence-raw-string-1",
        // Single-fence raw string as a node *argument* (the
        // canonical KDL 2.0 shape). Both parsers must accept and
        // expose `node` as the top-level node name.
        source: "node #\"raw value\"#",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "multi-fence-raw-string-2",
        // Double-fence raw string containing a single `"#` sequence.
        // Both parsers must accept the raw form verbatim.
        source: "node ##\"contains \\\"# quote\\\"##",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "unicode-escape",
        source: r#"node "a\u{1F600}b""#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "named-escape-tab",
        source: r#"node "col1\tcol2""#,
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "integer-decimal",
        source: "node 42",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "integer-hex",
        source: "node 0xdeadbeef",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "integer-octal",
        source: "node 0o755",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "integer-binary",
        source: "node 0b1010",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "signed-integer",
        source: "node -17",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "float",
        source: "node 1.5e10",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "boolean-true",
        source: "node #true",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "boolean-false",
        source: "node #false",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "null",
        source: "node #null",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "type-annotation",
        // KDL 2.0 grammar: a type annotation is `(name)name value`, with
        // the annotation preceding the node name. `(u32)42` alone is
        // invalid (annotation must precede an identifier), which is
        // why elang-kdl rejects it — and the differential corpus
        // matches that with the canonical form `(u32)port 8080`.
        source: r"(u32)port 8080",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "slashdash-skips-node",
        source: "/- skipped\nnode 1",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "line-comment",
        source: "// comment\nnode 1",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "block-comment",
        source: "/* comment */ node 1",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "crlf-line-endings",
        source: "node 1\r\nnode 2\r\n",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "empty-document",
        source: "",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "version-pragma-2",
        // KDL 2.0 version pragma: slash + `kdl-version "2.0"`.
        // elang-kdl accepts it and exposes `version()` of "2.0";
        // the reference `kdl-rs` v6.5.0 parser rejects the same
        // shape (the v6 series dropped pragma handling — see
        // https://github.com/kdl-org/kdl-rs). The corpus therefore
        // flags this as an elang-only accept, not a disagreement.
        source: "/kdl-version \"2.0\"\nnode 1",
        both_accept: false,
        elang_only_accept: true,
    },
    DifferentialCase {
        id: "unicode-identifier",
        source: "café 1",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "duplicate-property",
        // Both parsers accept duplicate property keys at the
        // surface-parser level — duplicate-property rejection is
        // a *schema-layer* concern (T0511, lives in `elang-epf` /
        // `elang-eld` / `elang-integrity`), not a property of the
        // surface parser. The differential harness therefore expects
        // both parsers to agree on accept.
        source: "node key=1 key=2",
        both_accept: true,
        elang_only_accept: false,
    },
    DifferentialCase {
        id: "kdl1-version-pragma-rejected",
        // `/kdl-version=1` is a KDL 1.0 pragma; ELANG rejects it
        // with KDL_UNSUPPORTED_VERSION and so does the reference
        // parser (KDL 2.0 default in v6).
        source: "/kdl-version=1\nnode 1",
        both_accept: false,
        elang_only_accept: false,
    },
];

/// Top-level node names as observed by `elang_kdl`.
fn elang_top_level_names(source: &str) -> Vec<String> {
    let Ok(doc) = elang_kdl::parse(source.as_bytes()) else {
        return Vec::new();
    };
    doc.nodes().iter().map(|n| n.name().to_string()).collect()
}

/// Top-level node names as observed by the reference `kdl` parser.
fn reference_top_level_names(source: &str) -> Result<Vec<String>, String> {
    match kdl::KdlDocument::from_str(source) {
        Ok(doc) => Ok(doc.nodes().iter().map(|n| n.name().to_string()).collect()),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// Run every case in [`DIFFERENTIAL_CASES`]. On mismatch, the test
/// fails with the case id and both verdicts so a regression in
/// either parser is attributed unambiguously.
#[test]
fn differential_accept_reject_matches_reference_parser() {
    for case in DIFFERENTIAL_CASES {
        let elang_ok = elang_kdl::parse(case.source.as_bytes()).is_ok();
        let reference_result = reference_top_level_names(case.source);

        if case.both_accept {
            assert!(
                elang_ok,
                "case `{}`: elang-kdl rejected an input the reference parser accepts (source: `{}`)",
                case.id,
                case.source,
            );
            let reference_ok = reference_result.is_ok();
            assert!(
                reference_ok,
                "case `{}`: reference parser rejected `{}`: {:?}",
                case.id,
                case.source,
                reference_result.err(),
            );
        } else if case.elang_only_accept {
            assert!(
                elang_ok,
                "case `{}`: elang-only-accept must be accepted by elang-kdl (source: `{}`)",
                case.id, case.source,
            );
            assert!(
                reference_result.is_err(),
                "case `{}`: elang-only-accept is no longer elang-only — reference parser accepted `{}`",
                case.id,
                case.source,
            );
        } else {
            assert!(
                !elang_ok,
                "case `{}`: elang-kdl accepted an input both parsers must reject (source: `{}`)",
                case.id, case.source,
            );
            assert!(
                reference_result.is_err(),
                "case `{}`: reference parser accepted `{}` — corpus disagrees with KDL 2.0 spec",
                case.id,
                case.source,
            );
        }
    }
}

/// For inputs both parsers accept, assert that the top-level node
/// names and their order match. Property names, argument values,
/// type annotations and children are not compared here — only the
/// structural accept/reject and the top-level node sequence, which
/// is what every schema layer in ELANG consumes.
#[test]
fn differential_node_names_match_reference_parser() {
    for case in DIFFERENTIAL_CASES.iter().filter(|c| c.both_accept) {
        let elang_names = elang_top_level_names(case.source);
        let reference_names = reference_top_level_names(case.source)
            .unwrap_or_else(|e| panic!("case `{}`: reference parser failed: {e}", case.id));

        // Both parsers may quote node names; the reference parser
        // preserves the quoting when the source quoted the name,
        // while elang-kdl normalises to the bare form. Strip
        // surrounding double quotes from the reference output so
        // we compare on the canonical (unquoted) name.
        let reference_normalised: Vec<String> = reference_names
            .iter()
            .map(|n| n.trim_matches('"').to_string())
            .collect();

        assert_eq!(
            elang_names,
            reference_normalised,
            "case `{}`: top-level node names disagree (source: `{}`)\n  elang-kdl:   {:?}\n  reference:   {:?}",
            case.id,
            case.source,
            elang_names,
            reference_normalised,
        );
    }
}

/// Smoke check: the differential harness covers at least one case per
/// major KDL 2.0 spec surface (value forms, comments, escape
/// sequences, raw strings, slashdash, version pragma). This catches a
/// regression where the corpus silently shrinks because someone
/// removed a case without updating the rationale.
#[test]
fn differential_corpus_covers_spec_surfaces() {
    let mut saw_bare_node = false;
    let mut saw_typed_value = false;
    let mut saw_raw_string = false;
    let mut saw_unicode_escape = false;
    let mut saw_slashdash = false;
    let mut saw_block_comment = false;
    let mut saw_version_pragma = false;
    let mut saw_reject = false;
    let mut saw_elang_only = false;

    for case in DIFFERENTIAL_CASES {
        if case.id == "bare-node" {
            saw_bare_node = true;
        }
        if case.id.starts_with("type-annotation") {
            saw_typed_value = true;
        }
        if case.id.starts_with("multi-fence-raw-string") {
            saw_raw_string = true;
        }
        if case.id.starts_with("unicode-escape") {
            saw_unicode_escape = true;
        }
        if case.id.starts_with("slashdash") {
            saw_slashdash = true;
        }
        if case.id.starts_with("block-comment") {
            saw_block_comment = true;
        }
        if case.id.starts_with("version-pragma") {
            saw_version_pragma = true;
        }
        if !case.both_accept {
            saw_reject = true;
        }
        if case.elang_only_accept {
            saw_elang_only = true;
        }
    }

    assert!(saw_bare_node, "corpus missing bare-node case");
    assert!(saw_typed_value, "corpus missing type-annotation case");
    assert!(saw_raw_string, "corpus missing multi-fence raw string case");
    assert!(saw_unicode_escape, "corpus missing Unicode escape case");
    assert!(saw_slashdash, "corpus missing slashdash case");
    assert!(saw_block_comment, "corpus missing block comment case");
    assert!(saw_version_pragma, "corpus missing version pragma case");
    assert!(saw_reject, "corpus missing at least one reject case");
    assert!(
        saw_elang_only,
        "corpus missing at least one elang-only-accept case"
    );
}
