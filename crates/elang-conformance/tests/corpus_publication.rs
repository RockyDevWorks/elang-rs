//! Drift test: the published conformance corpus must match what the
//! conformance crate's source declares.
//!
//! Every archive-level and document-level fixture published by
//! `gen_conformance_corpus` has a path inside `fixtures/`. This test
//! asserts:
//!
//! * **A fresh generation reproduces the committed corpus byte for
//!   byte.** The generator is re-run with its working directory pointed
//!   at a scratch tree under `target/`, and every file it emits is
//!   compared byte-for-byte against the committed copy. A generator
//!   change that was not followed by a regeneration fails here, with
//!   the exact file that drifted.
//! * Every published fixture has the expected file on disk, and its
//!   committed SHA-256 matches the bytes the generator would publish.
//! * Every published archive / document fixture's committed bytes match
//!   the in-crate source it was built from.
//! * The `counts` block in the index matches what the catalog says it
//!   should.
//! * The `modes` recorded for every published fixture match the reader
//!   modes that actually observe the fixture's outcome.
//!
//! The test never writes into the repository. The generator runs
//! against a scratch directory, so a drifted corpus cannot be "fixed"
//! by the very test that is supposed to detect the drift, and two
//! concurrently-running tests can never observe a half-written
//! artefact. The developer regenerates the corpus explicitly via
//! `cargo run -p elang-conformance --bin gen_conformance_corpus`.

#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use elang_conformance::binary_vectors::{hex, sha256};
use elang_conformance::catalog;
use elang_conformance::corpus::{
    DocumentExpectation, DocumentFixture, Expectation, Fixture, Schema,
};

const CORPUS_INDEX: &str = "fixtures/corpus_index.json";
const SCHEMA_PATH: &str = "fixtures/expected-result.schema.json";
const FIXTURE_ROOT: &str = "fixtures";

/// The repository root, resolved from the crate manifest directory so
/// the test works regardless of the working directory `cargo test`
/// happens to use.
fn release_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

/// Resolve a repository-relative path.
fn repo_path(rel: &str) -> PathBuf {
    release_root().join(rel)
}

#[test]
fn schema_and_index_are_committed() {
    assert!(
        repo_path(SCHEMA_PATH).exists(),
        "missing {SCHEMA_PATH}: run `cargo run -p elang-conformance --bin gen_conformance_corpus` to publish it"
    );
    assert!(
        repo_path(CORPUS_INDEX).exists(),
        "missing {CORPUS_INDEX}: run `cargo run -p elang-conformance --bin gen_conformance_corpus` to publish it"
    );
}

#[test]
fn schema_parses_as_json_object() {
    let body = fs::read_to_string(repo_path(SCHEMA_PATH)).expect("read schema");
    let value: serde_json::Value = serde_json::from_str(&body).expect("schema is JSON");
    let obj = value.as_object().expect("schema is an object");
    assert_eq!(
        obj.get("$schema").and_then(serde_json::Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "the schema must declare draft 2020-12"
    );
    assert!(
        obj.contains_key("properties"),
        "schema must define properties"
    );
}

#[test]
fn counts_match_the_catalog() {
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path(CORPUS_INDEX)).expect("read index"))
            .expect("index is JSON");
    let counts = index.get("counts").expect("counts object");
    let catalog = catalog::catalog();
    let mut published = 0usize;
    let mut vector = 0usize;
    let mut meta = 0usize;
    let sources = published_sources();
    for entry in &catalog {
        if sources.contains_key(&entry.id) {
            published += 1;
        } else if is_meta(&entry.id) {
            meta += 1;
        } else {
            vector += 1;
        }
    }
    assert_eq!(
        counts.get("total").and_then(serde_json::Value::as_u64),
        Some(catalog.len() as u64)
    );
    assert_eq!(
        counts.get("published").and_then(serde_json::Value::as_u64),
        Some(published as u64)
    );
    assert_eq!(
        counts.get("vector").and_then(serde_json::Value::as_u64),
        Some(vector as u64)
    );
    assert_eq!(
        counts.get("meta").and_then(serde_json::Value::as_u64),
        Some(meta as u64)
    );
}

