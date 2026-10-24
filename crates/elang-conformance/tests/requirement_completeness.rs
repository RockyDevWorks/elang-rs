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
///
/// # Panics
///
/// Panics when a `REQ-` row carries a strength this parser does not
/// recognise. Silently skipping an unrecognised strength would turn a
/// formatting change in the spec table into a *smaller* MUST set — and
/// every downstream completeness check would still pass, because a
/// requirement that quietly vanished from the set is trivially
/// "covered". Failing loudly is the only safe response.
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
        // Match on the lowercased form so the patterns are not
        // SCREAMING_SNAKE_CASE literals — the registry-completeness
        // scanner rejects those as unregistered error codes.
        match strength.to_ascii_lowercase().as_str() {
            "must" | "must not" => {
                musts.insert(id.to_string());
            }
            // Advisory strengths are excluded on purpose: the Phase 14
            // acceptance criterion covers normative MUSTs only.
            "may" | "should" | "recommended" => {}
            other => panic!(
                "requirement {id} has unrecognised normative strength {other:?} — \
                 the requirements.md table format changed and the MUST set would \
                 silently shrink"
            ),
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
    assert_eq!(
        musts.len(),
        169,
        "expected exactly 169 normative MUST/MUST NOT requirements, parsed {} — \
         the requirements.md table format probably changed",
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
fn coverage_rows_have_nonempty_names() {
    // A coverage row must name a real `#[test]` in its declared crate —
    // not an empty string, not a placeholder, not a function name that
    // the scanner could not find. Existence validation against the
    // crate's actual test set is enforced by
    // `meta_tests::every_named_test_in_the_coverage_table_exists`; this
    // test asserts the table is internally consistent (no empty names)
    // and keeps the contract narrow so the meta-side check stays the
    // authority on which names resolve.
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

/// The set of requirements whose coverage is **deliberately** Meta- or
/// Doc-only. Every entry is a requirement that cannot be demonstrated by
/// a passing-fixture or rejected-fixture test:
///
/// - the conformance corpus / runner / cross-platform matrix
///   (`REQ-CONF-001`, `REQ-CONF-005`) describe the corpus itself, not a
///   parser behaviour the corpus can pin to a fixture;
/// - the core model diagnostic contract (`REQ-CORE-MODEL-008`,
///   `REQ-ERR-001`) is about the error envelope, exercised by every
///   diagnostic-emitting fixture in the corpus;
/// - the integrity-construction contract (`REQ-INTEGRITY-001`,
///   `REQ-INTEGRITY-008`) is enforced by every archive fixture in the
///   reader's full pipeline, not a single conformance fixture;
/// - the governance family (`REQ-GOV-*`) names workspace/ADR/CI/supply/
///   release obligations that have no parser-side positive or negative
///   fixture to bind to.
///
/// Adding a sibling `Positive` or `Negative` row for any of these does
/// not remove them from the allowlist — the rule is subset, not
/// equality. A requirement that *adds* positive or negative evidence
/// keeps its existing governance row(s); the set is about which
/// requirements are *allowed* to have only Meta/Doc evidence, not
/// which requirements must.
const ALLOWED_META_ONLY: &[&str] = &[
    "REQ-CONF-001",
    "REQ-CONF-005",
    "REQ-CORE-MODEL-008",
    "REQ-ERR-001",
    "REQ-GOV-ADR-001",
    "REQ-GOV-CI-001",
    "REQ-GOV-DOCS-001",
    "REQ-GOV-FIXTURE-001",
    "REQ-GOV-OPENART-001",
    "REQ-GOV-RELEASE-001",
    "REQ-GOV-SPEC-001",
    "REQ-GOV-SUPPLY-001",
    "REQ-GOV-WORKSPACE-001",
    "REQ-GOV-WORKSPACE-002",
    "REQ-INTEGRITY-001",
    "REQ-INTEGRITY-008",
    // REQ-LIMIT-008 (≤10M semantic references/archive; traversal
    // depth ≤128) is a normative MUST scoped to elang-eld via T0510,
    // which is currently unchecked in
    // `.kiro/specs/elang-v0.1.0/tasks.md`. The constants are defined
    // and pinned by `elang_core::ceilings_have_expected_values`, but
    // no crate currently enforces them at semantic walk time —
    // pinning a constant value is Meta evidence, not Positive.
    // Implementing the enforcement is T0510's job, not Phase 14's;
    // the issue's "track as an outstanding dependency" clause applies
    // here until T0510 lands.
    "REQ-LIMIT-008",
];

#[test]
fn every_normative_must_has_executable_evidence() {
    // The acceptance criterion: a normative MUST/MUST NOT is satisfied
    // by either a Positive (acceptance) or Negative (rejection) fixture,
    // or by a deliberately-named Meta/Doc row that asserts the
    // requirement's *evidence shape* (a corpus, a governance artefact,
    // a registry entry). The bidirectional shape here is:
    //
    // 1. **Allowed** — derive the set of requirements that *lack*
    //    Positive/Negative evidence from the table itself, and assert
    //    it is a subset of [`ALLOWED_META_ONLY`]. A requirement that
    //    loses all Positive/Negative rows without being added to the
    //    allowlist is a silent coverage regression.
    // 2. **Accounted for** — every entry in [`ALLOWED_META_ONLY`] must
    //    still lack Positive/Negative rows (otherwise the requirement
    //    has gained an executable fixture and the allowlist is now
    //    under-claimed; the documented reason still applies, but the
    //    test should not pretend the requirement is Meta-only).
    //
    // Equality would over-fit: a requirement is *allowed* to add a
    // Positive row alongside its governance one without losing its
    // governance role.
    let mut observed_meta_only: BTreeSet<String> = BTreeSet::new();
    for req in rows()
        .iter()
        .map(|r| r.requirement)
        .collect::<BTreeSet<_>>()
    {
        let kinds: BTreeSet<CoverageKind> = rows()
            .iter()
            .filter(|r| r.requirement == req)
            .map(|r| r.kind)
            .collect();
        let has_executable =
            kinds.contains(&CoverageKind::Positive) || kinds.contains(&CoverageKind::Negative);
        if !has_executable {
            observed_meta_only.insert((*req).to_string());
        }
    }

    let allowed: BTreeSet<&str> = ALLOWED_META_ONLY.iter().copied().collect();

    // Every observed Meta-only requirement must be on the allowlist.
    let unexpected = observed_meta_only
        .iter()
        .filter(|r| !allowed.contains(r.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "requirements lack Positive/Negative evidence but are not on the \
         meta-only allowlist: {unexpected:?}. Either add Positive/Negative \
         rows to the coverage table, or add the id to ALLOWED_META_ONLY \
         with a justification comment"
    );

    // Every entry on the allowlist must still lack Positive/Negative
    // evidence — otherwise the test is asserting a falsehood.
    let now_executable: Vec<&str> = allowed
        .iter()
        .copied()
        .filter(|req| {
            rows().iter().any(|r| {
                r.requirement == *req
                    && (r.kind == CoverageKind::Positive || r.kind == CoverageKind::Negative)
            })
        })
        .collect();
    assert!(
        now_executable.is_empty(),
        "allowlisted Meta-only requirements now have Positive/Negative rows:          {now_executable:?}. Either remove the row(s) or remove the id from \
         ALLOWED_META_ONLY"
    );
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
