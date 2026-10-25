//! `gen_peak_memory` — Phase 16 worst-case peak-memory harness (T1614).
//!
//! Measures the peak resident-set size (`VmHWM`) of the ELANG reader on
//! **worst-case accepted** and **worst-case rejected** packages.
//!
//! # Why each case runs in its own process
//!
//! `VmHWM` is *monotonic* across a process's lifetime: it never goes
//! down. Measuring several scenarios inside one process would therefore
//! report the high-water mark of the **largest** case for every case —
//! the numbers would be silently wrong. This binary solves that by
//! re-executing itself as a one-shot probe (`--probe <case>`), so each
//! scenario gets a fresh address space and an honest `VmHWM`.
//!
//! # What "worst case" means here
//!
//! The format's documented ceilings are far larger than any machine will
//! comfortably hold (a 1 GiB per-resource ceiling, an 8 GiB aggregate
//! ceiling, 25,000 resources). A harness that tried to materialise the
//! literal maximum would need 8 GiB of RAM and would still only prove
//! the reader works on the machine that ran it. This harness instead
//! measures two things that are actually useful:
//!
//! * **Accepted cases** are built to sit at the *documented budget
//!   boundary* for the dimension being stressed (resource count, single
//!   resource size, total payload), while remaining feasible to build in
//!   CI.
//! * **Rejected cases** are archives that *claim* to exceed a ceiling.
//!   The interesting measurement is not the memory used to reject them
//!   but the memory **not** used: a reader that must refuse a 1 GiB
//!   resource without allocating 1 GiB is the whole point of the
//!   budget. The reported figure for a rejected case is therefore the
//!   proof that the claim was refused cheaply.
//!
//! # Platform support
//!
//! Measurement requires Linux `/proc/self/status`. On other platforms
//! the harness records the **methodology** and reports
//! `"measured": false` — it never fabricates a number.
//!
//! # Reproducing a published run
//!
//! ```sh
//! cargo run --release -p elang-conformance --bin gen_peak_memory
//! ```

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::needless_borrow
)]

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use elang_conformance::peak_memory;
use elang_reader::Package;

const REPORT_MD: &str = "docs/peak-memory.md";
const REPORT_JSON: &str = "docs/peak-memory.json";
const REPORT_SHAPE_MD: &str = "docs/peak-memory.shape.md";
const REPORT_SHAPE_JSON: &str = "docs/peak-memory.shape.json";

/// Sentinel that replaces every measured number in the shape variant of
/// the report. The shape variant is the byte-reproducible side of the
/// peak-memory output: the case ids, the expectations, the outcomes,
/// the stable error codes and the budgets are deterministic; the
/// resident-set sizes are not, and so are masked.
const SHAPE: &str = "<MEASURED>";

/// The EPF used by every synthetic case.
///
/// It names the example's phoneme inventory so the archive is a *valid*
/// package apart from the dimension under test.
const EPF_BYTES: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:mem-probe:1.0.0\"\n    name \"Memory Probe\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

/// The ELD used by every synthetic case: two roots, the smallest valid
/// inventory.
const ELD_BYTES: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Probe mode: run exactly one scenario, print one JSON line, exit.
    if let Some(pos) = args.iter().position(|a| a == "--probe") {
        let case = args.get(pos + 1).map(String::as_str).unwrap_or("");
        return run_probe(case);
    }

    // `--shape` writes the structural side of the report only — every
    // measured resident-set number is replaced with `<MEASURED>`. The
    // shape is what the reproducible-build workflow compares, because
    // the kernel's high-water marks are never byte-equal between two
    // runs even on the same machine.
    let shape = args.iter().any(|a| a == "--shape");

    run_driver(shape)
}

