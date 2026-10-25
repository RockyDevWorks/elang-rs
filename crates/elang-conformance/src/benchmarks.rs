//! Deterministic performance benchmarks for the ELANG v0.1.0 core package
//! operations (T1613).
//!
//! # Why a hand-rolled harness instead of `criterion`
//!
//! The workspace pins its dependencies exactly and runs `cargo deny` over
//! the graph, so adding a benchmarking framework is a supply-chain
//! decision, not a one-line `Cargo.toml` edit. More importantly, the
//! release gate needs numbers that are **reproducible under a documented
//! methodology** — the same binary, the same inputs, the same reported
//! figures — not statistical confidence intervals that move between runs.
//! This harness therefore reports the **median** of a fixed number of
//! iterations over a fixed input, which is a deterministic function of
//! the machine and the build.
//!
//! # What is measured
//!
//! The five principal package operations named by the v0.1.0 release
//! gate:
//!
//! | Benchmark | Operation |
//! |---|---|
//! | `inspect` | Parse the header, footer and index; build the resource listing. No resource is decompressed. |
//! | `verify` | Stages 8–11: decode every resource within budget and recompute its SHA-256. |
//! | `validate` | Stages 8–12: full ELD semantic validation. |
//! | `canonical-pack` | Build a canonical `.elang` archive from the example sources. |
//! | `random-access` | Retrieve one indexed resource from a verified package by index. |
//!
//! # What is deliberately *not* claimed
//!
//! These are **measurements of this implementation on one machine**, not
//! performance guarantees. The numbers say nothing about any other
//! machine, build profile, or future release. See
//! [`docs/benchmarks.md`](../../../../../docs/benchmarks.md) for the
//! recorded run, its environment, and the explicit limitations.
//!
//! # Reproducing a run
//!
//! ```sh
//! cargo run --release -p elang-conformance --bin gen_benchmarks
//! ```
//!
//! The generator writes `docs/benchmarks.md` and `docs/benchmarks.json`.
//! Re-running it on the same machine and the same build reproduces the
//! same figures (within the resolution of the platform clock); the JSON
//! carries the environment it was produced on so a reader can tell when
//! the numbers came from somewhere else.

use std::time::{Duration, Instant};

use crate::binary_vectors::{hex, sha256};
use elang_reader::Package;
use elang_writer::{write_archive, PackageConfig, Profile};

/// The number of timed iterations per benchmark.
///
/// Fixed (not adaptive) so that a run's workload is identical across
/// machines and across rebuilds, which is what makes the recorded
/// figures comparable to a future run.
pub const ITERATIONS: u32 = 50;

/// The number of untimed warm-up iterations per benchmark.
///
/// The first iteration of any measured operation pays one-off costs —
/// page faults for the freshly-mapped binary, allocator warm-up, and
/// lazily-initialised statics. Warming up before measuring keeps those
/// out of the reported figures.
pub const WARMUP: u32 = 5;

/// One measured benchmark result.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Stable benchmark id (matches the benchmark table in the docs).
    pub id: &'static str,
    /// What the benchmark measures.
    pub description: &'static str,
    /// Number of timed iterations.
    pub iterations: u32,
    /// Median wall-clock duration of one iteration.
    pub median: Duration,
    /// Fastest observed iteration.
    pub min: Duration,
    /// Slowest observed iteration.
    pub max: Duration,
    /// Total wall-clock duration of all timed iterations.
    pub total: Duration,
}

impl BenchmarkResult {
    /// The median duration in nanoseconds.
    pub fn median_nanos(&self) -> u128 {
        self.median.as_nanos()
    }

    /// The median duration in microseconds, as a float, for reporting.
    pub fn median_micros(&self) -> f64 {
        self.median.as_nanos() as f64 / 1000.0
    }

    /// The fastest observed duration in nanoseconds.
    pub fn min_nanos(&self) -> u128 {
        self.min.as_nanos()
    }

    /// The slowest observed duration in nanoseconds.
    pub fn max_nanos(&self) -> u128 {
        self.max.as_nanos()
    }
}

/// Run `f` `WARMUP` times untimed, then `ITERATIONS` times timed, and
/// summarize the timings.
///
/// The median is the reported figure: it is robust to a single scheduling
/// hiccup in a way the mean is not, and unlike the mean it does not
/// require a second pass over the samples to compute.
pub fn measure<F>(id: &'static str, description: &'static str, mut f: F) -> BenchmarkResult
where
    F: FnMut(),
{
    for _ in 0..WARMUP {
        f();
    }

    let mut samples: Vec<Duration> = Vec::with_capacity(ITERATIONS as usize);
    let mut total = Duration::ZERO;
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed();
        total += elapsed;
        samples.push(elapsed);
    }

    samples.sort_unstable();
    // For an even number of samples the statistical median is the mean of
    // the two central order statistics, not the upper of the two. Taking
    // `samples[len / 2]` alone biases the reported figure high by about
    // half a sample step.
    let mid = samples.len() / 2;
    let median = if samples.len().is_multiple_of(2) {
        (samples[mid - 1] + samples[mid]) / 2
    } else {
        samples[mid]
    };
    let min = samples[0];
    let max = samples[samples.len() - 1];

    BenchmarkResult {
        id,
        description,
        iterations: ITERATIONS,
        median,
        min,
        max,
        total,
    }
}

