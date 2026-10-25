//! Aggregate corpus-runner gate.
//!
//! The per-family tests (`every_binary_fixture_behaves_as_declared`,
//! `every_integrity_fixture_behaves_as_declared`,
//! `every_zstd_negative_is_rejected_at_the_declared_level`,
//! `every_level_of_every_golden_package_passes`) cover each fixture
//! family in isolation. They drive [`crate::corpus::run_fixture`] one
//! fixture at a time.
//!
//! This integration test drives the corpus through the aggregate
//! [`crate::corpus::run_corpus`] entry point — the one the fixture
//! model exists to support — over every archive fixture in the
//! repository and asserts the run produces zero failures.
//!
//! It also pins a minimum fixture count so the test cannot pass
//! vacuously: an empty fixture enumeration would make `run_corpus`
//! trivially green.
//!
//! # Why this is its own file
//!
//! `run_corpus` was previously defined but had no call sites — the
//! aggregate runner existed as documentation but was never executed.
//! An aggregate gate is the natural way to (a) keep `run_corpus` alive
//! and (b) catch a regression in any family that the per-family tests
//! happened to skip.
//!
// a single failure in any family will surface here.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "aggregate corpus runner: a precondition violation (empty fixture list, malformed fixture) is a corpus bug, not a runtime condition"
)]

use elang_conformance::corpus::{run_corpus, Fixture};

/// Every archive-level fixture in the corpus.
///
/// Archive fixtures are the ones that carry an inspect/verify/validate
/// level; the schema-level (`DocumentFixture`) families are parsed
/// directly and have no reader level. Mirrors the enumeration used by
/// `tests/stage_conformance.rs::archive_fixtures`.
fn archive_fixtures() -> Vec<Fixture> {
    let mut all = Vec::new();
    all.extend(elang_conformance::negatives::binary_fixtures());
    all.extend(elang_conformance::negatives::integrity_fixtures());
    all.extend(elang_conformance::negatives::validate_negatives());
    all.extend(elang_conformance::zstd_negatives::fixtures());
    all.extend(elang_conformance::inspect_matrix::fixtures());
    all.extend(elang_conformance::parser_budgets::archive_fixtures());
    all
}

#[test]
fn the_archive_corpus_runs_cleanly() {
    let fixtures = archive_fixtures();
    assert!(
        !fixtures.is_empty(),
        "the archive fixture list must not be empty — a regression in any family's enumeration \
         must surface here, not silently reduce coverage",
    );
    assert!(
        fixtures.len() >= 30,
        "the aggregate corpus runner must cover a non-trivial fraction of the archive corpus; \
         only {} fixtures were enumerated — check that every fixture family is wired in",
        fixtures.len(),
    );
    let failures = run_corpus(&fixtures);
    println!(
        "aggregate corpus run: {} archive fixtures, {} failure(s)",
        fixtures.len(),
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "run_corpus reported {} failure(s) over {} archive fixtures:\n{}",
        failures.len(),
        fixtures.len(),
        failures
            .iter()
            .map(|f| format!(
                "  - fixture `{}` (requirement {}): {}",
                f.fixture, f.requirement, f.detail
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn every_archive_fixture_id_is_unique() {
    let fixtures = archive_fixtures();
    let duplicates = elang_conformance::corpus::duplicate_ids(&fixtures);
    assert!(
        duplicates.is_empty(),
        "duplicate archive fixture id(s): {duplicates:?}",
    );
}