/// One measured scenario.
struct Scenario {
    /// Stable id used in the report.
    id: &'static str,
    /// What the scenario builds and what it stresses.
    description: &'static str,
    /// Whether the package is expected to be accepted or rejected.
    expectation: &'static str,
    /// The expected stable rejection code; absent for accepted scenarios.
    expected_error_code: Option<&'static str>,
    /// The documented ceiling this case sits at (or claims to exceed).
    budget: &'static str,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        id: "accepted/baseline",
        description:
            "The smallest valid package (five mandatory resources, no extras) read through the full validate path.",
        expectation: "accepted",
        expected_error_code: None,
        budget: "n/a — the floor of the accepted space",
    },
    Scenario {
        id: "accepted/max-resource-count",
        description:
            "A valid package carrying the maximum number of resources the format allows (25,000 total), each with a small payload.",
        expectation: "accepted",
        expected_error_code: None,
        budget: "MAX_RESOURCES = 25,000",
    },
    Scenario {
        id: "accepted/large-single-resource",
        description:
            "A valid package carrying one extra resource of 64 MiB, which the reader must decode and hash in full.",
        expectation: "accepted",
        expected_error_code: None,
        budget: "MAX_GENERIC_RESOURCE_BYTES = 1 GiB (measured at 1/16 of the ceiling)",
    },
    Scenario {
        id: "rejected/oversized-resource-claim",
        description:
            "An archive whose index claims a resource larger than the 1 GiB per-resource ceiling. The reader must refuse it without allocating the claimed size.",
        expectation: "rejected",
        expected_error_code: Some("INDEX_SIZE_OVER_CEILING"),
        budget: "MAX_GENERIC_RESOURCE_BYTES = 1 GiB",
    },
    Scenario {
        id: "rejected/aggregate-size-claim",
        description:
            "An archive whose index claims an aggregate uncompressed size above the 8 GiB aggregate ceiling. The reader must refuse it without allocating the claimed size.",
        expectation: "rejected",
        expected_error_code: Some("LIMIT_UNCOMPRESSED_AGGREGATE"),
        budget: "MAX_UNCOMPRESSED_AGGREGATE_BYTES = 8 GiB",
    },
    Scenario {
        id: "rejected/resource-count-claim",
        description:
            "An archive whose index claims more resources than the 25,000 ceiling allows. The reader must refuse it without building the claimed index.",
        expectation: "rejected",
        expected_error_code: Some("INDEX_COUNT_OUT_OF_RANGE"),
        budget: "MAX_RESOURCES = 25,000",
    },
];

