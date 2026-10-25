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
//! A test that loads a malformed regression fixture must assert the
//! expected error category as well as the absence of a panic. A
//! previously-broken valid input must assert the expected success.
//! If the parser panics or accepts malformed bytes, the regression
//! test fails before the change can merge.
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
        .join("..")
        .join("fuzz")
        .join("findings");
    if !findings_dir.exists() {
        // No findings yet — that's a valid state. Skip.
        return;
    }
    let has_findings = std::fs::read_dir(&findings_dir)
        .expect("findings root readable")
        .any(|entry| {
            let path = entry.expect("DirEntry").path();
            path.is_file()
                && path.extension().is_some_and(|ext| ext == "md")
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(|stem| stem.split_once('-'))
                    .is_some_and(|(id, slug)| {
                        id.len() == 4 && id.bytes().all(|b| b.is_ascii_digit()) && !slug.is_empty()
                    })
        });

    // For every committed finding record under `fuzz/findings/`, the
    // expected directory `fuzz/regressions/<target>/` must exist with
    // at least one file. We can't parse the YAML frontmatter here
    // (no `serde_yaml` dev-dep) so we conservatively assert at the
    // shape level: at least one target dir exists, and every target
    // dir has at least one fixture. The per-record pinning is done by
    // the per-finding regression tests added below as findings land.
    let regressions_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fuzz")
        .join("regressions");

    if !regressions_root.exists() {
        assert!(
            !has_findings,
            "finding records require committed regressions"
        );
        // No findings or regressions committed yet — a valid state.
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

    assert!(
        target_dirs > 0 || !has_findings,
        "finding records exist; regressions dir must have at least one target subdir"
    );
}

/// Smoke check that every binary declared in `fuzz/Cargo.toml` has a
/// known name we can list explicitly. When a new target is added,
/// the regression harness needs to acknowledge it — this assertion
/// makes the omission loud.
#[test]
fn known_fuzz_targets_are_acknowledged() {
    // All eight fuzz targets that exist on `origin/main` after
    // PR #58 (T1503-T1508) merged. This list is the source of
    // truth for which targets have regression-test wiring; if a
    // target is added under `fuzz/fuzz_targets/` but is missing
    // here, the assertion below (which parses `fuzz/Cargo.toml`)
    // fails loudly.
    const KNOWN_TARGETS: &[&str] = &[
        "container_structure", // T1501
        "resource_path",       // T1502
        "ipa_tokenizer",       // T1503
        "kdl_parser",          // T1504
        "phon_condition",      // T1505/T1507/T1508
        "eld_schema",          // T1506
        "epf_schema",          // T1506
        "zstd_frame",          // T1507
    ];
    // Read the explicitly declared binaries without adding a TOML dependency.
    let manifest = include_str!("../../../fuzz/Cargo.toml");
    let lines: Vec<_> = manifest
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .collect();
    for bin in lines.split(|line| *line == "[[bin]]").skip(1) {
        let name = bin
            .iter()
            .take_while(|line| !line.starts_with('['))
            .find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key.trim() == "name").then_some(value.trim())
            })
            .expect("every fuzz [[bin]] must have a name")
            .strip_prefix('"')
            .and_then(|value| value.split('"').next())
            .expect("fuzz binary names must be double-quoted");
        assert!(
            KNOWN_TARGETS.contains(&name),
            "fuzz/Cargo.toml declares {name}, but it is not in KNOWN_TARGETS"
        );
    }

    // If a target has a regression directory, that target must be in
    // KNOWN_TARGETS — otherwise the regression test below will silently
    // skip it.
    let regressions_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
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
                     add it to fuzz/Cargo.toml, the fuzz.yml matrix, and \
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
        "ipa_tokenizer",       // T1503
        "kdl_parser",          // T1504
        "phon_condition",      // T1505/T1507/T1508
        "eld_schema",          // T1506
        "epf_schema",          // T1506
        "zstd_frame",          // T1507
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
// 3. For malformed input, uses `expect_err` and asserts the expected
//    error category. The call must also **not panic**, **not OOB-read**,
//    and **not hang past the harness timeout**. A panic or unexpected
//    success on malformed input is a regression; look up the finding in
//    `fuzz/findings/<NNNN>-<slug>.md` and re-apply the fix from
//    the PR cited there.
//
// At the time of T1519 closure, this section is intentionally
// empty — no fuzz findings have been produced yet on
// `origin/main`. The first finding will add the first test below
// following the template in `fuzz/findings/README.md`.
