//! Stage conformance: every negative archive fixture must be rejected at a
//! stage the normative registry publishes for its diagnostic code.
//!
//! REQ-CONF-003 (normative) reads:
//!
//! > Assert expected stage + error code for each fixture under
//! > inspect/verify/validate.
//!
//! The error-code half of that sentence is checked by the corpus runner
//! itself (`corpus::run_fixture` matches the observed code *and* category
//! exactly, and refuses a fixture that was rejected below its declared
//! level). This file covers the **stage** half.
//!
//! # Why the registry is the source of truth for stages
//!
//! `specs/registry/error-codes.json` publishes, for every registered code,
//! the set of pipeline stages at which that code may legitimately be
//! raised — for example `HEADER_BAD_MAGIC` is published for stages
//! `[1, 2]`, while `INTEGRITY_HASH_MISMATCH` is published for stage `[11]`.
//! That is the normative statement of "expected stage", and it lives in
//! exactly one place, so the corpus does not have to restate it per
//! fixture (and cannot drift from it).
//!
//! Reading the registry at test time rather than hard-coding a table
//! follows the same reasoning as `requirement_completeness.rs`: a
//! hard-coded snapshot keeps passing when the registry changes, so the
//! test would stop detecting the drift it exists to detect.
//!
//! # Two independent assertions
//!
//! 1. The observed stage is one of the stages the registry publishes for
//!    the code the reader actually emitted.
//! 2. The observed stage is at or below the ceiling of the level the
//!    fixture declares — a fixture that claims to fail at `Verify` must
//!    not surface a stage-12 diagnostic, because stage 12 is only reached
//!    by `Validate`.
//!
//! The second assertion is what makes the first one meaningful: without it
//! a fixture could satisfy the registry check while contradicting its own
//! declared level.

// The crate's strict lints deny `expect`, `panic`, and unchecked
// indexing. These tests are the spec for the stage invariant: a
// precondition violation (a missing registry file, a malformed registry,
// a missing fixture family) must fail loudly rather than silently
// skipping the check. The same module-level allow and justification are
// used by the other integration tests in this crate.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "stage conformance is the spec; precondition violations fail loudly"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use elang_conformance::corpus::{observe_fixture, Expectation, Fixture};

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

/// The published stages for every registered diagnostic code.
///
/// Read from `specs/registry/error-codes.json`. Panics if the registry is
/// missing or malformed — a broken normative registry is a hard failure,
/// not a reason to skip the stage check.
fn registry_stages() -> BTreeMap<String, Vec<u8>> {
    let root = workspace_root().expect("the workspace root must be discoverable");
    let path = root.join("specs/registry/error-codes.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the normative error-code registry must be readable: {e}"));

    let value: serde_json::Value =
        serde_json::from_str(&text).expect("the error-code registry must be valid JSON");
    let codes = value
        .get("codes")
        .and_then(serde_json::Value::as_array)
        .expect("the error-code registry must carry a `codes` array");

    let mut stages: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for entry in codes {
        let code = entry
            .get("code")
            .and_then(serde_json::Value::as_str)
            .expect("every registry entry must carry a `code` string");
        let published = entry
            .get("stages")
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("registry entry `{code}` must carry a `stages` array"));
        let mut parsed: Vec<u8> = Vec::with_capacity(published.len());
        for stage in published {
            let number = stage
                .as_u64()
                .unwrap_or_else(|| panic!("registry entry `{code}` has a non-numeric stage"));
            parsed.push(u8::try_from(number).expect("a pipeline stage fits in a u8"));
        }
        stages.insert(code.to_string(), parsed);
    }
    assert!(
        !stages.is_empty(),
        "the error-code registry must publish at least one code"
    );
    stages
}

/// Every archive-level fixture in the corpus.
///
/// Archive fixtures are the ones that carry an inspect/verify/validate
/// level; the schema-level (`DocumentFixture`) families are parsed
/// directly and have no reader level.
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

/// Every negative archive fixture, with its declared code and level.
fn negative_fixtures() -> Vec<Fixture> {
    archive_fixtures()
        .into_iter()
        .filter(|f| matches!(f.expectation, Expectation::Reject { .. }))
        .collect()
}

#[test]
fn every_negative_fixture_code_is_registered() {
    let stages = registry_stages();
    let fixtures = negative_fixtures();
    assert!(
        !fixtures.is_empty(),
        "the corpus must contain negative archive fixtures for this test to mean anything"
    );
    for fixture in &fixtures {
        let Expectation::Reject { code, .. } = fixture.expectation else {
            unreachable!("filtered to Reject expectations above");
        };
        assert!(
            stages.contains_key(code),
            "fixture `{}` declares the diagnostic code `{code}`, which is not in the normative \
             error-code registry — a fixture may only assert a registered code",
            fixture.id
        );
    }
}

#[test]
fn every_negative_fixture_is_rejected_at_a_registered_stage() {
    let stages = registry_stages();
    let fixtures = negative_fixtures();
    assert!(
        !fixtures.is_empty(),
        "the corpus must contain negative archive fixtures for this test to mean anything"
    );

    for fixture in &fixtures {
        let Expectation::Reject { level, code, .. } = fixture.expectation else {
            unreachable!("filtered to Reject expectations above");
        };
        let observed = observe_fixture(fixture)
            .unwrap_or_else(|f| panic!("fixture `{}` did not behave as declared: {f}", fixture.id));

        let published = stages.get(code).unwrap_or_else(|| {
            panic!(
                "fixture `{}` declares unregistered code `{code}`",
                fixture.id
            )
        });
        assert!(
            published.contains(&observed.stage),
            "fixture `{}` declares code `{code}`, which the registry publishes for stage(s) \
             {published:?}, but the reader reported stage {}",
            fixture.id,
            observed.stage
        );

        assert!(
            observed.stage <= level.ceiling_stage(),
            "fixture `{}` declares the level {level:?} (ceiling stage {}), but the reader \
             reported stage {}, which is above that ceiling",
            fixture.id,
            level.ceiling_stage(),
            observed.stage
        );
    }
}

#[test]
fn the_stage_check_covers_the_whole_negative_corpus() {
    // Fail closed: if the fixture enumeration were emptied by a refactor,
    // the two tests above would pass vacuously. Pin a floor that is well
    // below the current corpus size but well above zero.
    let count = negative_fixtures().len();
    assert!(
        count >= 20,
        "the stage check must cover the negative archive corpus; only {count} fixtures were found"
    );
    println!("stage conformance checked {count} negative archive fixtures");
}
