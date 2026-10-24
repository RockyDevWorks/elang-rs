//! Semantic attribution guard for the `REQ-WHEN-*` coverage rows.
//!
//! # Why this exists
//!
//! The coverage table in [`crate::requirement_coverage_data`] is a
//! *hand-authored* claim: "this test proves this requirement". The
//! pre-existing meta-test
//! ([`crate::meta_tests::every_named_test_in_the_coverage_table_exists`])
//! only proves the named test **exists**. That is the weakest possible
//! check — it would happily accept a row that names a real test which
//! has nothing to do with the requirement.
//!
//! That gap was not theoretical. The original `REQ-WHEN-*` block
//! attributed 33 rows, and 17 of them failed the checks below — for
//! example `parse_atomic_with_no_arg_is_rejected` was filed under
//! `REQ-WHEN-001` (the AST-shape requirement) when it actually
//! exercises atomic-predicate arity (`REQ-WHEN-005`), and
//! `seventeen_atomic_predicates_is_rejected` was filed under
//! `REQ-WHEN-002` (one `when` per root) when it asserts
//! `WHEN_TOO_MANY_PREDICATES`, which belongs to `REQ-WHEN-008`.
//!
//! # What this guard checks
//!
//! Two independent, machine-checkable properties per `REQ-WHEN-*` row:
//!
//! 1. **Observable match** — the named test's source must mention at
//!    least one token drawn from the requirement's own vocabulary (the
//!    ELD predicate names and structural keywords the requirement
//!    governs). A test about canonical key ordering cannot prove a
//!    requirement about neighbour matching.
//!
//! 2. **Diagnostic-subset match** — every diagnostic code the test
//!    asserts must be one the requirement's own rejection path can
//!    produce. This is a *subset* check, not merely "some code is
//!    present": a body that asserts `WHEN_TOO_DEEP` while the row
//!    claims a requirement whose budget is arity is a mis-attribution
//!    even if the arity code also appears.
//!
//! A third, independent cross-check uses the `T10xx` task tags that the
//! original Phase 10 implementation left in the test bodies: a test
//! tagged `T1019` may only be filed under the requirement that owns
//! `T1019`.
//!
//! # Why the evidence table is hand-authored
//!
//! The `allowed_diagnostics` sets are read off the implementation's
//! error sites in `crates/elang-eld/src/when.rs`, and the `observables`
//! are the requirement's own normative vocabulary from
//! `.kiro/specs/elang-v0.1.0/requirements.md`. Both are properties of
//! the *specification and implementation*, not of the coverage table,
//! so the table cannot be used to validate itself.

// The whole module is test-only: it asserts things about the workspace's
// source tree and about the coverage table's claims, which is
// meaningless outside a test build.
#![cfg(test)]
#![allow(clippy::expect_used, reason = "tests are the spec")]

use crate::meta_tests::{mask_non_code, walk_rs, workspace_root};
use crate::requirement_coverage_data::{rows, CoverageKind, CoverageRow};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The evidence a `REQ-WHEN-*` requirement demands of its tests.
struct RequirementEvidence {
    /// The requirement id this evidence describes.
    requirement: &'static str,
    /// Diagnostic codes this requirement's rejection path can produce.
    ///
    /// A test filed under this requirement may assert any of these, and
    /// no others. An empty set means the requirement has no rejection
    /// path of its own (a MUST about shape, budget, or evaluation
    /// semantics has no error code), so the test must assert none.
    allowed_diagnostics: &'static [&'static str],
    /// Tokens drawn from the requirement's normative vocabulary.
    ///
    /// At least one must appear as a whole word in the test's name or
    /// body, treating underscores as separators. These are
    /// the ELD predicate names (`previous-phoneme`, `word-position`,
    /// …) and structural keywords (`group`, `or`, `when`) that the
    /// requirement governs.
    observables: &'static [&'static str],
}