/// The published example's EPF source bytes.
///
/// Embedded directly from the published file (rather than duplicated as a
/// literal here) so the benchmark provably measures the artifact that
/// ships, and so the two can never drift apart.
pub const EXAMPLE_EPF: &str = include_str!("../../../examples/hello-en/package.epf");

/// The published example's ELD source bytes.
///
/// Embedded directly from the published file; see [`EXAMPLE_EPF`].
pub const EXAMPLE_ELD: &str = include_str!("../../../examples/hello-en/Linguistics/phonemes.eld");

/// Build the canonical example archive from the published sources.
///
/// # Errors
///
/// Returns the parse/validation error if the sources do not form a valid
/// package. The sources are compile-time constants that the
/// `example_package` integration test also validates, so this cannot
/// fail for the shipped example.
pub fn build_example_archive() -> Result<Vec<u8>, String> {
    let epf = elang_epf::parse(EXAMPLE_EPF.as_bytes()).map_err(|e| format!("parse EPF: {e}"))?;
    let rep =
        elang_repertoire::repertoire::load().map_err(|e| format!("load repertoire: {e:?}"))?;
    let doc =
        elang_eld::parse(EXAMPLE_ELD.as_bytes(), &rep).map_err(|e| format!("parse ELD: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("validate ELD: {e}"))?;
    let config = PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile: Profile::Canonical,
        format_version: "0.1.0".to_string(),
    };
    write_archive(&config).map_err(|e| format!("write archive: [{}] {e}", e.code()))
}

/// Run the full benchmark suite and return one result per operation.
///
/// # Errors
///
/// Propagates the archive-build failure; see [`build_example_archive`].
pub fn run_suite() -> Result<Vec<BenchmarkResult>, String> {
    let archive = build_example_archive()?;

    // --- inspect: stages 1-7, no resource decompression ---------------
    let inspect = measure(
        "inspect",
        "Parse header/footer/index and build the resource listing (stages 1-7, no decompression).",
        || {
            let pkg = Package::open(&archive).expect("open archive");
            let listing = pkg.inspect();
            // Consume the result so the optimizer cannot elide the work.
            std::hint::black_box(listing.len());
        },
    );

    // --- verify: stages 8-11, full resource integrity -----------------
    let verify = measure(
        "verify",
        "Decode every resource within budget and recompute its SHA-256 (stages 8-11).",
        || {
            let pkg = Package::open(&archive).expect("open archive");
            let verified = pkg.verify().expect("verify archive");
            std::hint::black_box(verified.resource_count());
        },
    );

    // --- validate: stages 8-12, full ELD semantics ---------------------
    let validate = measure(
        "validate",
        "Full ELD semantic validation on top of resource integrity (stages 8-12).",
        || {
            let pkg = Package::open(&archive).expect("open archive");
            let validated = pkg.validate().expect("validate archive");
            std::hint::black_box(validated.root_count());
        },
    );

    // --- canonical pack: build the archive from scratch ---------------
    let canonical_pack = measure(
        "canonical-pack",
        "Build a canonical `.elang` archive (canonical EPF + ELD, integrity manifest, Zstd frames, index, footer).",
        || {
            let bytes = build_example_archive().expect("build example archive");
            std::hint::black_box(bytes.len());
        },
    );

    // --- random access: retrieve one indexed resource ------------------
    // The package is verified once outside the timed loop (verification is
    // measured separately above); the timed operation is the indexed
    // lookup itself.
    let verified = Package::open(&archive)
        .expect("open archive")
        .verify()
        .expect("verify archive");
    let random_access = measure(
        "random-access",
        "Retrieve one indexed resource from a verified package by index (no re-verification).",
        || {
            // Index 2 is `Linguistics/phonemes.eld` in the canonical
            // resource order (bytewise ascending by path).
            let entry = verified.resource_at(2).expect("index 2 exists");
            std::hint::black_box(entry.1.len());
        },
    );

    Ok(vec![
        inspect,
        verify,
        validate,
        canonical_pack,
        random_access,
    ])
}

/// The SHA-256 of the benchmark input archive, as lowercase hex.
///
/// Recorded alongside the results so a future run can prove it measured
/// the same bytes.
pub fn input_digest(archive: &[u8]) -> String {
    hex(&sha256(archive))
}