#[test]
fn a_fresh_generation_reproduces_the_committed_corpus_byte_for_byte() {
    // The strongest drift guarantee available: run the real generator
    // into a scratch tree and diff every byte it produces against the
    // committed corpus. The scratch tree keeps the repository read-only,
    // so this test can never "repair" the drift it is looking for.
    let scratch = scratch_dir("corpus-drift");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("create scratch directory");

    let status = Command::new(env!("CARGO_BIN_EXE_gen_conformance_corpus"))
        .current_dir(&scratch)
        .status()
        .expect("spawn gen_conformance_corpus");
    assert!(status.success(), "gen_conformance_corpus exited non-zero");

    let generated = scratch.join("fixtures");
    assert!(
        generated.is_dir(),
        "the generator did not produce a fixtures/ tree in the scratch directory"
    );

    let mut files = Vec::new();
    collect_files(&generated, &generated, &mut files);
    files.sort();
    assert!(!files.is_empty(), "the generator produced no files");

    for rel in &files {
        let fresh = fs::read(generated.join(rel)).expect("read generated file");
        let committed_path = repo_path(&format!("fixtures/{}", rel.display()));
        let committed = fs::read(&committed_path).unwrap_or_else(|e| {
            panic!(
                "the generator publishes fixtures/{} but it is not committed: {e}\n\
                 re-run `cargo run -p elang-conformance --bin gen_conformance_corpus`",
                rel.display()
            )
        });
        assert_eq!(
            fresh,
            committed,
            "published artefact fixtures/{} drifted from a fresh generation\n\
             re-run `cargo run -p elang-conformance --bin gen_conformance_corpus`",
            rel.display()
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn every_published_fixture_records_the_modes_the_reader_would_apply() {
    // `modes` is not a free-form annotation. It tells an independent
    // implementation which reader modes must observe the fixture's
    // outcome, so it has to match the reader's own pipeline:
    //
    // * A rejection fires at one stage, and only the modes whose
    //   pipeline ceiling reaches that stage observe it. A digest
    //   mismatch surfaces at stage 11, so `inspect` (ceiling 7)
    //   *accepts* that package and must not be listed.
    // * Acceptance is cumulative, so a passing record lists every mode
    //   down to the asserted level — always a prefix of the canonical
    //   mode order.
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path(CORPUS_INDEX)).expect("read index"))
            .expect("index is JSON");
    let fixtures = index
        .get("fixtures")
        .and_then(serde_json::Value::as_array)
        .expect("fixtures array");

    let mut rejections = 0usize;
    for fixture in fixtures {
        // Vector and meta entries are not published fixtures and carry
        // no `modes`.
        let Some(modes_value) = fixture.get("modes") else {
            continue;
        };
        let modes: Vec<&str> = modes_value
            .as_array()
            .expect("modes is an array")
            .iter()
            .map(|m| m.as_str().expect("mode is a string"))
            .collect();
        let id = fixture
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("entry has an id");
        let outcome = fixture
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .expect("entry has an outcome");

        if outcome == "fail" {
            let stage = fixture
                .get("stage")
                .and_then(serde_json::Value::as_u64)
                .expect("a rejection records its stage");
            let stage = u8::try_from(stage).expect("stage fits u8");
            let expected: Vec<&str> = MODES
                .iter()
                .copied()
                .filter(|m| ceiling_for_mode(m) >= stage)
                .collect();
            assert_eq!(
                modes, expected,
                "rejection `{id}` fires at stage {stage}, so exactly the modes \
                 whose pipeline ceiling reaches stage {stage} must be listed"
            );
            rejections = rejections.saturating_add(1);
        } else {
            assert!(
                MODES.starts_with(&modes[..]),
                "acceptance `{id}` lists modes {modes:?}, which is not a prefix of {MODES:?}"
            );
        }
    }
    assert!(rejections > 0, "the index contains no rejection fixtures");
}

#[test]
fn every_published_fixture_is_on_disk() {
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path(CORPUS_INDEX)).expect("read index"))
            .expect("index is JSON");
    let fixtures = index
        .get("fixtures")
        .and_then(serde_json::Value::as_array)
        .expect("fixtures array");
    for fixture in fixtures {
        if !fixture
            .get("published")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let input = fixture
            .get("input")
            .and_then(serde_json::Value::as_str)
            .expect("published entry has input");
        let expected_sha = fixture
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .expect("published entry has sha256");
        assert!(
            repo_path(input).exists(),
            "published fixture exists in the index but not on disk: {input}; re-run gen_conformance_corpus"
        );
        let bytes = fs::read(repo_path(input)).expect("read published input");
        let actual = hex(&sha256(&bytes));
        assert_eq!(
            actual, expected_sha,
            "published sha256 mismatch for {input}: re-run gen_conformance_corpus"
        );
    }
}

