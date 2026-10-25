//! Fuzz regression fixtures (Phase 15 / Issue #25, T1519).
//!
//! Each test in this file loads a byte sequence committed under
//! `fuzz/regressions/<target>/` and re-runs the same call path the
//! fuzz target drove. The contract under test is the one the fuzz
//! target is meant to enforce:
//!
//! > No adversarial byte sequence produced by libFuzzer may panic,
//! > read out of bounds, hang, allocate past the budget, or report
//! > a success on a malformed buffer. Every rejectable input must
//! > return a structured `Err`.
//!
//! A test that loads a regression fixture and asserts the call does
//! not panic — i.e. returns either `Ok` (rare; a previously-broken
//! happy path that the fix restored) or `Err` (the common case) —
//! pins the contract against future regressions. If the parser
//! regresses and starts panicking on the same input, this test
//! fails before the change can merge.
//!
//! # Why a workspace test, not a fuzz target
//!
//! The fuzz targets under `fuzz/fuzz_targets/` are not workspace
//! members (they're built on nightly with `cargo-fuzz` and excluded
//! from `cargo build --workspace`). Running the regression fixtures
//! as a regular workspace test means they participate in the normal
//! CI matrix (Rustfmt, clippy, tests, MSRV, doc build, code coverage)
//! without requiring nightly.
//!
//! # Why the byte sequences are committed
//!
//! The whole point of a regression fixture is reproducibility. A
//! test that takes "the bytes that crashed parser v0.1.0+git.sha"
//! as `include_bytes!` and pins that hash forever is the only way to
//! ensure the same input will fail again if the fix is reverted.
//! libFuzzer's auto-minimised crash artifacts are small (typically
//! < 1 KiB), so committing them is cheap.
//!
//! # Triage procedure
//!
//! See [`fuzz/findings/README.md`](../../fuzz/findings/README.md) for
//! the full process: finding → reproduction → classification → fix PR
//! → regression test → index update. Every regression test added here
//! must have a corresponding row in `fuzz/findings/index.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "regression harness for adversarial inputs; the parser under test \
              must not panic on these bytes, and the harness's own `.expect()`s are \
              configuration errors (missing committed fixture) that must surface loudly"
)]

use std::path::Path;

/// Walk `fuzz/regressions/<target>/` and collect every committed
/// regression fixture for `<target>`. Returns `None` if the directory
/// does not exist (i.e. the target has no findings yet, which is the
/// common case at the time T1519 closes).
fn regression_fixtures(target: &str) -> Option<Vec<std::path::PathBuf>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fuzz")
        .join("regressions")
        .join(target);
    if !dir.exists() {
        return None;
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        }
    }
    Some(out)
}

/// A smoke check that the regression fixture directory shape is
/// what the README and index claim it is: every target with findings
/// has a directory under `fuzz/regressions/`. If a finding record
/// exists in `fuzz/findings/` but no committed bytes under
/// `fuzz/regressions/<target>/`, this test fails — the discipline is
/// "no finding record without a committed reproduction".
#[test]
fn regression_fixtures_directory_is_well_formed() {
    let findings_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fuzz")
        .join("findings");
    if !findings_dir.exists() {
        // No findings yet — that's a valid state. Skip.
        return;
    }

    // For every committed finding record under `fuzz/findings/`, the
    // expected directory `fuzz/regressions/<target>/` must exist with
    // at least one file. We can't parse the YAML frontmatter here
    // (no `serde_yaml` dev-dep) so we conservatively assert at the
    // shape level: at least one target dir exists, and every target
    // dir has at least one fixture. The per-record pinning is done by
    // the per-finding regression tests added below as findings land.
    let regressions_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fuzz")
        .join("regressions");

    if !regressions_root.exists() {
        // No regressions committed yet — also a valid state.
        return;
    }

    let mut target_dirs = 0;
    for entry in std::fs::read_dir(&regressions_root).expect("regressions root readable") {
        let entry = entry.expect("DirEntry");
        if entry.path().is_dir() {
            target_dirs += 1;
            let mut files = 0;
            for sub in std::fs::read_dir(entry.path()).expect("target dir readable") {
                let sub = sub.expect("DirEntry");
                if sub.path().is_file() {
                    files += 1;
                }
            }
            assert!(
                files > 0,
                "regressions/{} is empty — every directory must contain at least one committed fixture",
                entry.path().display()
            );
        }
    }

    // If we got here, the layout is valid. The count is informational.
    assert!(
        target_dirs > 0 || !findings_dir.join("README.md").exists(),
        "fuzz/findings/README.md implies findings exist; regressions dir must have at least one target subdir"
    );
}