fn run_driver(shape: bool) -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot locate own executable: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut measurements: Vec<Measurement> = Vec::new();
    let mut platform = Platform::detect();
    let mut all_matched = true;

    for scenario in SCENARIOS {
        eprintln!("measuring {} ...", scenario.id);
        let output = Command::new(&exe).arg("--probe").arg(scenario.id).output();
        let measurement = match output {
            Ok(out) if out.status.success() => match parse_probe_output(&out.stdout) {
                Some(m) => m,
                None => {
                    eprintln!(
                        "probe for {} produced no parseable output (status {:?})",
                        scenario.id,
                        out.status.code()
                    );
                    Measurement::failed(scenario, "probe produced no output")
                }
            },
            Ok(out) => Measurement::failed(
                scenario,
                &format!(
                    "probe exited unsuccessfully (status {:?})",
                    out.status.code()
                ),
            ),
            Err(e) => Measurement::failed(scenario, &format!("probe failed to spawn: {e}")),
        };
        if !measurement.matches_scenario(scenario) {
            eprintln!(
                "probe mismatch for {}: expected {} with code {:?}; got id {:?}, accepted {:?}, code {:?}, error {:?}",
                scenario.id,
                scenario.expectation,
                scenario.expected_error_code,
                measurement.id,
                measurement.accepted,
                measurement.error_code,
                measurement.error,
            );
            all_matched = false;
        }
        measurements.push(measurement);
    }

    // Record whether the platform supports measurement at all.
    platform.memory_supported = peak_memory::current().is_some();

    let (md_path, json_path) = if shape {
        (REPORT_SHAPE_MD, REPORT_SHAPE_JSON)
    } else {
        (REPORT_MD, REPORT_JSON)
    };

    let md = render_markdown(&measurements, &platform, shape);
    if let Err(e) = write_if_changed(md_path, &md) {
        eprintln!("failed to write {md_path}: {e}");
        return ExitCode::FAILURE;
    }
    let json = render_json(&measurements, &platform, shape);
    if let Err(e) = write_if_changed(json_path, &json) {
        eprintln!("failed to write {json_path}: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!(
        "wrote {md_path} and {json_path} ({} scenarios)",
        measurements.len()
    );
    if all_matched {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The result of measuring one scenario.
struct Measurement {
    id: String,
    description: String,
    expectation: String,
    budget: String,
    /// Whether the reader accepted the package.
    accepted: Option<bool>,
    /// The stable error code, when the reader rejected it.
    error_code: Option<String>,
    /// Peak resident set size in KiB.
    vmhwm_kib: Option<u64>,
    /// Resident set size at read time, in KiB.
    vmrss_kib: Option<u64>,
    /// Peak virtual size in KiB.
    vmpeak_kib: Option<u64>,
    /// Size of the archive under test, in bytes.
    archive_bytes: u64,
    /// Free-form note (probe error, etc.).
    error: Option<String>,
}

impl Measurement {
    /// Validate the probe against the scenario that launched it.
    fn matches_scenario(&self, scenario: &Scenario) -> bool {
        self.error.is_none()
            && self.id == scenario.id
            && self.accepted == Some(scenario.expectation == "accepted")
            && self.error_code.as_deref() == scenario.expected_error_code
    }

    /// The outcome label shown in the report.
    fn outcome(&self) -> String {
        if let Some(e) = &self.error {
            return format!("not measured ({e})");
        }
        match self.accepted {
            Some(true) => "accepted".to_string(),
            Some(false) => "rejected".to_string(),
            None => "not measured".to_string(),
        }
    }
}

impl Measurement {
    fn failed(scenario: &Scenario, error: &str) -> Self {
        Measurement {
            id: scenario.id.to_string(),
            description: scenario.description.to_string(),
            expectation: scenario.expectation.to_string(),
            budget: scenario.budget.to_string(),
            accepted: None,
            error_code: None,
            vmhwm_kib: None,
            vmrss_kib: None,
            vmpeak_kib: None,
            archive_bytes: 0,
            error: Some(error.to_string()),
        }
    }
}

/// The host platform and the measurement support it offers.
struct Platform {
    os: String,
    arch: String,
    cpu_brand: String,
    kernel_release: String,
    profile: &'static str,
    memory_supported: bool,
}

impl Platform {
    fn detect() -> Self {
        Platform {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_brand: read_cpu_brand(),
            kernel_release: read_kernel_release(),
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            memory_supported: false,
        }
    }
}

fn read_cpu_brand() -> String {
    fs::read_to_string("/proc/cpuinfo")
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
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

/// Run one scenario in this (already isolated) process and print the
/// measurement as a single JSON line.
fn run_probe(case: &str) -> ExitCode {
    let scenario = match SCENARIOS.iter().find(|s| s.id == case) {
        Some(s) => s,
        None => {
            eprintln!("unknown probe case: {case}");
            return ExitCode::FAILURE;
        }
    };

    let archive = match build_case_archive(scenario.id) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("failed to build case {case}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let archive_bytes = archive.len() as u64;

    // Run the reader. For accepted cases we drive the full validate path;
    // for rejected cases the rejection happens somewhere inside that same
    // path, which is exactly what we want to measure.
    let (accepted, error_code) = match Package::open(&archive).and_then(|p| p.validate()) {
        Ok(validated) => {
            std::hint::black_box(validated.root_count());
            (true, None)
        }
        Err(e) => (false, Some(e.code().to_string())),
    };

    // Read the peak *after* the operation: VmHWM is monotonic, so the
    // post-operation value is the peak reached during it.
    let sample = peak_memory::current();

    let line = match sample {
        Some(s) => format!(
            "{{\"id\":{},\"accepted\":{},\"error_code\":{},\"vmhwm_kib\":{},\"vmrss_kib\":{},\"vmpeak_kib\":{},\"archive_bytes\":{}}}",
            escape_json(&scenario.id),
            accepted,
            match &error_code {
                Some(c) => escape_json(c),
                None => "null".to_string(),
            },
            s.vmhwm_kib(),
            s.vmrss_kib(),
            s.vmpeak_kib(),
            archive_bytes,
        ),
        None => format!(
            "{{\"id\":{},\"accepted\":{},\"error_code\":{},\"vmhwm_kib\":null,\"vmrss_kib\":null,\"vmpeak_kib\":null,\"archive_bytes\":{}}}",
            escape_json(&scenario.id),
            accepted,
            match &error_code {
                Some(c) => escape_json(c),
                None => "null".to_string(),
            },
            archive_bytes,
        ),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

/// Build the archive for one scenario.
fn build_case_archive(case: &str) -> Result<Vec<u8>, String> {
    match case {
        "accepted/baseline" => build_valid_package(Vec::new()),
        "accepted/max-resource-count" => {
            // 24,995 extras + the 5 mandatory resources = exactly 25,000.
            let extras = (0..24_995)
                .map(|i| elang_writer::ExtraResource {
                    path: format!("data/{i:05}.txt"),
                    canonical_bytes: format!("payload {i}").into_bytes(),
                })
                .collect();
            build_valid_package(extras)
        }
        "accepted/large-single-resource" => {
            // 64 MiB of zeros: compresses to almost nothing on the wire,
            // but the reader must decode and hash the full 64 MiB.
            let extras = vec![elang_writer::ExtraResource {
                path: "data/large.bin".to_string(),
                canonical_bytes: vec![0u8; 64 * 1024 * 1024],
            }];
            build_valid_package(extras)
        }
        "rejected/oversized-resource-claim" => build_oversized_resource_claim(),
        "rejected/aggregate-size-claim" => build_aggregate_size_claim(),
        "rejected/resource-count-claim" => build_resource_count_claim(),
        other => Err(format!("unknown case {other}")),
    }
}

/// Build a valid package with the given extra resources.
fn build_valid_package(extras: Vec<elang_writer::ExtraResource>) -> Result<Vec<u8>, String> {
    let epf = elang_epf::parse(EPF_BYTES).map_err(|e| format!("parse EPF: {e}"))?;
    let rep =
        elang_repertoire::repertoire::load().map_err(|e| format!("load repertoire: {e:?}"))?;
    let doc = elang_eld::parse(ELD_BYTES, &rep).map_err(|e| format!("parse ELD: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("validate ELD: {e}"))?;
    let config = elang_writer::PackageConfig {
        epf,
        eld,
        extra_resources: extras,
        profile: elang_writer::Profile::Fast,
        format_version: "0.1.0".to_string(),
    };
    elang_writer::write_archive(&config).map_err(|e| format!("write archive: [{}] {e}", e.code()))
}

/// Locate the index region inside an archive so a claim can be written
/// into it.
///
/// The container layout is: header, resource frames, index, footer. The
/// footer carries the index offset and size, so the index can be found by
/// reading the footer rather than by pattern-matching the bytes.
fn index_region(archive: &[u8]) -> Option<(usize, usize)> {
    // The footer is the final 64 bytes.
    let start = archive.len().checked_sub(64)?;
    let footer = archive.get(start..)?;
    // Footer layout (see elang-container/src/footer.rs):
    //   offset 8  u64 LE  index_offset
    //   offset 16 u64 LE  index_size
    //   offset 24 32 bytes index_sha256
    let index_offset = u64::from_le_bytes(footer.get(8..16)?.try_into().ok()?) as usize;
    let index_size = u64::from_le_bytes(footer.get(16..24)?.try_into().ok()?) as usize;
    Some((index_offset, index_size))
}

/// Iterate over the index entries inside an archive, returning the byte
/// range of each entry's uncompressed_size field.
fn uncompressed_size_fields(archive: &[u8]) -> Result<Vec<std::ops::Range<usize>>, String> {
    uncompressed_size_fields_with_paths(archive)
        .map(|entries| entries.into_iter().map(|(range, _path)| range).collect())
}

/// Like [`uncompressed_size_fields`], but also returns each entry's resource
/// path so callers can target an entry *by path* rather than by a fragile
/// positional index. The index is sorted by raw UTF-8 path, so a positional
/// index is only meaningful if the caller knows the exact sort order of every
/// mandatory and extra entry — which is exactly the kind of assumption that
/// silently inflates the wrong resource when the entry set changes.
fn uncompressed_size_fields_with_paths(
    archive: &[u8],
) -> Result<Vec<(std::ops::Range<usize>, String)>, String> {
    let (index_offset, index_size) =
        index_region(archive).ok_or_else(|| "cannot locate the index region".to_string())?;
    let bytes = archive
        .get(
            index_offset
                ..index_offset
                    .checked_add(index_size)
                    .ok_or_else(|| "index region wraps".to_string())?,
        )
        .ok_or_else(|| "index region is out of range".to_string())?;
    // Index header: 16 bytes (magic 8, version 2, entry_format 2, count 4).
    if bytes.len() < 16 {
        return Err("index is shorter than the 16-byte header".to_string());
    }
    let count = u32::from_le_bytes(
        bytes
            .get(12..16)
            .ok_or_else(|| "count field read".to_string())?
            .try_into()
            .map_err(|_| "count field read".to_string())?,
    ) as usize;
    let mut cursor = 16usize;
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        let path_len = u16::from_le_bytes(
            bytes
                .get(cursor..cursor.saturating_add(2))
                .ok_or_else(|| "index entry is truncated".to_string())?
                .try_into()
                .map_err(|_| "path_len read".to_string())?,
        ) as usize;
        cursor += 2;
        if cursor + path_len + 8 + 8 + 8 + 1 > bytes.len() {
            return Err("index entry is truncated".to_string());
        }
        let path = bytes
            .get(cursor..cursor.saturating_add(path_len))
            .ok_or_else(|| "index entry is truncated".to_string())?
            .to_vec();
        cursor += path_len; // path
        cursor += 8; // frame_offset
        cursor += 8; // compressed_size
        let uncompressed_start = cursor;
        cursor += 8;
        cursor += 1; // compression code
        let absolute = index_offset + uncompressed_start;
        fields.push((
            absolute..absolute + 8,
            String::from_utf8_lossy(&path).into_owned(),
        ));
    }
    Ok(fields)
}

/// Find the positional index of the entry whose resource path is `path`.
///
/// Returns the entry's position in the index (the order the entries are
/// stored in), which is what [`patch_index_uncompressed_size`] expects.
fn index_entry_for_path(archive: &[u8], path: &str) -> Result<usize, String> {
    let entries = uncompressed_size_fields_with_paths(archive)?;
    entries
        .iter()
        .position(|(_range, entry_path)| entry_path == path)
        .ok_or_else(|| format!("no index entry for path {path:?}"))
}

/// Patch one index entry's declared uncompressed size, then recompute the
/// index SHA-256 and patch the footer to match.
fn patch_index_uncompressed_size(
    archive: &mut [u8],
    entry: usize,
    claimed: u64,
) -> Result<(), String> {
    let fields = uncompressed_size_fields(archive)?;
    let range = fields
        .get(entry)
        .ok_or_else(|| {
            format!(
                "index has only {} entries; cannot patch entry {entry}",
                fields.len()
            )
        })?
        .clone();
    let bytes = claimed.to_le_bytes();
    archive
        .get_mut(range.clone())
        .ok_or_else(|| "uncompressed_size field is out of range".to_string())?
        .copy_from_slice(&bytes);
    patch_index_hash(archive)
}

/// Patch the index resource count, then recompute the index SHA-256 and
/// patch the footer to match.
fn patch_index_resource_count(archive: &mut [u8], count: u32) -> Result<(), String> {
    let (index_offset, _index_size) =
        index_region(archive).ok_or_else(|| "cannot locate the index region".to_string())?;
    let bytes = count.to_le_bytes();
    let at = index_offset + 12;
    archive
        .get_mut(at..at + 4)
        .ok_or_else(|| "index count field is out of range".to_string())?
        .copy_from_slice(&bytes);
    patch_index_hash(archive)
}

/// Recompute the SHA-256 of the index region and write it into the
/// footer's `index_sha256` field.
fn patch_index_hash(archive: &mut [u8]) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let (index_offset, index_size) =
        index_region(archive).ok_or_else(|| "cannot locate the index region".to_string())?;
    let bytes = archive
        .get(
            index_offset
                ..index_offset
                    .checked_add(index_size)
                    .ok_or_else(|| "index region wraps".to_string())?,
        )
        .ok_or_else(|| "index region is out of range".to_string())?;
    let digest = Sha256::digest(bytes);
    let footer_at = archive
        .len()
        .checked_sub(64)
        .ok_or_else(|| "archive too short".to_string())?
        + 24;
    archive
        .get_mut(footer_at..footer_at + 32)
        .ok_or_else(|| "footer index_sha256 field is out of range".to_string())?
        .copy_from_slice(&digest);
    Ok(())
}

/// Build an archive whose index claims a resource larger than the 1 GiB
/// per-resource ceiling.
fn build_oversized_resource_claim() -> Result<Vec<u8>, String> {
    let mut archive = build_valid_package(vec![elang_writer::ExtraResource {
        path: "data/claim.bin".to_string(),
        canonical_bytes: vec![0u8; 1024],
    }])?;
    // Target the extra resource by path. The index is sorted by raw UTF-8
    // path and `package.epf` sorts *after* `data/...`, so a positional
    // "last entry" would inflate the mandatory EPF instead of the extra.
    let entry = index_entry_for_path(&archive, "data/claim.bin")
        .map_err(|e| format!("locate entries: {e}"))?;
    // 1 GiB + 1 byte: one byte over the ceiling.
    patch_index_uncompressed_size(&mut archive, entry, 1024 * 1024 * 1024 + 1)?;
    Ok(archive)
}

/// Build an archive whose index claims an aggregate uncompressed size
/// above the 8 GiB aggregate ceiling.
///
/// Each claimed entry sits *exactly at* the 1 GiB per-resource ceiling —
/// legal on its own — so the aggregate check is what has to fire. Nine
/// such entries claim 9 GiB, which is over the 8 GiB aggregate ceiling
/// while no single entry exceeds the per-resource one.
fn build_aggregate_size_claim() -> Result<Vec<u8>, String> {
    let extras: Vec<elang_writer::ExtraResource> = (0..12)
        .map(|i| elang_writer::ExtraResource {
            path: format!("data/claim{i:02}.bin"),
            canonical_bytes: vec![0u8; 16],
        })
        .collect();
    let mut archive = build_valid_package(extras)?;
    // Target the nine "data/claimNN.bin" extras by path. The four mandatory
    // paths ("Header/format.version", "Header/mimetype",
    // "Linguistics/phonemes.eld", "META-INF/integrity.kdl") sort before
    // "data/...", and "package.epf" sorts after it; addressing the extras
    // by path keeps the mandatory resources at their true sizes.
    let one_gib = 1024u64 * 1024 * 1024;
    for offset in 0..9usize {
        let path = format!("data/claim{offset:02}.bin");
        let entry =
            index_entry_for_path(&archive, &path).map_err(|e| format!("locate {path}: {e}"))?;
        patch_index_uncompressed_size(&mut archive, entry, one_gib)?;
    }
    Ok(archive)
}

/// Build an archive whose index claims more resources than the 25,000
/// ceiling allows.
fn build_resource_count_claim() -> Result<Vec<u8>, String> {
    let mut archive = build_valid_package(Vec::new())?;
    // MAX_RESOURCES + 1 = 25,001; the reader's Index::decode checks this
    // before it tries to parse the entries, so the claim is rejected with
    // INDEX_COUNT_OUT_OF_RANGE without ever materialising 25,001 entries.
    patch_index_resource_count(&mut archive, 25_001)?;
    Ok(archive)
}

/// Parse the single JSON line a probe printed.
fn parse_probe_output(stdout: &[u8]) -> Option<Measurement> {
    let text = std::str::from_utf8(stdout).ok()?;
    let line = text.lines().find(|l| l.starts_with('{'))?;
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let id = value.get("id")?.as_str()?.to_string();
    let scenario = SCENARIOS.iter().find(|s| s.id == id)?;
    Some(Measurement {
        id,
        description: scenario.description.to_string(),
        expectation: scenario.expectation.to_string(),
        budget: scenario.budget.to_string(),
        accepted: value.get("accepted").and_then(serde_json::Value::as_bool),
        error_code: value
            .get("error_code")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        vmhwm_kib: value.get("vmhwm_kib").and_then(serde_json::Value::as_u64),
        vmrss_kib: value.get("vmrss_kib").and_then(serde_json::Value::as_u64),
        vmpeak_kib: value.get("vmpeak_kib").and_then(serde_json::Value::as_u64),
        archive_bytes: value
            .get("archive_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        error: None,
    })
}

fn write_if_changed(path: &str, contents: &str) -> std::io::Result<()> {
    if let Ok(existing) = fs::read_to_string(Path::new(path)) {
        if existing == contents {
            return Ok(());
        }
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn render_markdown(measurements: &[Measurement], platform: &Platform, shape: bool) -> String {
    let mut out = String::new();
    out.push_str("# ELANG v0.1.0 peak-memory report\n\n");
    if shape {
        out.push_str("> **Shape variant.** Every measured resident-set number is replaced with `<MEASURED>` so the structure of this report can be byte-compared across runs and machines. The shape is what the reproducible-build workflow checks; the real numbers live in `docs/peak-memory.md`.\n\n");
    } else {
        out.push_str("> Generated by `gen_peak_memory`. Every case runs in its own process so `VmHWM` is a true per-case peak rather than a running maximum across cases.\n\n");
    }

    out.push_str("## Environment\n\n");
    if shape {
        out.push_str(&format!("- **OS / arch:** {SHAPE} / {SHAPE}\n"));
        out.push_str(&format!("- **CPU:** {SHAPE}\n"));
        out.push_str(&format!("- **Kernel:** {SHAPE}\n"));
        out.push_str(&format!("- **Build profile:** `{SHAPE}`\n"));
        out.push_str(&format!("- **Measurement method:** {SHAPE}\n\n"));
    } else {
        out.push_str(&format!(
            "- **OS / arch:** {} / {}\n",
            platform.os, platform.arch
        ));
        out.push_str(&format!("- **CPU:** {}\n", platform.cpu_brand));
        out.push_str(&format!("- **Kernel:** {}\n", platform.kernel_release));
        out.push_str(&format!("- **Build profile:** `{}`\n", platform.profile));
        out.push_str(&format!(
            "- **Measurement method:** {}\n\n",
            if platform.memory_supported {
                "Linux `/proc/self/status` `VmHWM` (peak resident set size), sampled immediately after the operation completes"
            } else {
                "**not available on this platform** — methodology only; no number is reported"
            }
        ));
    }

    out.push_str("## Measurements\n\n");
    out.push_str("| Case | Expectation | Outcome | Archive bytes | `VmHWM` (KiB) | `VmHWM` (MiB) | `VmRSS` (KiB) | `VmPeak` (KiB) | Error code | Budget |\n");
    out.push_str("|---|---|---|---:|---:|---:|---:|---:|---|---|\n");
    for m in measurements {
        let outcome = m.outcome();
        // The shape variant is the portable half of the report:
        // every resident-set column carries the sentinel regardless
        // of whether the host happens to expose `/proc/self/status`.
        // A `None` here would mean "this platform could not measure",
        // and that fact is already carried by `memory_supported` and
        // the methodology line — using a second sentinel would make
        // the shape differ between Linux and every other platform,
        // which is the very thing the shape is supposed to hide.
        let measured = |v: Option<u64>| -> String {
            if shape {
                SHAPE.to_string()
            } else {
                v.map(|k| k.to_string()).unwrap_or_else(|| "—".to_string())
            }
        };
        let vmhwm = measured(m.vmhwm_kib);
        let vmhwm_mib = if shape {
            SHAPE.to_string()
        } else {
            match m.vmhwm_kib {
                Some(k) => format!("{:.1}", k as f64 / 1024.0),
                None => "—".to_string(),
            }
        };
        let vmrss = measured(m.vmrss_kib);
        let vmpeak = measured(m.vmpeak_kib);
        let code = m.error_code.clone().unwrap_or_else(|| "—".to_string());
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | `{}` | {} |\n",
            m.id,
            m.expectation,
            outcome,
            m.archive_bytes,
            vmhwm,
            vmhwm_mib,
            vmrss,
            vmpeak,
            code,
            m.budget
        ));
    }
    out.push('\n');

    out.push_str("## What each case proves\n\n");
    for m in measurements {
        out.push_str(&format!("### `{}`\n\n", m.id));
        out.push_str(&format!("{}\n\n", m.description));
        let rss: String = if shape {
            SHAPE.to_string()
        } else {
            m.vmhwm_kib.map(|k| k.to_string()).unwrap_or_default()
        };
        match m.accepted {
            Some(true) => out.push_str(&format!(
                "**Result:** accepted, peak RSS {rss} KiB.\n\n"
            )),
            Some(false) => out.push_str(&format!(
                "**Result:** rejected with `{}` at a peak RSS of {rss} KiB. The rejection cost is the point: the reader refused the claim without materialising the size that was claimed.\n\n",
                m.error_code.clone().unwrap_or_default(),
            )),
            None => out.push_str("**Result:** not measured.\n\n"),
        }
    }

    out.push_str("## Evaluation against the documented limits\n\n");
    out.push_str(
        "| Limit | Ceiling | Worst observed in this run | Headroom |\n|---|---:|---:|---|\n",
    );
    out.push_str("| `MAX_RESOURCES` (resources per archive) | 25,000 | 25,000 (the `accepted/max-resource-count` case) | at the ceiling by construction |\n");
    out.push_str("| `MAX_GENERIC_RESOURCE_BYTES` (per resource) | 1 GiB | 64 MiB decoded in `accepted/large-single-resource` | 16x below the ceiling |\n");
    out.push_str("| `MAX_UNCOMPRESSED_AGGREGATE_BYTES` (per archive) | 8 GiB | ~64 MiB decoded in the same case | ~128x below the ceiling |\n");
    out.push_str("| `MAX_COMPRESSED_ARCHIVE_BYTES` (per archive) | 6 GiB | see the `archive_bytes` column | — |\n\n");

    out.push_str("## Limitations and disclosures\n\n");
    out.push_str("- The accepted cases sit at the documented ceiling for **resource count** and at a fraction of it for **byte size**. A literal 1 GiB single-resource case and a literal 8 GiB aggregate case were not run: they need the corresponding amount of RAM to be *valid* on the host, and a number produced on a machine that could not hold the payload would not describe the reader's behaviour on a machine that can.\n");
    out.push_str("- The rejected cases measure the **cost of refusal**, not the cost of acceptance. The interesting property is that the reader rejects a 1 GiB / 8 GiB claim in a few MiB of RSS — that is the budget doing its job.\n");
    out.push_str("- `VmHWM` is a high-water mark for the whole process, so it includes the harness's own overhead (the archive bytes are held in memory while it is read). The absolute figures are therefore an upper bound on the reader's own footprint, not a measurement of it in isolation.\n");
    out.push_str("- On platforms without `/proc/self/status` the methodology above still applies but no number is reported; the release notes ship the Linux figures and state the non-Linux position explicitly.\n");
    out.push_str(
        r##"
## Reproducibility

This report mixes two kinds of content: a deterministic half (the
scenario ids, the expectations, the accept/reject outcomes, the stable
error codes, the budgets) and a measured half (the `VmHWM` / `VmRSS` /
`VmPeak` numbers). The measured half is a function of the kernel and
the moment of the run, so it is **not** reproducible.

The generator therefore also has a `--shape` mode that writes
`docs/peak-memory.shape.md` and `docs/peak-memory.shape.json`: the same
report with every measured number replaced by a sentinel and every
host-derived environment value masked. The shape files are what the
`reproducible-build` CI workflow compares across two clean containers,
and `crates/elang-conformance/tests/measurement_shape.rs` mirrors that
check locally.
"##,
    );

    out
}

fn render_json(measurements: &[Measurement], platform: &Platform, shape: bool) -> String {
    let entries: Vec<String> = measurements
        .iter()
        .map(|m| {
            format!(
                "    {{\"id\":{},\"description\":{},\"expectation\":{},\"accepted\":{},\"error_code\":{},\
                 \"vmhwm_kib\":{},\"vmrss_kib\":{},\"vmpeak_kib\":{},\"archive_bytes\":{}}}",
                escape_json(&m.id),
                escape_json(&m.description),
                escape_json(&m.expectation),
                match m.accepted {
                    Some(v) => v.to_string(),
                    None => "null".to_string(),
                },
                match &m.error_code {
                    Some(c) => escape_json(c),
                    None => "null".to_string(),
                },
                // Shape mode keeps the key and nulls the value: the
                // key set stays identical across runs, the numbers do
                // not survive.
                match m.vmhwm_kib {
                    Some(_) if shape => "null".to_string(),
                    Some(v) => v.to_string(),
                    None => "null".to_string(),
                },
                match m.vmrss_kib {
                    Some(_) if shape => "null".to_string(),
                    Some(v) => v.to_string(),
                    None => "null".to_string(),
                },
                match m.vmpeak_kib {
                    Some(_) if shape => "null".to_string(),
                    Some(v) => v.to_string(),
                    None => "null".to_string(),
                },
                m.archive_bytes,
            )
        })
        .collect();

    format!(
        "{{\n  \"method\": \"linux_proc_self_status_vmhwm\",\n  \"memory_supported\": {supported},\n\
         \n  \"environment\": {{\n    \"os\": {os},\n    \"arch\": {arch},\n    \"cpu_brand\": {cpu},\n    \
         \"kernel_release\": {kernel},\n    \"profile\": {profile}\n  }},\n\
         \n  \"measurements\": [\n{entries}\n  ]\n}}\n",
        // In shape mode the environment block and `memory_supported`
        // are nulled so the shape is identical on any host, any
        // toolchain and any platform; the key set stays the same.
        supported = if shape { "null".to_owned() } else { platform.memory_supported.to_string() },
        os = if shape { "null".to_owned() } else { escape_json(&platform.os) },
        arch = if shape { "null".to_owned() } else { escape_json(&platform.arch) },
        cpu = if shape { "null".to_owned() } else { escape_json(&platform.cpu_brand) },
        kernel = if shape { "null".to_owned() } else { escape_json(&platform.kernel_release) },
        profile = if shape { "null".to_owned() } else { escape_json(&platform.profile) },
        entries = entries.join(",\n"),
    )
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(scenario: &Scenario, accepted: bool, code: Option<&str>) -> Measurement {
        let output = serde_json::json!({
            "id": scenario.id,
            "accepted": accepted,
            "error_code": code,
        });
        parse_probe_output(&serde_json::to_vec(&output).expect("serialize probe"))
            .expect("parse probe")
    }

    #[test]
    fn expected_outcomes_match_even_without_memory_support() {
        for scenario in SCENARIOS {
            let measurement = probe(
                scenario,
                scenario.expectation == "accepted",
                scenario.expected_error_code,
            );
            assert!(measurement.matches_scenario(scenario), "{}", scenario.id);
        }
    }

    #[test]
    fn wrong_acceptance_or_rejection_code_does_not_match() {
        for scenario in SCENARIOS {
            let wrong_acceptance = probe(
                scenario,
                scenario.expectation != "accepted",
                scenario.expected_error_code,
            );
            assert!(
                !wrong_acceptance.matches_scenario(scenario),
                "{}",
                scenario.id
            );
            let wrong_code = probe(scenario, scenario.expectation == "accepted", Some("WRONG"));
            assert!(!wrong_code.matches_scenario(scenario), "{}", scenario.id);
            if scenario.expected_error_code.is_some() {
                assert!(!probe(scenario, false, None).matches_scenario(scenario));
            }
        }
    }

    #[test]
    fn failed_missing_or_misdirected_probe_does_not_match() {
        let mut scenarios = SCENARIOS.iter();
        let scenario = scenarios.next().expect("baseline scenario");
        let other = scenarios.next().expect("second accepted scenario");
        assert!(!Measurement::failed(scenario, "probe failed").matches_scenario(scenario));
        assert!(!probe(other, true, None).matches_scenario(scenario));
        let missing_outcome = serde_json::json!({ "id": scenario.id });
        let measurement = parse_probe_output(
            &serde_json::to_vec(&missing_outcome).expect("serialize missing outcome"),
        )
        .expect("parse missing outcome");
        assert!(!measurement.matches_scenario(scenario));
    }
}