#[test]
fn every_published_archive_matches_its_source() {
    // Walk every archive-level fixture in the in-crate sources and
    // check the on-disk bytes match. This is the strongest drift
    // guarantee: the corpus cannot lie about what it tests.
    let sources = published_sources();
    for fixture in elang_conformance::negatives::fixtures() {
        let Some(corpus_id) = sources.get(fixture.id) else {
            continue;
        };
        assert_published_matches_archive(&fixture, corpus_id);
    }
    for fixture in elang_conformance::zstd_negatives::fixtures() {
        let Some(corpus_id) = sources.get(fixture.id) else {
            continue;
        };
        assert_published_matches_archive(&fixture, corpus_id);
    }
    for fixture in elang_conformance::inspect_matrix::fixtures() {
        let Some(corpus_id) = sources.get(fixture.id) else {
            continue;
        };
        assert_published_matches_archive(&fixture, corpus_id);
    }
    for pkg in elang_conformance::golden_packages::all().expect("golden packages build") {
        let Some(corpus_id) = sources.get(pkg.id) else {
            continue;
        };
        assert_published_matches_golden(&pkg, corpus_id);
    }
}

#[test]
fn every_published_document_matches_its_source() {
    let sources = published_sources();
    for fixture in elang_conformance::epf_eld_negatives::all_fixtures() {
        let Some(corpus_id) = sources.get(fixture.id) else {
            continue;
        };
        assert_published_matches_document(&fixture, corpus_id);
    }
    for fixture in elang_conformance::parser_budgets::fixtures() {
        let Some(corpus_id) = sources.get(fixture.id) else {
            continue;
        };
        assert_published_matches_document(&fixture, corpus_id);
    }
}

fn assert_published_matches_archive(fixture: &Fixture, corpus_id: &str) {
    let corpus_path = format!("{FIXTURE_ROOT}/{corpus_id}");
    let input = format!("{corpus_path}/archive.elang");
    let published = fs::read(repo_path(&input))
        .unwrap_or_else(|e| panic!("missing published archive for `{}`: {e}", fixture.id));
    assert_eq!(
        published, fixture.archive,
        "published archive bytes for `{}` drifted from the in-code source",
        fixture.id
    );
    assert_eq!(
        hex(&sha256(&fixture.archive)),
        sidecar_sha256(&input),
        "sidecar sha256 for `{}` does not match the in-code source",
        fixture.id
    );
    assert!(
        repo_path(&format!("{corpus_path}/expected.json")).exists(),
        "missing expected.json for `{}`",
        fixture.id
    );
}

fn assert_published_matches_golden(
    pkg: &elang_conformance::golden_packages::GoldenPackage,
    corpus_id: &str,
) {
    let corpus_path = format!("{FIXTURE_ROOT}/{corpus_id}");
    let input = format!("{corpus_path}/archive.elang");
    let published = fs::read(repo_path(&input))
        .unwrap_or_else(|e| panic!("missing published golden `{}`: {e}", pkg.id));
    assert_eq!(published, pkg.archive, "golden archive drifted");
    assert_eq!(hex(&sha256(&pkg.archive)), sidecar_sha256(&input));
    for (segment, sha) in [
        ("header.bin", &pkg.header_sha256),
        ("index.bin", &pkg.index_sha256),
        ("footer.bin", &pkg.footer_sha256),
    ] {
        assert_eq!(sha, &sidecar_sha256(&format!("{corpus_path}/{segment}")));
    }
}