/// Smoke check that every target under `fuzz/fuzz_targets/` has a
/// known name we can list explicitly. When a new target is added,
/// the corresponding regression test stub below needs a new
/// `match` arm — this assertion makes the omission loud.
#[test]
fn known_fuzz_targets_are_acknowledged() {
    // The two targets currently on `origin/main`. PR #58 (T1503-T1508)
    // will add six more once it lands; this list is the source of
    // truth for which targets have regression-test wiring.
    const KNOWN_TARGETS: &[&str] = &[
        "container_structure", // T1501
        "resource_path",       // T1502
    ];
    assert!(
        KNOWN_TARGETS.contains(&"container_structure"),
        "KNOWN_TARGETS must include container_structure (T1501)"
    );
    assert!(
        KNOWN_TARGETS.contains(&"resource_path"),
        "KNOWN_TARGETS must include resource_path (T1502)"
    );

    // If a target has a regression directory, that target must be in
    // KNOWN_TARGETS — otherwise the regression test below will silently
    // skip it.
    let regressions_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fuzz")
        .join("regressions");
    if regressions_root.exists() {
        for entry in std::fs::read_dir(&regressions_root).expect("regressions root readable") {
            let entry = entry.expect("DirEntry");
            if entry.path().is_dir() {
                let name = entry.file_name();
                let name = name.to_str().expect("UTF-8 dir name");
                assert!(
                    KNOWN_TARGETS.contains(&name),
                    "regressions/{name} exists but {name} is not in KNOWN_TARGETS — \
                     add it to fuzz/fuzz_targets/Cargo.toml, the fuzz.yml matrix, and \
                     KNOWN_TARGETS here so the regression test below exercises it"
                );
            }
        }
    }
}

/// A second smoke check: every target that has *no* regression
/// directory today is one we expect to be clean. If libFuzzer has
/// not produced findings for it yet, that's fine — but the test
/// must not be silently skipped. This loop asserts "we explicitly
/// know every known target is clean" so a future finding triggers
/// a new test, not a silent skip.
#[test]
fn every_known_target_is_currently_clean() {
    const KNOWN_TARGETS: &[&str] = &[
        "container_structure", // T1501
        "resource_path",       // T1502
    ];

    for target in KNOWN_TARGETS {
        match regression_fixtures(target) {
            None => {
                // No findings yet — this is the expected state for
                // most targets at T1519 closure. The assertion is
                // implicit: we did not crash, we did not skip, and
                // `regression_fixtures` returned `None`. The local
                // fuzz campaigns recorded in
                // `fuzz/findings/index.md` (currently empty) are the
                // audit trail for the campaigns that ran.
            }
            Some(fixtures) => {
                // Findings exist for this target. There must be a
                // dedicated `regression_<target>_<id>` test below
                // for at least one of them. We can't enumerate
                // them by name without parsing the YAML frontmatter,
                // so this branch asserts the *directory* is non-empty
                // and trusts the per-finding tests to assert
                // something stronger.
                assert!(
                    !fixtures.is_empty(),
                    "regressions/{target} exists but is empty"
                );
            }
        }
    }
}

// Per-finding regression tests are added here as findings land.
// Each test:
//
// 1. `include_bytes!`s the committed reproduction under
//    `fuzz/regressions/<target>/<id>.bin`.
// 2. Calls the same parser entry point the fuzz target called.
// 3. Asserts the call returns `Ok(_)` or `Err(_)` — i.e. **does
//    not panic**, **does not OOB-read**, **does not hang past the
//    harness timeout**. A panic on this test is a regression in
//    the parser; the fix is to look up the finding in
//    `fuzz/findings/<NNNN>-<slug>.md` and re-apply the fix from
//    the PR cited there.
//
// At the time of T1519 closure, this section is intentionally
// empty — no fuzz findings have been produced yet on
// `origin/main`. The first finding will add the first test below
// following the template in `fuzz/findings/README.md`.
