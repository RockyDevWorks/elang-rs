//! `gen_benchmarks` — Phase 16 deterministic performance harness (T1613, T1614).
//!
//! Runs the five principal package operations against the published
//! example, measures the median wall-clock time per iteration, records the
//! Linux `/proc/self/status` peak-memory numbers, and writes:
//!
//! * `docs/benchmarks.md` — human-readable, lives next to the docs site
//!   build.
//! * `docs/benchmarks.json` — machine-readable, includes the environment
//!   so a reader can tell when the numbers came from somewhere other than
//!   the documented build.
//!
//! # Why no command-line arguments
//!
//! The harness is intentionally parameter-free: there are no `--iter`,
//! `--input`, or `--platform` knobs. Allowing those would mean a recorded
//! number might have come from a run with different settings than the
//! release gate, which defeats reproducibility. The environment is
//! captured into the JSON output instead.
//!
//! # Why `release`
//!
//! Release-mode builds strip assertions and enable the optimizer's
//! full LTO/Codegen-Units=1 treatment, which is what the release gate
//! cares about. A `dev` run gives the relative shape of the curve but
//! its numbers are not the release numbers. The harness documents the
//! build profile in the JSON output so a reader can confirm.
//!
//! # Reproducing a published run
//!
//! ```sh
//! cargo run --release -p elang-conformance --bin gen_benchmarks
//! ```
//!
//! Re-running on the same machine and the same `release` build
//! reproduces the same numbers within the platform clock's resolution.

#![allow(clippy::arithmetic_side_effects)]
// The harness is a build-time tool: every failure mode it can hit means
// the corpus the release gate cares about cannot be measured. A panic is
// the intended fail-closed mode here, mirroring the other conformance
// generators (`gen_canonical_goldens`, `gen_normative_manifest`).
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use elang_conformance::benchmarks::{self, input_digest};
use elang_conformance::peak_memory;

const BENCHMARKS_MD: &str = "docs/benchmarks.md";
const BENCHMARKS_JSON: &str = "docs/benchmarks.json";
const BENCHMARKS_SHAPE_MD: &str = "docs/benchmarks.shape.md";
const BENCHMARKS_SHAPE_JSON: &str = "docs/benchmarks.shape.json";

/// Sentinel that replaces every measured number in the shape variant
/// of the report. The shape variant is the byte-reproducible side of
/// the benchmark output: the op names, the iteration counts, the
/// environment block, and the methodology text are deterministic; the
/// durations are not, and so are masked.
const SHAPE: &str = "<MEASURED>";

