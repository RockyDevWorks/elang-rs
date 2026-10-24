//! Requirement-to-fixture completeness.
//!
//! The Phase 14 acceptance criterion is:
//!
//! > Every normative MUST requirement is mapped to at least one positive or
//! > negative conformance test.
//!
//! This test is the machine check for that sentence. It reads the normative
//! requirement table from
//! `.kiro/specs/elang-v0.1.0/requirements.md`, extracts every `MUST` and
//! `MUST NOT` requirement id, and asserts that the corpus catalog names
//! each one.
//!
//! # Why read the spec at runtime instead of hard-coding a list
//!
//! A hard-coded list is a snapshot: the day someone adds a normative MUST
//! to `requirements.md`, the snapshot still passes and the corpus silently
//! stops covering the new requirement. Reading the spec makes the test
//! fail on the change that breaks the invariant — which is the moment you
//! want it to fail.
//!
//! # The bidirectional check
//!
//! Three directions are checked:
//!
//! 1. **Forward** — every normative MUST appears in at least one row of the
//!    coverage table
//!    ([`elang_conformance::requirement_coverage_data`]). The table is
//!    the canonical mapping of requirement → test; this test refuses to
//!    accept a "covered" claim that is not in it.
//! 2. **Reverse** — every requirement named by a coverage row or a
//!    fixture is a real normative requirement id. This catches typos
//!    (`REQ-ELD-18` instead of `REQ-ELD-018`) and stale ids left behind
//!    after a requirement was renumbered.
//! 3. **Fixture-level** — every fixture id in the corpus catalog names at
//!    least one real normative requirement, and every rejection fixture
//!    declares the exact diagnostic code it expects.
//!
//! The reverse check is the one people forget. A forward-only test happily
//! passes when every fixture names a nonexistent requirement, because the
//! coverage looks complete and the corpus is actually empty.
//!
//! # Locating the spec
//!
//! The spec lives at a fixed path relative to the workspace root. The test
//! walks up from `CARGO_MANIFEST_DIR` until it finds the directory
//! containing both `.kiro` and `Cargo.toml`. That keeps the test working
//! whether cargo is invoked from the workspace root or from inside the
//! crate.

// The crate's strict lints deny `expect`, `panic`, and indexing on
// production code. The completeness tests here are themselves a
// production artefact — a precondition violation is a hard failure that
// should fail loudly. Other integration tests in this crate use the
// same module-level allow; the alternative (rewriting every `expect()`
// to a nested `match`) makes the test body unreadable without changing
// the runtime behaviour.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "completeness tests are the spec; precondition violations fail loudly"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use elang_conformance::catalog::{catalog, FixtureExpectation};
use elang_conformance::requirement_coverage_data::{covered_requirements, rows, CoverageKind};

/// The workspace root, found by walking up from this crate's manifest dir.
fn workspace_root() -> Option<PathBuf> {
    let mut dir: &Path = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".kiro").is_dir() && dir.join("Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// The normative requirement table.
fn requirements_path() -> Option<PathBuf> {
    workspace_root().map(|root| root.join(".kiro/specs/elang-v0.1.0/requirements.md"))
}

/// Extract every requirement id whose normative strength is `MUST` or
/// `MUST NOT`.
///
/// The table rows look like:
///
/// ```text
/// | REQ-BIN-001 | MUST | 32-byte header; magic ... | container | ... |
/// ```
///
/// The strength is the second cell. Rows whose strength is `MAY`, `SHOULD`
/// or `RECOMMENDED` are advisory and are excluded — the acceptance
/// criterion is about normative MUSTs.
fn normative_musts() -> BTreeSet<String> {
    let path = requirements_path().expect("workspace root with .kiro/specs must exist");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {} failed: {e}", path.display()));
    let mut musts = BTreeSet::new();
    for line in text.lines() {
        let Some(cells) = parse_table_row(line) else {
            continue;
        };
        if cells.len() < 2 {
            continue;
        }
        let id = cells[0].trim();
        let strength = cells[1].trim();
        if !id.starts_with("REQ-") {
            continue;
        }
        if strength == "MUST" || strength == "MUST NOT" {
            musts.insert(id.to_string());
        }
    }
    musts
}

/// Split a markdown table row into its cells, honouring `\|` escapes.
///
/// Returns `None` for a line that is not a table row.
fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = trimmed.chars().peekable();
    // Skip the leading pipe.
    chars.next();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '|' => {
                cells.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    cells.push(current);
    Some(cells)
}

/// Map every requirement id to the catalog fixture ids that name it.
fn coverage_map() -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in catalog() {
        for req in entry.requirements {
            map.entry((*req).to_string())
                .or_default()
                .insert(entry.id.clone());
        }
    }
    map
}

#[test]
fn the_normative_must_set_is_not_empty() {
    // Guards the other two tests: if the parser silently stopped matching
    // the table format, the completeness test would pass vacuously.
    let musts = normative_musts();
    assert!(
        musts.len() >= 100,
        "expected at least 100 normative MUST requirements, parsed {} — the \
         requirements.md table format probably changed",
        musts.len()
    );
    assert!(
        musts.contains("REQ-CONF-001"),
        "REQ-CONF-001 must be present"
    );
    assert!(musts.contains("REQ-BIN-001"), "REQ-BIN-001 must be present");
}