fn assert_published_matches_document(fixture: &DocumentFixture, corpus_id: &str) {
    let corpus_path = format!("{FIXTURE_ROOT}/{corpus_id}");
    let ext = match fixture.schema {
        Schema::Epf => "epf",
        Schema::Eld => "eld",
    };
    let input = format!("{corpus_path}/input.{ext}");
    let published = fs::read(repo_path(&input))
        .unwrap_or_else(|e| panic!("missing published document for `{}`: {e}", fixture.id));
    assert_eq!(
        published, fixture.source,
        "published document bytes for `{}` drifted",
        fixture.id
    );
    assert_eq!(
        hex(&sha256(&fixture.source)),
        sidecar_sha256(&input),
        "sidecar sha256 for `{}` does not match",
        fixture.id
    );
}

/// Read the `<file>.sha256` sidecar and return the lowercase hex digest.
fn sidecar_sha256(path: &str) -> String {
    let body = fs::read_to_string(repo_path(&format!("{path}.sha256"))).expect("read sidecar");
    body.split_whitespace()
        .next()
        .expect("sidecar has a digest")
        .to_string()
}

/// The published corpus id for every published catalog entry.
///
/// The mapping comes from the index's `corpusId` field rather than being
/// re-derived here: the generator owns the layout, and a second
/// implementation of that layout in the test would be a second source of
/// truth that could drift from the first.
fn published_sources() -> BTreeMap<String, String> {
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path(CORPUS_INDEX)).expect("read index"))
            .expect("index is JSON");
    let mut out = BTreeMap::new();
    let fixtures = index
        .get("fixtures")
        .and_then(serde_json::Value::as_array)
        .expect("fixtures array");
    for fixture in fixtures {
        if !fixture
            .get("published")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let catalog_id = fixture
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("catalog id");
        let corpus_id = fixture
            .get("corpusId")
            .and_then(serde_json::Value::as_str)
            .expect("published entry has corpusId");
        out.insert(catalog_id.to_string(), corpus_id.to_string());
    }
    out
}

fn is_meta(id: &str) -> bool {
    id.starts_with("roundtrip/")
        || id.starts_with("ci/")
        || id.starts_with("docs/")
        || id.starts_with("reader/")
        || id.starts_with("cli/")
        || id.starts_with("workspace/")
        || id.starts_with("traceability/")
        || id.starts_with("governance/")
}

/// The reader modes, in canonical order.
const MODES: [&str; 3] = ["inspect", "verify", "validate"];

/// The highest reader stage a mode's pipeline reaches.
///
/// Mirrors `elang_conformance::corpus::Level::ceiling_stage`.
fn ceiling_for_mode(mode: &str) -> u8 {
    match mode {
        "inspect" => 7,
        "verify" => 11,
        "validate" => 12,
        other => panic!("unknown reader mode {other}"),
    }
}

/// A scratch directory under `target/`, unique to this test run.
///
/// `target/` is git-ignored and always writable, and keeping the
/// scratch tree inside the workspace avoids the platform temp
/// directory.
fn scratch_dir(name: &str) -> PathBuf {
    let mut p = release_root();
    p.push("target");
    p.push(format!("{name}-{}", std::process::id()));
    p
}

/// Collect every file under `dir`, as paths relative to `root`.
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if entry.file_type().expect("file type").is_dir() {
            collect_files(root, &path, out);
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("path lives under root")
                .to_path_buf();
            out.push(rel);
        }
    }
}

#[allow(dead_code)]
fn expected_outcome(fixture: &Fixture) -> &'static str {
    match fixture.expectation {
        Expectation::Accept(_) => "pass",
        Expectation::Reject { .. } => "fail",
    }
}

#[allow(dead_code)]
fn document_outcome(fixture: &DocumentFixture) -> &'static str {
    match fixture.expectation {
        DocumentExpectation::Accept => "pass",
        DocumentExpectation::Reject { .. } => "fail",
    }
}