/// The `REQ-WHEN-*` evidence table.
///
/// `REQ-WHEN-004` ("Reject explicit `and`/`not` and unknown predicates")
/// is a MUST NOT. Its tests are filed as `Negative` because they prove
/// the rejection happens — but note the polarity is inverted from a
/// plain MUST: the requirement is satisfied when the rejection occurs.
const WHEN_EVIDENCE: &[RequirementEvidence] = &[
    RequirementEvidence {
        requirement: "REQ-WHEN-001",
        allowed_diagnostics: &[],
        observables: &["Condition::Or", "Condition::Group", "Condition::Predicate"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-002",
        allowed_diagnostics: &[
            "WHEN_EMPTY",
            "WHEN_BAD_ARITY",
            "ELD_PHONEME_INVALID",
            "ELD_ALLOPHONE_INVALID",
        ],
        observables: &["when"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-003",
        allowed_diagnostics: &["WHEN_BAD_ARITY", "WHEN_GROUP_ARITY"],
        observables: &["group", "or"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-004",
        allowed_diagnostics: &["WHEN_UNKNOWN_NODE"],
        observables: &["explicit", "unknown"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-005",
        allowed_diagnostics: &["WHEN_PREDICATE_ARITY", "WHEN_BAD_ARITY"],
        observables: &["atomic"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-006",
        allowed_diagnostics: &["WHEN_BAD_VALUE"],
        observables: &[
            "word-position",
            "word_position",
            "WordPosition",
            "syllable-position",
            "syllable_position",
            "SyllablePosition",
        ],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-007",
        allowed_diagnostics: &[],
        observables: &[
            "previous-phoneme",
            "next-phoneme",
            "Category::When",
            "0361",
            "035C",
            "E000",
        ],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-008",
        allowed_diagnostics: &["WHEN_TOO_DEEP", "WHEN_TOO_MANY_PREDICATES"],
        observables: &[
            "predicate",
            "predicates",
            "nesting",
            "nested",
            "budget",
            "budgets",
        ],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-009",
        allowed_diagnostics: &["WHEN_REFERENCE_UNRESOLVED"],
        observables: &["reference", "references", "refs", "InventoryRefs"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-010",
        allowed_diagnostics: &[],
        observables: &["or", "group", "and", "siblings"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-011",
        allowed_diagnostics: &[],
        observables: &["contradictory", "duplicate", "initial", "final"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-012",
        allowed_diagnostics: &[],
        observables: &[
            "word-position",
            "word_position",
            "WordPosition",
            "initial",
            "medial",
            "final",
        ],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-013",
        allowed_diagnostics: &[],
        observables: &[
            "syllable-position",
            "syllable_position",
            "SyllablePosition",
            "onset",
            "nucleus",
            "coda",
        ],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-014",
        allowed_diagnostics: &[],
        observables: &["neighbour", "neighbours"],
    },
    RequirementEvidence {
        requirement: "REQ-WHEN-015",
        allowed_diagnostics: &[],
        observables: &[
            "eligible",
            "eligibility",
            "Eligibility",
            "priority",
            "realizations",
        ],
    },
];

/// The `REQ-WHEN-*` requirement → owning `T10xx` task mapping.
///
/// Read from the `Tasks` column of the `REQ-WHEN` table in
/// `.kiro/specs/elang-v0.1.0/requirements.md`. The task ids are the
/// original implementer's own provenance tags, so they are independent
/// of the coverage table.
const WHEN_TASK_OWNER: &[(&str, &[&str])] = &[
    ("REQ-WHEN-001", &["T1001"]),
    ("REQ-WHEN-002", &["T1002", "T1003"]),
    ("REQ-WHEN-003", &["T1004", "T1005"]),
    ("REQ-WHEN-004", &["T1006"]),
    ("REQ-WHEN-005", &["T1007"]),
    ("REQ-WHEN-006", &["T1008", "T1009"]),
    ("REQ-WHEN-007", &["T1010"]),
    ("REQ-WHEN-008", &["T1011", "T1012"]),
    ("REQ-WHEN-009", &["T1013", "T1014", "T1015", "T1016"]),
    ("REQ-WHEN-010", &["T1017", "T1018"]),
    ("REQ-WHEN-011", &["T1019"]),
    ("REQ-WHEN-012", &["T1020", "T1021"]),
    ("REQ-WHEN-013", &["T1022", "T1023"]),
    ("REQ-WHEN-014", &["T1024", "T1025"]),
    ("REQ-WHEN-015", &["T1026", "T1027", "T1028"]),
];

/// Find the source file that defines `test_name` in `crate_name`.
///
/// Scans the crate's `src/` and `tests/` trees for a `#[test]` function
/// with that name. The scan reuses the workspace's dependency-free
/// function scanner so a coverage row cannot be satisfied by a
/// zero-argument *helper* that happens to share a name.
fn file_defining(crate_name: &str, test_name: &str) -> Option<PathBuf> {
    let root = workspace_root()?;
    let crate_dir = root.join("crates").join(crate_name);
    for sub in ["src", "tests"] {
        let dir = crate_dir.join(sub);
        if !dir.exists() {
            continue;
        }
        for file in walk_rs(&dir) {
            if crate::meta_tests::collect_fn_names(&file)
                .iter()
                .any(|n| n == test_name)
            {
                return Some(file);
            }
        }
    }
    None
}

/// Extract the body of `fn <test_name>()` from `file`.
///
/// The body is the text between the function's opening and closing
/// braces. Brace counting runs over a *masked* copy of the source
/// (comments and string literals blanked) so that a brace inside a
/// literal cannot end the body early, but the returned text is the
/// **original** source — the assertions are what the guard must read.
fn extract_body(file: &Path, test_name: &str) -> Option<String> {
    let original = std::fs::read_to_string(file).ok()?;
    let masked = mask_non_code(&original);
    let bytes = masked.as_bytes();

    // Locate `fn <test_name>` at a token boundary.
    let needle = format!("fn {test_name}");
    let mut search_from = 0usize;
    let mut body_start = None;
    while let Some(offset) = masked[search_from..].find(&needle) {
        let at = search_from + offset;
        let before_ok = at == 0 || !crate::meta_tests::is_ident_byte(bytes[at - 1]);
        let after = at + needle.len();
        let after_ok = after >= bytes.len() || !crate::meta_tests::is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            body_start = Some(after);
            break;
        }
        search_from = after;
    }
    let start = body_start?;

    // Find the opening brace of the body: skip the (empty) parameter
    // list and any return type up to the first `{` at depth zero.
    //
    // Only parentheses and brackets are counted. Angle brackets are
    // deliberately NOT counted: a return type such as `-> Vec<u8>`
    // contains a `>` that would otherwise unbalance the depth counter
    // and make the body brace unreachable. Generics in a test
    // signature are vanishingly rare, and the brace-depth rule below
    // still finds the body correctly without them.
    let mut depth = 0i32;
    let mut i = start;
    let mut open = None;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b'{' if depth == 0 => {
                open = Some(i);
                break;
            }
            b';' if depth == 0 => return None,
            _ => {}
        }
        i += 1;
    }
    let open = open?;

    // Brace-match to the closing brace.
    let mut depth = 0i32;
    let mut j = open;
    let mut close = None;
    while j < bytes.len() {
        match bytes[j] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    let close = close?;

    Some(original[open..=close].to_string())
}

/// Locate a named test and return its source body.
fn test_body_for(crate_name: &str, test_name: &str) -> Option<String> {
    let file = file_defining(crate_name, test_name)?;
    extract_body(&file, test_name)
}

/// Extract the contents of every Rust string literal in `text`.
///
/// Comments are skipped so a diagnostic named in prose cannot be
/// mistaken for an assertion, and character literals are skipped so a
/// quote inside `'"'` cannot open a bogus string. Raw strings
/// (`r"…"`, `r#"…"#`) are handled.
fn string_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // Line comment.
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment (nesting allowed).
        if bytes[i..].starts_with(b"/*") {
            let mut depth = 1i32;
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw string: `r"…"` or `r#"…"#`.
        if bytes[i] == b'r' {
            let mut quote = i + 1;
            while bytes.get(quote) == Some(&b'#') {
                quote += 1;
            }
            if bytes.get(quote) == Some(&b'"') {
                let hashes = quote - i - 1;
                let terminator = format!("\"{}", "#".repeat(hashes));
                if let Some(end) = text[quote + 1..].find(&terminator) {
                    let content_start = quote + 1;
                    let content_end = content_start + end;
                    out.push(text[content_start..content_end].to_string());
                    i = content_end + terminator.len();
                    continue;
                }
            }
            i += 1;
            continue;
        }
        // Character literal — skip it so a quote inside cannot open a
        // bogus string. A lifetime (`'a`) is not a char literal.
        if bytes[i] == b'\'' {
            let is_char = bytes.get(i + 1) == Some(&b'\\')
                || bytes
                    .get(i + 1)
                    .is_some_and(|c| !crate::meta_tests::is_ident_byte(*c));
            if is_char {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'\'' {
                    if bytes[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                i = (j + 1).min(bytes.len());
                continue;
            }
            i += 1;
            continue;
        }
        // Ordinary string literal.
        if bytes[i] == b'"' {
            let content_start = i + 1;
            let mut j = content_start;
            let mut content = String::new();
            while j < bytes.len() {
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    content.push(bytes[j + 1] as char);
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    break;
                }
                content.push(bytes[j] as char);
                j += 1;
            }
            out.push(content);
            i = (j + 1).min(bytes.len());
            continue;
        }
        i += 1;
    }
    out
}

/// Whether `s` is a diagnostic code of the shape `WHEN_*` / `ELD_*`.
fn is_diagnostic_code(s: &str) -> bool {
    let rest = if let Some(r) = s.strip_prefix("WHEN_") {
        r
    } else if let Some(r) = s.strip_prefix("ELD_") {
        r
    } else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c == '_' || c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// The diagnostic codes asserted in `body`.
fn diagnostics_in(body: &str) -> BTreeSet<String> {
    string_literals(body)
        .into_iter()
        .filter(|s| is_diagnostic_code(s))
        .collect()
}

/// The `T10xx` task tags appearing in `body`.
///
/// Tags are written as `T1001`-style identifiers in the test body's
/// comments. Only the `T1` prefix is recognised so unrelated
/// identifiers cannot be mistaken for task tags.
fn task_tags_in(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i + 3 <= bytes.len() {
        if bytes[i] == b'T' && bytes[i + 1] == b'1' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j >= i + 4 {
                let before_ok = i == 0 || !crate::meta_tests::is_ident_byte(bytes[i - 1]);
                let after_ok = j >= bytes.len() || !crate::meta_tests::is_ident_byte(bytes[j]);
                if before_ok && after_ok {
                    out.insert(body[i..j].to_string());
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// The evidence record for `requirement`, if the requirement is a
/// `REQ-WHEN-*` one covered by [`WHEN_EVIDENCE`].
fn evidence_for(requirement: &str) -> Option<&'static RequirementEvidence> {
    WHEN_EVIDENCE.iter().find(|e| e.requirement == requirement)
}

/// Whether `token` occurs as a whole word; underscores separate words.
fn contains_word(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(at, _)| {
        let before_ok = text[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = text[at + token.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        before_ok && after_ok
    })
}

/// Check the complete semantic attribution decision for one coverage row.
fn check_attribution(row: &CoverageRow, body: Option<&str>) -> Result<(), String> {
    let Some(evidence) = evidence_for(row.requirement) else {
        return Err(format!(
            "{} :: {} — no evidence record",
            row.requirement, row.test
        ));
    };
    let Some(body) = body else {
        return Err(format!(
            "{} :: {} — test body not found in crate `{}`",
            row.requirement, row.test, row.crate_name
        ));
    };

    // 1. Observable match — the requirement's vocabulary must
    //    appear in the test's name or body as a complete word.
    let haystack = format!("{}\n{}", row.test, body);
    if !evidence
        .observables
        .iter()
        .any(|token| contains_word(&haystack, token))
    {
        return Err(format!(
            "{} :: {} — no vocabulary from the requirement appears in the \
             test (expected one of {:?})",
            row.requirement, row.test, evidence.observables
        ));
    }

    // 2. Diagnostic-subset match.
    let asserted = diagnostics_in(body);
    let allowed: BTreeSet<&str> = evidence.allowed_diagnostics.iter().copied().collect();
    let foreign: Vec<&String> = asserted
        .iter()
        .filter(|code| !allowed.contains(code.as_str()))
        .collect();
    if !foreign.is_empty() {
        return Err(format!(
            "{} :: {} — asserts diagnostic(s) {foreign:?} that belong to a \
             different requirement (this requirement allows {:?})",
            row.requirement, row.test, evidence.allowed_diagnostics
        ));
    }

    // 3. A Negative row on a requirement that has a rejection path
    //    must actually assert one of its codes — otherwise the row
    //    claims a rejection the test never demonstrates.
    if row.kind == CoverageKind::Negative
        && !evidence.allowed_diagnostics.is_empty()
        && asserted.is_empty()
    {
        return Err(format!(
            "{} :: {} — filed as a Negative row but asserts no diagnostic \
             code (expected one of {:?})",
            row.requirement, row.test, evidence.allowed_diagnostics
        ));
    }
    Ok(())
}

/// The task ids owned by `requirement`.
fn tasks_for(requirement: &str) -> Option<&'static [&'static str]> {
    WHEN_TASK_OWNER
        .iter()
        .find(|(r, _)| *r == requirement)
        .map(|(_, tasks)| *tasks)
}

/// The `REQ-WHEN-*` rows of the coverage table.
fn when_rows() -> Vec<&'static CoverageRow> {
    rows()
        .iter()
        .filter(|r| r.requirement.starts_with("REQ-WHEN-"))
        .collect()
}

#[test]
fn every_when_requirement_has_an_evidence_record() {
    // The evidence table and the coverage table must agree on which
    // requirements exist. A requirement with no evidence record cannot
    // be checked, and a requirement with an evidence record but no
    // rows is dead weight that will drift.
    let mut problems = Vec::new();
    for row in when_rows() {
        if evidence_for(row.requirement).is_none() {
            problems.push(format!("{}: no entry in WHEN_EVIDENCE", row.requirement));
        }
    }
    let covered: BTreeSet<&str> = when_rows().iter().map(|r| r.requirement).collect();
    for evidence in WHEN_EVIDENCE {
        if !covered.contains(evidence.requirement) {
            problems.push(format!(
                "{}: has an evidence record but no coverage rows",
                evidence.requirement
            ));
        }
    }
    problems.sort_unstable();
    assert!(
        problems.is_empty(),
        "REQ-WHEN evidence table and coverage table disagree:\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn every_when_coverage_row_matches_its_evidence() {
    // The core attribution check. Two properties per row:
    //
    //   1. the test mentions at least one token from the requirement's
    //      own vocabulary (observable match), and
    //   2. every diagnostic code the test asserts belongs to the
    //      requirement's own rejection path (diagnostic-subset match).
    //
    // Property 2 is a subset check on purpose: a body that asserts a
    // code belonging to a *different* requirement is mis-attributed
    // even when it also asserts a legitimate one.
    let mut failures = Vec::new();
    for row in when_rows() {
        let body = test_body_for(row.crate_name, row.test);
        if let Err(failure) = check_attribution(row, body.as_deref()) {
            failures.push(failure);
        }
    }
    failures.sort_unstable();
    failures.dedup();
    assert!(
        failures.is_empty(),
        "{} REQ-WHEN coverage row(s) fail the semantic attribution check:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn every_when_task_tag_agrees_with_its_requirement() {
    // Independent provenance cross-check. The Phase 10 implementation
    // left `T10xx` task tags in the test bodies. Those tags were
    // written by the implementer, independently of the coverage table,
    // so they can falsify a row that the vocabulary check accepts.
    //
    // Only tests that actually carry a tag are checked — a tag is
    // optional, and requiring one everywhere would be a different
    // (and much noisier) policy.
    let mut failures = Vec::new();
    for row in when_rows() {
        let Some(owned) = tasks_for(row.requirement) else {
            failures.push(format!(
                "{} :: {} — no task mapping",
                row.requirement, row.test
            ));
            continue;
        };
        let Some(body) = test_body_for(row.crate_name, row.test) else {
            continue;
        };
        let tags = task_tags_in(&body);
        if tags.is_empty() {
            continue;
        }
        let owned: BTreeSet<&str> = owned.iter().copied().collect();
        let foreign: Vec<&String> = tags
            .iter()
            .filter(|tag| !owned.contains(tag.as_str()))
            .collect();
        if !foreign.is_empty() {
            failures.push(format!(
                "{} :: {} — body is tagged {foreign:?}, which belong(s) to a \
                 different requirement (this requirement owns {owned:?})",
                row.requirement, row.test
            ));
        }
    }
    failures.sort_unstable();
    failures.dedup();
    assert!(
        failures.is_empty(),
        "{} REQ-WHEN coverage row(s) carry a task tag from another \
         requirement:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn the_attribution_guard_rejects_a_known_bad_attribution() {
    // Exercise the same complete decision as the real coverage rows.
    // Each rejection otherwise satisfies the preceding checks, so it
    // cannot pass merely because another part of the guard rejected it.
    let cases = [
        (
            "REQ-WHEN-001",
            "atomic_arity_rejection",
            CoverageKind::Negative,
            r#"{ let _ = Condition::Predicate; assert_eq!(e.code(), "WHEN_PREDICATE_ARITY"); }"#,
            Some("asserts diagnostic(s)"),
        ),
        (
            "REQ-WHEN-006",
            "word_position_value_rejection",
            CoverageKind::Negative,
            r#"{ assert_eq!(e.code(), "WHEN_BAD_VALUE"); }"#,
            None,
        ),
        (
            "REQ-WHEN-003",
            "canonical_order",
            CoverageKind::Negative,
            r#"{ assert_eq!(e.code(), "WHEN_GROUP_ARITY"); }"#,
            Some("no vocabulary"),
        ),
        (
            "REQ-WHEN-005",
            "atomic_arity_rejection_with_foreign_diagnostic",
            CoverageKind::Negative,
            r#"{ assert_eq!(e.code(), "WHEN_PREDICATE_ARITY"); assert_eq!(e.code(), "WHEN_TOO_DEEP"); }"#,
            Some("asserts diagnostic(s)"),
        ),
        (
            "REQ-WHEN-006",
            "word_position_rejection_without_diagnostic",
            CoverageKind::Negative,
            "{ assert!(result.is_err()); }",
            Some("asserts no diagnostic code"),
        ),
        (
            "REQ-WHEN-006",
            "word_position_acceptance_without_diagnostic",
            CoverageKind::Positive,
            "{ assert!(result.is_ok()); }",
            None,
        ),
        (
            "REQ-WHEN-015",
            "no_eligible_realizations",
            CoverageKind::Negative,
            "{ assert!(result.is_empty()); }",
            None,
        ),
        (
            "REQ-WHEN-006",
            "valid_value",
            CoverageKind::Positive,
            "{ let _ = WordPosition::Initial; }",
            None,
        ),
        (
            "REQ-WHEN-003",
            "format_error",
            CoverageKind::Positive,
            "for item in items { record(item); }",
            Some("no vocabulary"),
        ),
        (
            "REQ-WHEN-006",
            "word_position_rejects_bad_value_and_depth",
            CoverageKind::Negative,
            r#"assert_eq!(e.code(), "WHEN_BAD_VALUE"); assert_eq!(other.code(), "WHEN_TOO_DEEP");"#,
            Some("asserts diagnostic(s)"),
        ),
        (
            "REQ-WHEN-010",
            "and_evaluates_siblings",
            CoverageKind::Positive,
            "assert!(result);",
            None,
        ),
        (
            "REQ-WHEN-014",
            "missing_neighbour_is_false",
            CoverageKind::Negative,
            "assert!(!result);",
            None,
        ),
    ];
    for (requirement, test, kind, body, expected_error) in cases {
        let row = CoverageRow {
            requirement,
            crate_name: "synthetic",
            test,
            kind,
        };
        let result = check_attribution(&row, Some(body));
        match expected_error {
            Some(message) => assert!(
                result.as_ref().is_err_and(|error| error.contains(message)),
                "{test}: expected {message:?}, got {result:?}"
            ),
            None => assert!(result.is_ok(), "{test}: {result:?}"),
        }
    }
}

#[test]
fn observables_match_whole_words_with_underscores_as_separators() {
    for (haystack, token) in [
        ("or", "or"),
        ("or_rejects_invalid_children", "or"),
        ("nested_or", "or"),
        ("nested_or_accepts", "or"),
        ("order or", "or"),
        ("{ or { } }", "or"),
        ("word-position", "word-position"),
        ("truth_table_word_position", "word_position"),
        ("Condition::Or(children)", "Condition::Or"),
        ("Eligibility::Empty", "Eligibility"),
        ("é_or_é", "or"),
    ] {
        assert!(contains_word(haystack, token), "{haystack:?}: {token}");
    }
    for (haystack, token) in [
        ("", "or"),
        ("order", "or"),
        ("for", "or"),
        ("sorted", "or"),
        ("or2", "or"),
        ("2or", "or"),
        ("éor", "or"),
        ("oré", "or"),
        ("Condition::Order", "Condition::Or"),
        ("ineligible", "eligible"),
        ("Eligibility", "eligibility"),
    ] {
        assert!(!contains_word(haystack, token), "{haystack:?}: {token}");
    }
}

#[test]
fn observable_words_respect_boundaries() {
    for (text, token, expected) in [
        ("or", "or", true),
        ("or_with_cross_category_alternatives", "or", true),
        ("nested_or", "or", true),
        ("for error format", "or", false),
        ("format then or", "or", true),
        ("or2 2or", "or", false),
        ("éor oré", "or", false),
        ("whenever", "when", false),
        ("parse_when_block", "when", true),
        ("finally", "final", false),
        ("initial_and_final", "final", true),
        ("(Condition::Group)", "Condition::Group", true),
        ("Condition::Groups", "Condition::Group", false),
        (r#"word-position "initial""#, "word-position", true),
        ("word_position_is_initial", "word_position", true),
        ("Eligibility::Empty", "Eligibility", true),
    ] {
        assert_eq!(contains_word(text, token), expected, "{text:?}: {token:?}");
    }
}

#[test]
fn string_literal_extraction_ignores_comments_and_char_literals() {
    // The diagnostic scan reads string literals. If it also picked up
    // comments, a diagnostic merely *mentioned* in prose would count as
    // asserted and the subset check would lose its teeth.
    let body = r#"
        // "WHEN_TOO_DEEP" is mentioned in a comment.
        /* "WHEN_GROUP_ARITY" in a block comment. */
        let c = '"';
        let s = "WHEN_BAD_VALUE";
    "#;
    let found = diagnostics_in(body);
    assert_eq!(
        found.iter().cloned().collect::<Vec<_>>(),
        vec!["WHEN_BAD_VALUE".to_string()],
        "only the real string literal may count as an asserted diagnostic"
    );
}

#[test]
fn extract_body_handles_generics_and_return_types() {
    // The body extractor must cope with the breadth of test signature
    // shapes the workspace actually uses, plus a few shapes that are
    // rare but legal — a function with a return type, with a generic
    // parameter, and one whose parameter list contains a closure
    // generic. These cases broke an earlier version of the depth
    // counter that tracked `<` and `>` like parentheses.
    fn assert_body(source: &str, expected: &str) {
        let dir = std::env::temp_dir().join(format!(
            "elang-when-attr-{}-{}",
            std::process::id(),
            expected.len()
        ));
        std::fs::create_dir_all(&dir).expect("create probe dir");
        let path = dir.join("probe.rs");
        std::fs::write(&path, source).expect("write probe");
        let body = extract_body(&path, "target").expect("body found");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(body, expected, "body for `target`");
    }

    assert_body(
        "#[test] fn target() { let s = \"OK\"; }",
        "{ let s = \"OK\"; }",
    );
    assert_body(
        "#[test] fn target() -> &'static str { \"OK\" }",
        "{ \"OK\" }",
    );
    assert_body("#[test] fn target<T>() { let _: T; }", "{ let _: T; }");
    assert_body("#[test] fn target<F: Fn() -> i32>(f: F) { f() }", "{ f() }");
}