fn main() -> ExitCode {
    // `--shape` writes the structural side of the report only — every
    // measured numeric value is replaced with `<MEASURED>`. The shape
    // is what the reproducible-build workflow compares, because the
    // wall-clock numbers themselves are never byte-equal between two
    // runs even on the same machine.
    let shape = std::env::args().any(|a| a == "--shape");

    let started = Instant::now();

    // The benchmark input is the canonical example archive. Recording
    // its SHA-256 alongside the results lets a reader prove the same
    // bytes were measured.
    let archive = match benchmarks::build_example_archive() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("failed to build example archive: {e}");
            return ExitCode::FAILURE;
        }
    };
    let input_sha256 = input_digest(&archive);
    let input_bytes = archive.len();

    // Wall-clock timings.
    let results = match benchmarks::run_suite() {
        Ok(results) => results,
        Err(e) => {
            eprintln!("benchmark suite failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Peak-memory numbers from /proc/self/status (Linux only).
    let memory = peak_memory::current();
    let platform = platform_summary();

    let (md_path, json_path) = if shape {
        (BENCHMARKS_SHAPE_MD, BENCHMARKS_SHAPE_JSON)
    } else {
        (BENCHMARKS_MD, BENCHMARKS_JSON)
    };

    // Render Markdown.
    let md = render_markdown(
        &results,
        &memory,
        &input_sha256,
        input_bytes,
        &platform,
        shape,
    );
    if let Err(e) = write_if_changed(md_path, &md) {
        eprintln!("failed to write {md_path}: {e}");
        return ExitCode::FAILURE;
    }

    // Render JSON.
    let json = render_json(
        &results,
        &memory,
        &input_sha256,
        input_bytes,
        &platform,
        started.elapsed(),
        shape,
    );
    if let Err(e) = write_if_changed(json_path, &json) {
        eprintln!("failed to write {json_path}: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!(
        "wrote {md_path} and {json_path} ({} benchmarks, {}s elapsed)",
        results.len(),
        started.elapsed().as_secs()
    );
    ExitCode::SUCCESS
}

/// Summary of the build and host platform the harness ran on.
struct PlatformSummary {
    target_triple: String,
    cpu_brand: String,
    kernel_release: String,
    profile: &'static str,
    rustc_version: String,
}

fn platform_summary() -> PlatformSummary {
    PlatformSummary {
        target_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        cpu_brand: read_cpu_brand(),
        kernel_release: read_kernel_release(),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        rustc_version: rustc_version_runtime(),
    }
}

fn read_cpu_brand() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn read_kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn rustc_version_runtime() -> String {
    // Queried from the toolchain that built this binary. If the toolchain
    // is not on PATH at run time the field degrades to "<unknown>" rather
    // than reporting a stale hand-maintained value.
    match std::process::Command::new("rustc").arg("-V").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "<unknown>".to_string(),
    }
}

fn write_if_changed(path: &str, contents: &str) -> std::io::Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return Ok(());
        }
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn render_markdown(
    results: &[benchmarks::BenchmarkResult],
    memory: &Option<peak_memory::PeakMemorySample>,
    input_sha256: &str,
    input_bytes: usize,
    platform: &PlatformSummary,
    shape: bool,
) -> String {
    let mut out = String::new();

    out.push_str("# ELANG v0.1.0 benchmark results\n\n");
    if shape {
        out.push_str("> **Shape variant.** Every measured numeric value is replaced with `<MEASURED>` so the structure of this report can be byte-compared across runs and machines. The shape is what the reproducible-build workflow checks; the real numbers live in `docs/benchmarks.md`.\n\n");
    } else {
        out.push_str("> Generated by `gen_benchmarks`. The numbers below are a function of the binary, the build profile and the machine; they are not a performance guarantee.\n\n");
    }

    // --- environment ---
    //
    // Every value in this block is derived from the machine or from the
    // build, not from the code under test. The shape variant masks the
    // whole block so the shape is identical on any host, any toolchain
    // and in any profile; that is what makes the committed shape file
    // a portable artefact rather than a snapshot of one laptop.
    out.push_str("## Environment\n\n");
    if shape {
        out.push_str(&format!("- **Build profile:** `{SHAPE}`\n"));
        out.push_str(&format!("- **Target triple:** `{SHAPE}`\n"));
        out.push_str(&format!("- **CPU:** {SHAPE}\n"));
        out.push_str(&format!("- **Kernel:** {SHAPE}\n"));
        out.push_str(&format!("- **rustc:** {SHAPE}\n"));
    } else {
        out.push_str(&format!("- **Build profile:** `{}`\n", platform.profile));
        out.push_str(&format!(
            "- **Target triple:** `{}`\n",
            platform.target_triple
        ));
        out.push_str(&format!("- **CPU:** {}\n", platform.cpu_brand));
        out.push_str(&format!("- **Kernel:** {}\n", platform.kernel_release));
        out.push_str(&format!("- **rustc:** {}\n", platform.rustc_version));
    }
    out.push_str(&format!(
        "- **Input archive:** `examples/hello-en/hello-en.elang` ({} bytes, sha256 `{}`)\n",
        input_bytes, input_sha256
    ));
    out.push_str(&format!(
        "- **Iterations per benchmark:** {} (after {} warm-up iterations)\n",
        benchmarks::ITERATIONS,
        benchmarks::WARMUP
    ));
    out.push_str("- **Reported statistic:** median (robust to a single scheduling hiccup)\n\n");

    // --- timings ---
    out.push_str("## Wall-clock timings\n\n");
    out.push_str("| Benchmark | Median (ns) | Median (µs) | Min (ns) | Max (ns) | Iterations | What it measures |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---|\n");
    for r in results {
        let (median_ns, median_us, min_ns, max_ns): (String, String, String, String) = if shape {
            (
                SHAPE.to_owned(),
                SHAPE.to_owned(),
                SHAPE.to_owned(),
                SHAPE.to_owned(),
            )
        } else {
            (
                r.median_nanos().to_string(),
                format!("{:.2}", r.median_micros()),
                r.min_nanos().to_string(),
                r.max_nanos().to_string(),
            )
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            r.id, median_ns, median_us, min_ns, max_ns, r.iterations, r.description
        ));
    }
    out.push('\n');

    // --- peak memory ---
    //
    // The shape variant unconditionally emits the measured-memory
    // table with every value replaced by the sentinel, regardless of
    // whether the host happens to expose `/proc/self/status`. Without
    // this, the shape would diverge between Linux (measured) and
    // macOS / Windows (unmeasured) — the opposite of portable.
    out.push_str("## Peak memory (Linux `/proc/self/status`)\n\n");
    if shape {
        out.push_str("| Metric | Value |\n|---|---:|\n");
        out.push_str(&format!("| `VmHWM` (peak resident set) | {SHAPE} |\n"));
        out.push_str(&format!(
            "| `VmRSS` (resident set at read time) | {SHAPE} |\n"
        ));
        out.push_str(&format!("| `VmPeak` (peak virtual size) | {SHAPE} |\n"));
        out.push_str(
            "\n`VmHWM` is the peak resident-set size the kernel attributes to the harness process; it is monotonic across the process lifetime and is the number that belongs in the release notes.\n\n",
        );
    } else {
        match memory {
            Some(sample) if sample.is_measured() => {
                out.push_str("| Metric | Value |\n|---|---:|\n");
                out.push_str(&format!(
                    "| `VmHWM` (peak resident set) | {} KiB ({:.1} MiB) |\n",
                    sample.vmhwm_kib(),
                    sample.vmhwm_mib()
                ));
                out.push_str(&format!(
                    "| `VmRSS` (resident set at read time) | {} KiB ({:.1} MiB) |\n",
                    sample.vmrss_kib(),
                    sample.vmrss_mib()
                ));
                out.push_str(&format!(
                    "| `VmPeak` (peak virtual size) | {} KiB ({:.1} MiB) |\n",
                    sample.vmpeak_kib(),
                    sample.vmpeak_mib()
                ));
                out.push_str(
                    "\n`VmHWM` is the peak resident-set size the kernel attributes to the harness process; it is monotonic across the process lifetime and is the number that belongs in the release notes.\n\n",
                );
            }
            Some(_) | None => {
                out.push_str(
                    "Peak memory was not measured on this run because `/proc/self/status` is not available on this platform. The release notes for v0.1.0 only ship Linux numbers; non-Linux platforms are documented as **method only**.\n\n",
                );
            }
        }
    }

    // --- methodology ---
    out.push_str("## Methodology\n\n");
    out.push_str("- The benchmark builds the canonical example archive from `examples/hello-en/{package.epf, Linguistics/phonemes.eld}` and measures five operations on those bytes.\n");
    out.push_str("- Each operation runs `WARMUP` (5) untimed iterations before the timed `ITERATIONS` (50) to amortise one-off setup costs (page faults, allocator warm-up, lazy statics).\n");
    out.push_str("- The reported figure is the **median** wall-clock duration; the min/max span is included so a reader can see how tightly the timings cluster.\n");
    out.push_str("- The `random-access` benchmark reuses one verified package — verification is measured separately; the timed operation is the indexed lookup itself.\n\n");

    out.push_str("## Limitations\n\n");
    out.push_str("- These are **micro-benchmarks of one input**. They do not characterise performance on the worst-case inputs the implementation accepts (25,000 resources / 1 GiB per resource / 256-root phoneme inventories). Those boundary cases are documented by [`docs/security-limits.md`](security-limits.md) and require a different harness to measure honestly.\n");
    out.push_str("- `VmHWM` is a single sample of one process on one kernel. Two runs of the same workload on the same machine can differ by a few MiB depending on allocator growth patterns and page reclamation at unfortunate moments.\n");
    out.push_str("- The benchmark is intentionally dependency-free. There is no `--iter` / `--input` knob; allowing those would let the recorded numbers come from a different run than the release gate.\n");
    out.push_str(
        r##"
## Reproducibility

This report mixes two kinds of content: a deterministic half (the
benchmark ids, the iteration counts, the environment block, the
methodology) and a measured half (the durations, the resident-set
numbers, the generation timestamp). The measured half is a function of
the machine and the moment of the run, so it is **not** reproducible.

The generator therefore also has a `--shape` mode that writes
`docs/benchmarks.shape.md` and `docs/benchmarks.shape.json`: the same
report with every measured number replaced by a sentinel and every
host-derived environment value masked. The shape files are what the
`reproducible-build` CI workflow compares across two clean containers,
and `crates/elang-conformance/tests/measurement_shape.rs` mirrors that
check locally.
"##,
    );

    out
}

