//! T1616 local guard for the T1615 reproducible-build claim.
//!
//! The CI workflow `reproducible-build.yml` re-builds everything in two
//! clean containers and asserts the shape files are byte-identical.
//! This test runs the same check on a single machine in a single test
//! invocation, so a regression of the shape's structural stability is
//! caught before it reaches CI.
//!
//! What is checked, per generator:
//!
//! 1. The shape output committed in the tree is exactly what the
//!    generator produces (no hand edits, no drift).
//! 2. Running the generator twice in a row produces byte-identical
//!    output (the shape is deterministic; the measured half is not, and
//!    that is the whole reason the shape exists).
//! 3. The shape JSON carries no measured value: every timing and every
//!    resident-set field is `null`, so a future regression that un-masks
//!    a measurement is caught immediately.
//!
//! The generators write into the working tree's `docs/*.shape.{md,json}`
//! files. Because the shape is deterministic, the regenerated content is
//! identical to what was committed — nothing is left dirty.
//!
//! All four checks share one run of each generator via a `OnceLock`:
//! the generators write to fixed paths, so two tests running them
//! concurrently would race on the same file and could read a torn
//! write.

#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// The three snapshots of one shape artifact.
struct ShapeRun {
    /// The content committed in the tree, read before the first run.
    before: Vec<u8>,
    /// The content after the first run.
    after: Vec<u8>,
    /// The content after the second run.
    after_second: Vec<u8>,
}

/// Both shape artifacts one generator produces.
struct GeneratorShape {
    md: ShapeRun,
    json: ShapeRun,
}

/// Run one of the measurement generators twice in `--shape` mode and
/// snapshot both of its artifacts before, after the first run, and
/// after the second run.
///
/// `CARGO_BIN_EXE_<name>` is set by cargo for integration tests and
/// points at the freshly-built binary, so the test exercises the same
/// code the release gate exercises.
fn run_shape_twice(exe: &str, label: &str, md_rel: &str, json_rel: &str) -> GeneratorShape {
    let md_before = read(md_rel);
    let json_before = read(json_rel);
    run(exe, label);
    let md_after = read(md_rel);
    let json_after = read(json_rel);
    run(exe, label);
    let md_after_second = read(md_rel);
    let json_after_second = read(json_rel);
    GeneratorShape {
        md: ShapeRun {
            before: md_before,
            after: md_after,
            after_second: md_after_second,
        },
        json: ShapeRun {
            before: json_before,
            after: json_after,
            after_second: json_after_second,
        },
    }
}

fn run(exe: &str, label: &str) {
    let status = Command::new(exe)
        .arg("--shape")
        .current_dir(workspace_root())
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {label}: {e}"));
    assert!(status.success(), "{label} --shape exited {status:?}");
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(p.pop(), "could not pop crate dir");
    assert!(p.pop(), "could not pop workspace dir");
    p
}

fn read(rel: &str) -> Vec<u8> {
    let p = workspace_root().join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// One lock per generator: the generator writes to fixed paths, so a
/// second, independent invocation running concurrently would race on
/// the same file and could read a torn write.
fn benchmark_run() -> &'static GeneratorShape {
    static RUN: OnceLock<GeneratorShape> = OnceLock::new();
    RUN.get_or_init(|| {
        run_shape_twice(
            env!("CARGO_BIN_EXE_gen_benchmarks"),
            "gen_benchmarks",
            "docs/benchmarks.shape.md",
            "docs/benchmarks.shape.json",
        )
    })
}

fn peak_memory_run() -> &'static GeneratorShape {
    static RUN: OnceLock<GeneratorShape> = OnceLock::new();
    RUN.get_or_init(|| {
        run_shape_twice(
            env!("CARGO_BIN_EXE_gen_peak_memory"),
            "gen_peak_memory",
            "docs/peak-memory.shape.md",
            "docs/peak-memory.shape.json",
        )
    })
}

#[test]
fn benchmark_shape_is_stable_and_matches_the_committed_artifact() {
    let r = &benchmark_run().md;
    assert_eq!(
        r.before, r.after,
        "the committed docs/benchmarks.shape.md differs from what gen_benchmarks --shape produces"
    );
    assert_eq!(
        r.after, r.after_second,
        "two consecutive gen_benchmarks --shape runs produced different bytes"
    );
}

#[test]
fn peak_memory_shape_is_stable_and_matches_the_committed_artifact() {
    let r = &peak_memory_run().md;
    assert_eq!(
        r.before, r.after,
        "the committed docs/peak-memory.shape.md differs from what gen_peak_memory --shape produces"
    );
    assert_eq!(
        r.after, r.after_second,
        "two consecutive gen_peak_memory --shape runs produced different bytes"
    );
}