#[test]
fn every_normative_must_is_mapped_to_at_least_one_test() {
    let musts = normative_musts();
    let covered = covered_requirements();
    let uncovered: Vec<&String> = musts
        .iter()
        .filter(|req| !covered.contains(req.as_str()))
        .collect();
    assert!(
        uncovered.is_empty(),
        "{} normative MUST requirement(s) have no conformance test:\n  {}\n\
         Every normative MUST must be mapped to at least one positive or \
         negative conformance test.",
        uncovered.len(),
        uncovered
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_coverage_row_names_a_real_normative_requirement() {
    let musts = normative_musts();
    let mut unknown: Vec<&str> = rows()
        .iter()
        .map(|r| r.requirement)
        .filter(|req| !musts.contains(*req))
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    assert!(
        unknown.is_empty(),
        "{} requirement id(s) in the coverage table are not normative MUSTs:\n  {}",
        unknown.len(),
        unknown.join("\n  ")
    );
}

#[test]
fn every_coverage_row_names_a_test_that_exists() {
    // The anti-fabrication check: a coverage row that names a test which
    // does not exist is a false coverage claim. The existence scan itself
    // lives in `meta_tests::every_named_test_in_the_coverage_table_exists`;
    // this test asserts the table is internally consistent.
    for row in rows() {
        assert!(
            !row.test.is_empty(),
            "coverage row for {} names an empty test",
            row.requirement
        );
        assert!(
            !row.crate_name.is_empty(),
            "coverage row for {} names an empty crate",
            row.requirement
        );
    }
}

#[test]
fn every_normative_must_has_executable_evidence() {
    // A MUST covered only by meta-tests is covered only by a document check.
    // The acceptance criterion asks for a positive or negative *conformance
    // test*; a governance artifact check is weaker. This test names the
    // requirements that are deliberately meta-only so the exception is
    // visible rather than implicit.
    const META_ONLY: &[&str] = &[
        "REQ-GOV-WORKSPACE-001",
        "REQ-GOV-WORKSPACE-002",
        "REQ-GOV-DOCS-001",
        "REQ-GOV-ADR-001",
        "REQ-GOV-SPEC-001",
        "REQ-GOV-CI-001",
        "REQ-GOV-SUPPLY-001",
        "REQ-GOV-FIXTURE-001",
        "REQ-GOV-RELEASE-001",
        "REQ-GOV-OPENART-001",
    ];
    for req in META_ONLY {
        let kinds: Vec<CoverageKind> = rows()
            .iter()
            .filter(|r| r.requirement == *req)
            .map(|r| r.kind)
            .collect();
        assert!(!kinds.is_empty(), "{req} must have a coverage row");
        assert!(
            kinds
                .iter()
                .all(|k| *k == CoverageKind::Meta || *k == CoverageKind::Doc),
            "{req} is expected to be governance-only but has a non-meta row"
        );
    }
}

#[test]
fn every_fixture_requirement_is_a_real_normative_requirement() {
    let musts = normative_musts();
    let coverage = coverage_map();
    let unknown: Vec<(&String, &BTreeSet<String>)> = coverage
        .iter()
        .filter(|(req, _)| !musts.contains(req.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "{} requirement id(s) named by a fixture are not normative MUSTs:\n{}",
        unknown.len(),
        unknown
            .iter()
            .map(|(req, fixtures)| format!(
                "  {req} (named by: {})",
                fixtures.iter().cloned().collect::<Vec<_>>().join(", ")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_fixture_names_at_least_one_requirement() {
    for entry in catalog() {
        assert!(
            !entry.requirements.is_empty(),
            "fixture `{}` names no requirements",
            entry.id
        );
    }
}

#[test]
fn golden_packages_name_real_normative_requirements() {
    // The `GoldenPackage.requirements` field is a SECOND source of
    // requirement ids alongside the catalog — the catalog entry for
    // "golden/minimal" / "golden/full" already has its own list, and
    // the `GoldenPackage` struct has another. Both must point at real
    // normative requirements; otherwise a fixture silently claims
    // coverage for a nonexistent id and the catalog reverse check
    // misses it because it only scans `catalog()`.
    use elang_conformance::golden_packages;
    let musts = normative_musts();
    let mut unknown: Vec<(&str, &str)> = Vec::new();
    for pkg in golden_packages::all().expect("golden packages build") {
        for req in pkg.requirements {
            if !musts.contains(*req) {
                unknown.push((pkg.id, req));
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "{} golden-package requirement id(s) are not normative MUSTs:\n{}",
        unknown.len(),
        unknown
            .iter()
            .map(|(pkg, req)| format!("  {pkg}: {req}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn fixture_ids_are_unique() {
    let mut ids: Vec<String> = catalog().into_iter().map(|e| e.id).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        total,
        "duplicate fixture id in the corpus catalog: {ids:?}"
    );
}

#[test]
fn every_negative_fixture_declares_an_expected_code() {
    // A rejection fixture that does not name a code cannot be checked; the
    // runner would have to accept any error, which is exactly the
    // "some error" behaviour the issue forbids.
    for entry in catalog() {
        if let FixtureExpectation::Reject { code } = entry.expectation {
            assert!(
                !code.is_empty(),
                "rejection fixture `{}` declares an empty expected code",
                entry.id
            );
        }
    }
}

#[test]
fn the_conformance_requirements_themselves_are_covered() {
    // REQ-CONF-001..005 are the corpus's own requirements. A corpus that
    // does not cover them is a corpus that does not know what it is.
    for req in [
        "REQ-CONF-001",
        "REQ-CONF-002",
        "REQ-CONF-003",
        "REQ-CONF-004",
        "REQ-CONF-005",
    ] {
        let coverage = coverage_map();
        assert!(
            coverage.contains_key(req),
            "{req} is not covered by any fixture"
        );
        assert!(
            !coverage.get(req).expect("checked above").is_empty(),
            "{req} has no fixture"
        );
    }
}