fn render_json(
    results: &[benchmarks::BenchmarkResult],
    memory: &Option<peak_memory::PeakMemorySample>,
    input_sha256: &str,
    input_bytes: usize,
    platform: &PlatformSummary,
    elapsed: std::time::Duration,
    shape: bool,
) -> String {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let results_json: Vec<String> = results
        .iter()
        .map(|r| {
            // In shape mode every measured field becomes JSON `null`:
            // the key stays present (so the schema is unchanged) but
            // the value carries no run-dependent information.
            let (median_ns, median_us, min_ns, max_ns, total_ns): (
                String,
                String,
                String,
                String,
                String,
            ) = if shape {
                (
                    "null".into(),
                    "null".into(),
                    "null".into(),
                    "null".into(),
                    "null".into(),
                )
            } else {
                (
                    r.median_nanos().to_string(),
                    format!("{:.3}", r.median_micros()),
                    r.min_nanos().to_string(),
                    r.max_nanos().to_string(),
                    r.total.as_nanos().to_string(),
                )
            };
            format!(
                "    {{\"id\":\"{}\",\"description\":{desc},\"iterations\":{},\
                 \"median_ns\":{median_ns},\"median_us\":{median_us},\"min_ns\":{min_ns},\
                 \"max_ns\":{max_ns},\"total_ns\":{total_ns}}}",
                r.id,
                r.iterations,
                desc = escape_json(r.description),
            )
        })
        .collect();

    let memory_json = if shape {
        // Shape mode is the portable half: always emit the measured
        // object with every field nulled, so the shape is identical
        // on every platform. The `measured: true` flag stays so the
        // key set matches a measured Linux run exactly; only the
        // numeric values are hidden.
        "{\"measured\":true,\"vmhwm_kib\":null,\"vmrss_kib\":null,\"vmpeak_kib\":null,\
         \"vmhwm_mib\":null,\"vmpeak_mib\":null}"
            .into()
    } else {
        match memory {
            Some(sample) if sample.is_measured() => {
                format!(
                    "{{\n\"measured\":true,\"vmhwm_kib\":{},\"vmrss_kib\":{},\"vmpeak_kib\":{},\
                 \"vmhwm_mib\":{:.3},\"vmpeak_mib\":{:.3}}}",
                    sample.vmhwm_kib(),
                    sample.vmrss_kib(),
                    sample.vmpeak_kib(),
                    sample.vmhwm_mib(),
                    sample.vmpeak_mib()
                )
            }
            _ => "{\"measured\":false,\"reason\":\"linux_proc_self_status_unavailable\"}".into(),
        }
    };

    // The generation timestamp and the harness wall time are measured
    // values, so the shape variant pins both to zero.
    let (now, elapsed) = if shape {
        ("0".to_owned(), "0".to_owned())
    } else {
        (now_unix.to_string(), elapsed.as_nanos().to_string())
    };

    format!(
        "{{\n  \"generated_at_unix\": {now},\n  \"elapsed_ns\": {elapsed},\
         \n  \"environment\": {{\n    \"target_triple\": {target},\n    \"cpu_brand\": {cpu},\
         \n    \"kernel_release\": {kernel},\n    \"profile\": {profile},\n    \"rustc_version\": {rustc}\n  }},\n\
         \n  \"input\": {{\n    \"path\": \"examples/hello-en/hello-en.elang\",\n    \"bytes\": {input_bytes},\n    \"sha256\": \"{input_sha256}\"\n  }},\n\
         \n  \"iterations\": {iters},\n  \"warmup\": {warmup},\n\
         \n  \"results\": [\n{results}\n  ],\n\
         \n  \"memory\": {memory}\n}}\n",
        now = now,
        elapsed = elapsed,
        // The environment block is host- and build-derived, so the
        // shape variant nulls it out; the key set stays identical.
        target = if shape { "null".to_owned() } else { escape_json(&platform.target_triple) },
        cpu = if shape { "null".to_owned() } else { escape_json(&platform.cpu_brand) },
        kernel = if shape { "null".to_owned() } else { escape_json(&platform.kernel_release) },
        profile = if shape { "null".to_owned() } else { escape_json(platform.profile) },
        rustc = if shape { "null".to_owned() } else { escape_json(&platform.rustc_version) },
        input_bytes = input_bytes,
        input_sha256 = input_sha256,
        iters = benchmarks::ITERATIONS,
        warmup = benchmarks::WARMUP,
        results = results_json.join(",\n"),
        memory = memory_json,
    )
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len().saturating_add(2));
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