#[test]
fn benchmark_shape_json_is_stable_and_matches_the_committed_artifact() {
    let r = &benchmark_run().json;
    assert_eq!(
        r.before, r.after,
        "the committed docs/benchmarks.shape.json differs from what gen_benchmarks --shape produces"
    );
    assert_eq!(
        r.after, r.after_second,
        "two consecutive gen_benchmarks --shape runs produced different bytes"
    );
}

#[test]
fn peak_memory_shape_json_is_stable_and_matches_the_committed_artifact() {
    let r = &peak_memory_run().json;
    assert_eq!(
        r.before, r.after,
        "the committed docs/peak-memory.shape.json differs from what gen_peak_memory --shape produces"
    );
    assert_eq!(
        r.after, r.after_second,
        "two consecutive gen_peak_memory --shape runs produced different bytes"
    );
}

#[test]
fn benchmark_shape_json_masks_every_measured_field() {
    let raw = &benchmark_run().json.after;
    let v: serde_json::Value =
        serde_json::from_slice(raw).expect("benchmarks.shape.json must be valid JSON");
    let results = v
        .get("results")
        .and_then(|x| x.as_array())
        .expect("results must be a JSON array");
    assert!(
        !results.is_empty(),
        "results array must be non-empty (no benchmarks were measured)"
    );
    for r in results {
        for k in ["median_ns", "median_us", "min_ns", "max_ns", "total_ns"] {
            assert!(
                r.get(k).map(|x| x.is_null()).unwrap_or(false),
                "benchmarks.shape.json result entry leaked a measured value at key {k}: {:?}",
                r.get(k)
            );
        }
    }
    let memory = v.get("memory").expect("memory object must exist");
    if memory.get("measured").and_then(serde_json::Value::as_bool) == Some(true) {
        for k in [
            "vmhwm_kib",
            "vmrss_kib",
            "vmpeak_kib",
            "vmhwm_mib",
            "vmpeak_mib",
        ] {
            assert!(
                memory.get(k).map(|x| x.is_null()).unwrap_or(false),
                "benchmarks.shape.json memory object leaked a measured value at key {k}: {:?}",
                memory.get(k)
            );
        }
    }
    // The generation timestamp and the harness wall time are measured
    // values too; the shape variant pins them to zero so the structure
    // of the timing budget is visible without the wall-clock noise.
    assert_eq!(
        v.get("generated_at_unix")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "generated_at_unix must be pinned to 0 in shape mode"
    );
    assert_eq!(
        v.get("elapsed_ns").and_then(serde_json::Value::as_u64),
        Some(0),
        "elapsed_ns must be pinned to 0 in shape mode"
    );
    // The environment block is host- and build-derived; it must be
    // masked or the shape would not be portable.
    let env = v.get("environment").expect("environment object must exist");
    for k in [
        "target_triple",
        "cpu_brand",
        "kernel_release",
        "profile",
        "rustc_version",
    ] {
        assert!(
            env.get(k).map(|x| x.is_null()).unwrap_or(false),
            "benchmarks.shape.json environment leaked a host-specific value at key {k}: {:?}",
            env.get(k)
        );
    }
}

#[test]
fn peak_memory_shape_json_masks_every_measured_field() {
    let raw = &peak_memory_run().json.after;
    let v: serde_json::Value =
        serde_json::from_slice(raw).expect("peak-memory.shape.json must be valid JSON");
    let measurements = v
        .get("measurements")
        .and_then(|x| x.as_array())
        .expect("measurements must be a JSON array");
    assert!(
        !measurements.is_empty(),
        "measurements array must be non-empty"
    );
    for m in measurements {
        for k in ["vmhwm_kib", "vmrss_kib", "vmpeak_kib"] {
            assert!(
                m.get(k).map(|x| x.is_null()).unwrap_or(false),
                "peak-memory.shape.json measurement entry leaked a measured value at key {k}: {:?}",
                m.get(k)
            );
        }
    }
    let env = v.get("environment").expect("environment object must exist");
    for k in ["os", "arch", "cpu_brand", "kernel_release", "profile"] {
        assert!(
            env.get(k).map(|x| x.is_null()).unwrap_or(false),
            "peak-memory.shape.json environment leaked a host-specific value at key {k}: {:?}",
            env.get(k)
        );
    }
    assert_eq!(
        v.get("memory_supported").map(|x| x.is_null()),
        Some(true),
        "memory_supported must be null in shape mode so the shape is platform-independent"
    );
}
