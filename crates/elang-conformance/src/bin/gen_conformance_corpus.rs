//! `gen_conformance_corpus` — Phase 16 conformance corpus publisher (T1612).
//!
//! Publishes the ELANG v0.1.0 conformance corpus
//! (`elang-canonical-zstd-v1-conformance`) as **files on disk** so that
//! an independent implementation can execute it without reading this
//! repository's Rust source.
//!
//! # What was already published
//!
//! The OA-14 canonical ELD goldens (`fixtures/valid/canonical/<case>/`)
//! and the container reference archive (`fixtures/valid/container/`) were
//! already on disk. What was **not** published was the negative corpus:
//! the 49 malformed packages and documents lived only as Rust `Vec`s
//! inside `elang-conformance`, which means an independent implementer
//! had no way to run them.
//!
//! # What this generator writes
//!
//! | Path | Contents |
//! |---|---|
//! | `fixtures/invalid/<family>/<name>/input.<ext>` | The malformed input |
//! | `fixtures/invalid/<family>/<name>/expected.json` | The expected-result record |
//! | `fixtures/valid/<family>/<name>/input.<ext>` | The accepted input (level-matrix + budget-at-limit cases) |
//! | `fixtures/valid/<family>/<name>/expected.json` | The expected-result record |
//! | `fixtures/corpus_index.json` | Machine-readable index of the **whole** corpus |
//! | `fixtures/expected-result.schema.json` | JSON Schema for `expected.json` |
//!
//! # Why the index carries more than the published files
//!
//! The catalog contains four kinds of entry:
//!
//! * **archive** / **document** — real inputs. These are published.
//! * **vector** — byte-equality checks over values the production code
//!   computes (condition sorting keys, NFC forms, Zstd frame digests).
//!   They have no portable on-disk representation: an independent
//!   implementation has to *produce* the value, so shipping the bytes
//!   would make the test vacuous. They stay in-crate and the index says
//!   so explicitly.
//! * **meta** — assertions about the repository itself (the CI matrix
//!   runs on three OSes, `docs/conformance.md` exists). These are
//!   executed by the test suite, not by an external runner.
//!
//! The index records all four so a reader can tell the difference
//! between "not published" and "does not exist".
//!
//! # Idempotence
//!
//! Every write goes through [`write_if_changed`], so re-running the
//! generator on an unchanged tree rewrites nothing and the working tree
//! stays clean. That is what makes the corpus safe to verify in CI.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use elang_conformance::binary_vectors::{hex, sha256};
use elang_conformance::catalog::{self, CatalogEntry, FixtureExpectation};
use elang_conformance::corpus::{
    DocumentExpectation, DocumentFixture, Expectation, Fixture, Level,
};

const FIXTURE_ROOT: &str = "fixtures";
const INDEX_PATH: &str = "fixtures/corpus_index.json";
const SCHEMA_PATH: &str = "fixtures/expected-result.schema.json";

/// The corpus name from steering §11.
const CORPUS_NAME: &str = "elang-canonical-zstd-v1-conformance";
/// The corpus format version.
const CORPUS_VERSION: &str = "0.1.0";

fn main() -> ExitCode {
    let mut published: Vec<PublishedFixture> = Vec::new();

    // --- Archive-level fixtures (malformed and accepted packages) ------
    for fixture in elang_conformance::negatives::fixtures() {
        published.push(publish_archive(&fixture));
    }
    for fixture in elang_conformance::zstd_negatives::fixtures() {
        published.push(publish_archive(&fixture));
    }
    for fixture in elang_conformance::inspect_matrix::fixtures() {
        published.push(publish_archive(&fixture));
    }

    // --- Golden packages (positive archive-level acceptance) -----------
    for pkg in elang_conformance::golden_packages::all().expect("golden packages build") {
        published.push(publish_golden_package(&pkg));
    }

    // --- Document-level fixtures (EPF / ELD sources) --------------------
    for fixture in elang_conformance::epf_eld_negatives::all_fixtures() {
        published.push(publish_document(&fixture));
    }
    for fixture in elang_conformance::parser_budgets::fixtures() {
        published.push(publish_document(&fixture));
    }

    published.sort_by(|a, b| a.id.cmp(&b.id));

    // Duplicate published ids would make a failure report ambiguous.
    let mut seen = std::collections::BTreeSet::new();
    for p in &published {
        assert!(
            seen.insert(p.id.clone()),
            "duplicate published fixture id: {}",
            p.id
        );
    }

    // --- The whole-catalog index ----------------------------------------
    let catalog_entries = catalog::catalog();
    let index = render_index(&catalog_entries, &published);

    let json = serde_json::to_string_pretty(&index).expect("index serializes");
    let mut json = json;
    json.push('\n');
    if let Err(e) = write_if_changed(INDEX_PATH, &json) {
        eprintln!("failed to write {INDEX_PATH}: {e}");
        return ExitCode::FAILURE;
    }

    let schema = EXPECTED_RESULT_SCHEMA;
    if let Err(e) = write_if_changed(SCHEMA_PATH, schema) {
        eprintln!("failed to write {SCHEMA_PATH}: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!(
        "published {} fixtures; wrote {INDEX_PATH} and {SCHEMA_PATH}",
        published.len()
    );
    ExitCode::SUCCESS
}

/// One published fixture, as recorded in the corpus index.
struct PublishedFixture {
    /// The corpus id (`valid/<family>/<name>` or `invalid/<family>/<name>`).
    id: String,
    /// The in-crate fixture id this was published from.
    source_id: String,
    /// `archive`, `document` or `vector`.
    kind: &'static str,
    /// Repository-relative path of the published input.
    input: String,
    /// SHA-256 of the published input bytes.
    sha256: String,
    /// The reader modes the expectation holds for.
    modes: Vec<&'static str>,
    /// The stage at which rejection must occur (`fail` only).
    stage: Option<u8>,
}

/// The modes a `pass` expectation at a given level holds for.
///
/// Acceptance is cumulative: a fixture asserted at `Validate` must also
/// be accepted at `Verify` and at `Inspect`, so the record lists every
/// mode down to the asserted level.
fn modes_for_pass(level: Level) -> Vec<&'static str> {
    match level {
        Level::Inspect => vec!["inspect"],
        Level::Verify => vec!["inspect", "verify"],
        Level::Validate => vec!["inspect", "verify", "validate"],
    }
}

/// The modes a `fail` expectation at a given rejection stage holds for.
///
/// Rejection is **not** cumulative. A fixture rejected at stage `S` is
/// rejected only by the modes whose pipeline ceiling actually reaches
/// `S` — see `Level::ceiling_stage()`. A digest mismatch, for example,
/// surfaces at stage 11, so `inspect` (ceiling 7) *accepts* that package
/// and must not be listed. The record therefore lists exactly the modes
/// that observe the rejection, in canonical order.
fn modes_for_fail(stage: u8) -> Vec<&'static str> {
    let mut modes = Vec::new();
    if Level::Inspect.ceiling_stage() >= stage {
        modes.push("inspect");
    }
    if Level::Verify.ceiling_stage() >= stage {
        modes.push("verify");
    }
    if Level::Validate.ceiling_stage() >= stage {
        modes.push("validate");
    }
    modes
}

/// The modes a document-level expectation holds for.
///
/// A document fixture is a source schema, not a package: it is parsed
/// directly by `elang_epf::parse` / `elang_eld::parse` and never travels
/// through the package reader's staged pipeline. The corpus records it at
/// the `inspect` level — the lowest reader level, and the first one at
/// which a source document is consumed.
fn modes_for_document() -> Vec<&'static str> {
    vec!["inspect"]
}

/// Split an in-crate fixture id (`family/name`) into its parts.
fn split_id(id: &str) -> (String, String) {
    match id.split_once('/') {
        Some((family, name)) => (family.to_string(), name.to_string()),
        None => ("misc".to_string(), id.to_string()),
    }
}

/// Publish an archive-level fixture.
fn publish_archive(fixture: &Fixture) -> PublishedFixture {
    let (family, name) = split_id(fixture.id);
    let (outcome, level, error_code) = match fixture.expectation {
        Expectation::Accept(level) => ("pass", level, None),
        Expectation::Reject { level, code, .. } => ("fail", level, Some(code.to_string())),
    };
    let outcome_dir_name = outcome_dir(outcome);
    let dir = format!("{FIXTURE_ROOT}/{outcome_dir_name}/{family}/{name}");
    let input_rel = format!("{dir}/archive.elang");
    let bytes = &fixture.archive;
    write_bytes(&input_rel, bytes).expect("write archive input");
    write_sidecar(&input_rel, bytes).expect("write sidecar");

    let stage = error_code.as_ref().map(|c| stage_for_code(c));
    // `modes` follows different rules for the two outcomes: acceptance
    // is cumulative (a Validate-accept also holds for Verify/Inspect),
    // while rejection holds only for the modes whose pipeline ceiling
    // actually reaches the rejection stage.
    let modes = match fixture.expectation {
        Expectation::Accept(_) => modes_for_pass(level),
        Expectation::Reject { .. } => {
            modes_for_fail(stage.expect("reject expectation carries a stage"))
        }
    };
    // The record's `input` is relative to the fixture directory, so the
    // record is self-contained: it resolves against the directory that
    // holds it. The corpus *index* keeps repository-root-relative paths
    // because it is one manifest for the whole tree.
    let record = expected_result_json(&ExpectedResult {
        fixture: &format!("{outcome_dir_name}/{family}/{name}"),
        description: fixture.description,
        requirements: fixture.requirements,
        input: basename(&input_rel),
        modes: &modes,
        outcome,
        stage,
        error_code: error_code.as_deref(),
        golden: &[],
    });
    write_if_changed(&format!("{dir}/expected.json"), &record).expect("write expected.json");

    PublishedFixture {
        id: format!("{outcome_dir_name}/{family}/{name}"),
        source_id: fixture.id.to_string(),
        kind: "archive",
        input: input_rel,
        sha256: hex(&sha256(bytes)),
        modes,
        stage,
    }
}

/// Publish a document-level fixture.
fn publish_document(fixture: &DocumentFixture) -> PublishedFixture {
    let (family, name) = split_id(fixture.id);
    let (outcome, error_code) = match fixture.expectation {
        DocumentExpectation::Accept => ("pass", None),
        DocumentExpectation::Reject { code, .. } => ("fail", Some(code.to_string())),
    };
    let outcome_dir_name = outcome_dir(outcome);
    let dir = format!("{FIXTURE_ROOT}/{outcome_dir_name}/{family}/{name}");
    let ext = match fixture.schema {
        elang_conformance::corpus::Schema::Epf => "epf",
        elang_conformance::corpus::Schema::Eld => "eld",
    };
    let input_rel = format!("{dir}/input.{ext}");
    let bytes = &fixture.source;
    write_bytes(&input_rel, bytes).expect("write document input");
    write_sidecar(&input_rel, bytes).expect("write sidecar");

    let stage = error_code.as_ref().map(|c| stage_for_code(c));
    // The rejection rule applies to documents as well as packages: an
    // EPF parse error at stage 2 is observed by every reader mode (the
    // index is read at all three), but an ELD semantic error at stage 12
    // is only observed by `validate`, which is the only mode that parses
    // the phonology document.
    let modes = match fixture.expectation {
        DocumentExpectation::Accept => modes_for_document(),
        DocumentExpectation::Reject { .. } => {
            modes_for_fail(stage.expect("reject expectation carries a stage"))
        }
    };
    let record = expected_result_json(&ExpectedResult {
        fixture: &format!("{outcome_dir_name}/{family}/{name}"),
        description: fixture.description,
        requirements: fixture.requirements,
        input: basename(&input_rel),
        modes: &modes,
        outcome,
        stage,
        error_code: error_code.as_deref(),
        golden: &[],
    });
    write_if_changed(&format!("{dir}/expected.json"), &record).expect("write expected.json");

    PublishedFixture {
        id: format!("{outcome_dir_name}/{family}/{name}"),
        source_id: fixture.id.to_string(),
        kind: "document",
        input: input_rel,
        sha256: hex(&sha256(bytes)),
        modes,
        stage,
    }
}

/// Publish the positive `golden_packages` archive and its segment
/// sidecars.
///
/// The golden packages carry a fixed archive + a fixed
/// header/index/footer split (the only on-disk artefact today is
/// `fixtures/valid/container/minimal-archive/`, which is exactly the
/// minimal package). Publishing them in their canonical
/// `fixtures/valid/<id>/` location gives independent implementations a
/// single, predictable place to fetch every positive package.
fn publish_golden_package(
    pkg: &elang_conformance::golden_packages::GoldenPackage,
) -> PublishedFixture {
    // The catalog id is `valid/minimal` / `valid/full`; the corpus path
    // reuses the same id (no `family/` prefix because the catalog id
    // already starts with `valid/`).
    let corpus_id = pkg.id.to_string();
    let dir = format!("{FIXTURE_ROOT}/{corpus_id}");
    let input_rel = format!("{dir}/archive.elang");
    write_bytes(&input_rel, &pkg.archive).expect("write golden archive");
    write_bytes(&format!("{dir}/header.bin"), &pkg.header).expect("write header");
    write_bytes(&format!("{dir}/index.bin"), &pkg.index).expect("write index");
    write_bytes(&format!("{dir}/footer.bin"), &pkg.footer).expect("write footer");
    write_sidecar(&input_rel, &pkg.archive).expect("archive sidecar");
    write_if_changed(
        &format!("{dir}/header.bin.sha256"),
        &format!(
            "{}  header.bin
",
            pkg.header_sha256
        ),
    )
    .expect("header sidecar");
    write_if_changed(
        &format!("{dir}/index.bin.sha256"),
        &format!(
            "{}  index.bin
",
            pkg.index_sha256
        ),
    )
    .expect("index sidecar");
    write_if_changed(
        &format!("{dir}/footer.bin.sha256"),
        &format!(
            "{}  footer.bin
",
            pkg.footer_sha256
        ),
    )
    .expect("footer sidecar");

    let modes = modes_for_pass(Level::Inspect);
    let golden: Vec<(String, String)> = vec![
        ("archive.elang".into(), pkg.archive_sha256.clone()),
        ("header.bin".into(), pkg.header_sha256.clone()),
        ("index.bin".into(), pkg.index_sha256.clone()),
        ("footer.bin".into(), pkg.footer_sha256.clone()),
    ];
    let record = expected_result_json(&ExpectedResult {
        fixture: &corpus_id,
        description: pkg.description,
        requirements: pkg.requirements,
        input: basename(&input_rel),
        modes: &modes,
        outcome: "pass",
        stage: None,
        error_code: None,
        golden: &golden,
    });
    write_if_changed(&format!("{dir}/expected.json"), &record).expect("golden expected.json");

    PublishedFixture {
        id: corpus_id,
        source_id: pkg.id.to_string(),
        kind: "archive",
        input: input_rel,
        sha256: pkg.archive_sha256.clone(),
        modes,
        stage: None,
    }
}

fn outcome_dir(outcome: &str) -> &'static str {
    if outcome == "pass" {
        "valid"
    } else {
        "invalid"
    }
}

/// The reader stage at which a registered error code fires.
///
/// The registry publishes the stages each code can fire at. A code that
/// the registry does not know about is a corpus bug, so this panics
/// rather than publishing a record with a missing stage.
fn stage_for_code(code: &str) -> u8 {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../../../../specs/registry/error-codes.json"))
            .expect("error-code registry parses");
    let codes = registry
        .get("codes")
        .and_then(serde_json::Value::as_array)
        .expect("registry has a codes array");
    for entry in codes {
        if entry.get("code").and_then(serde_json::Value::as_str) == Some(code) {
            let stages = entry
                .get("stages")
                .and_then(serde_json::Value::as_array)
                .expect("registry entry has stages");
            let first = stages
                .first()
                .and_then(serde_json::Value::as_u64)
                .expect("registry entry has at least one stage");
            return u8::try_from(first).expect("stage fits u8");
        }
    }
    panic!("error code {code} is not in the registry");
}

/// The fields of one `expected.json` record.
///
/// Grouped into a struct rather than nine positional parameters: the
/// record has enough fields that a positional call site is unreadable
/// and easy to mis-order.
struct ExpectedResult<'a> {
    fixture: &'a str,
    description: &'a str,
    requirements: &'a [&'a str],
    input: &'a str,
    modes: &'a [&'a str],
    outcome: &'a str,
    stage: Option<u8>,
    error_code: Option<&'a str>,
    golden: &'a [(String, String)],
}

/// Render an `expected.json` record.
fn expected_result_json(r: &ExpectedResult<'_>) -> String {
    let mut value = serde_json::Map::new();
    value.insert(
        "fixture".into(),
        serde_json::Value::String(r.fixture.into()),
    );
    value.insert(
        "description".into(),
        serde_json::Value::String(r.description.into()),
    );
    value.insert(
        "requirements".into(),
        serde_json::Value::Array(
            r.requirements
                .iter()
                .map(|r| serde_json::Value::String((*r).into()))
                .collect(),
        ),
    );
    value.insert("input".into(), serde_json::Value::String(r.input.into()));
    value.insert(
        "modes".into(),
        serde_json::Value::Array(
            r.modes
                .iter()
                .map(|m| serde_json::Value::String((*m).into()))
                .collect(),
        ),
    );
    value.insert(
        "outcome".into(),
        serde_json::Value::String(r.outcome.into()),
    );
    if let Some(s) = r.stage {
        value.insert("stage".into(), serde_json::Value::from(s));
    }
    if let Some(c) = r.error_code {
        value.insert("errorCode".into(), serde_json::Value::String(c.into()));
    }
    if !r.golden.is_empty() {
        value.insert(
            "golden".into(),
            serde_json::Value::Array(
                r.golden
                    .iter()
                    .map(|(path, digest)| {
                        let mut g = serde_json::Map::new();
                        g.insert("path".into(), serde_json::Value::String(path.clone()));
                        g.insert("sha256".into(), serde_json::Value::String(digest.clone()));
                        serde_json::Value::Object(g)
                    })
                    .collect(),
            ),
        );
    }
    let mut out =
        serde_json::to_string_pretty(&serde_json::Value::Object(value)).expect("record serializes");
    out.push('\n');
    out
}

/// Render the whole-corpus index.
fn render_index(
    catalog_entries: &[CatalogEntry],
    published: &[PublishedFixture],
) -> serde_json::Value {
    let by_source: BTreeMap<&str, &PublishedFixture> = published
        .iter()
        .map(|p| (p.source_id.as_str(), p))
        .collect();

    let mut entries: Vec<serde_json::Value> = Vec::new();
    let mut published_count = 0usize;
    let mut vector_count = 0usize;
    let mut meta_count = 0usize;

    for entry in catalog_entries {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::String(entry.id.clone()));
        obj.insert(
            "description".into(),
            serde_json::Value::String(entry.description.to_string()),
        );
        obj.insert(
            "requirements".into(),
            serde_json::Value::Array(
                entry
                    .requirements
                    .iter()
                    .map(|r| serde_json::Value::String((*r).into()))
                    .collect(),
            ),
        );
        let outcome = match entry.expectation {
            FixtureExpectation::Accept => "pass",
            FixtureExpectation::Reject { .. } => "fail",
        };
        obj.insert("outcome".into(), serde_json::Value::String(outcome.into()));
        obj.insert(
            "errorCode".into(),
            match entry.expectation {
                FixtureExpectation::Accept => serde_json::Value::Null,
                FixtureExpectation::Reject { code } => serde_json::Value::String(code.to_string()),
            },
        );

        match by_source.get(entry.id.as_str()) {
            Some(p) => {
                published_count = published_count.saturating_add(1);
                obj.insert("kind".into(), serde_json::Value::String(p.kind.into()));
                obj.insert("published".into(), serde_json::Value::Bool(true));
                obj.insert("corpusId".into(), serde_json::Value::String(p.id.clone()));
                obj.insert("input".into(), serde_json::Value::String(p.input.clone()));
                obj.insert("sha256".into(), serde_json::Value::String(p.sha256.clone()));
                obj.insert(
                    "modes".into(),
                    serde_json::Value::Array(
                        p.modes
                            .iter()
                            .map(|m| serde_json::Value::String((*m).into()))
                            .collect(),
                    ),
                );
                obj.insert(
                    "stage".into(),
                    match p.stage {
                        Some(s) => serde_json::Value::from(s),
                        None => serde_json::Value::Null,
                    },
                );
            }
            None => {
                let kind = if entry.id.starts_with("roundtrip/")
                    || entry.id.starts_with("ci/")
                    || entry.id.starts_with("docs/")
                    || entry.id.starts_with("reader/")
                    || entry.id.starts_with("cli/")
                    || entry.id.starts_with("workspace/")
                    || entry.id.starts_with("traceability/")
                    || entry.id.starts_with("governance/")
                {
                    meta_count = meta_count.saturating_add(1);
                    "meta"
                } else {
                    vector_count = vector_count.saturating_add(1);
                    "vector"
                };
                obj.insert("kind".into(), serde_json::Value::String(kind.into()));
                obj.insert("published".into(), serde_json::Value::Bool(false));
                obj.insert(
                    "note".into(),
                    serde_json::Value::String(match kind {
                        "meta" => "Asserted by the repository's own test suite, not by an external corpus runner.",
                        _ => "Byte-equality check over a value the implementation must produce; not publishable as a portable input.",
                    }.into()),
                );
            }
        }
        entries.push(serde_json::Value::Object(obj));
    }

    let mut root = serde_json::Map::new();
    root.insert(
        "corpus".into(),
        serde_json::Value::String(CORPUS_NAME.into()),
    );
    root.insert(
        "version".into(),
        serde_json::Value::String(CORPUS_VERSION.into()),
    );
    root.insert(
        "generatedBy".into(),
        serde_json::Value::String("gen_conformance_corpus".into()),
    );
    root.insert(
        "expectedResultSchema".into(),
        serde_json::Value::String(SCHEMA_PATH.into()),
    );
    root.insert(
        "conventions".into(),
        serde_json::Value::String(
            "fixtures/README.md documents the layout, the expected-result fields and the provenance rules. Paths in this index are repository-root-relative; paths inside an individual expected.json record are relative to the fixture directory that contains it."
                .into(),
        ),
    );
    root.insert(
        "counts".into(),
        serde_json::json!({
            "total": catalog_entries.len(),
            "published": published_count,
            "vector": vector_count,
            "meta": meta_count,
        }),
    );
    root.insert("fixtures".into(), serde_json::Value::Array(entries));
    serde_json::Value::Object(root)
}

fn write_bytes(path: &str, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    // Skip the write when the bytes on disk are already identical. A
    // byte-identical rewrite still bumps the file's mtime, which makes
    // the reproducible-build CI gate see spurious drift between two
    // otherwise-identical clean builds.
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
    }
    fs::write(path, bytes)
}

/// Write the `.sha256` sidecar next to a published input.
fn write_sidecar(input_path: &str, bytes: &[u8]) -> std::io::Result<()> {
    let line = format!("{}  {}\n", hex(&sha256(bytes)), basename(input_path));
    write_if_changed(&format!("{input_path}.sha256"), &line)
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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

/// The JSON Schema for an `expected.json` record.
///
/// Published as a standalone file so an independent implementation can
/// validate the corpus records it reads without needing this repository's
/// Rust model. `fixtures/README.md` records why it was previously
/// un-publishable (a workspace write restriction at the time); that
/// restriction no longer applies.
const EXPECTED_RESULT_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://github.com/RockyDevWorks/elang-rs/schemas/expected-result.schema.json",
  "title": "ELANG v0.1.0 conformance expected-result record",
  "description": "One record per conformance fixture. Describes the input, the reader modes the expectation holds for, and the expected outcome.",
  "type": "object",
  "required": ["fixture", "input", "modes", "outcome"],
  "additionalProperties": false,
  "properties": {
    "fixture": {
      "type": "string",
      "description": "Stable fixture id.",
      "pattern": "^(valid|invalid)/[a-z][a-z0-9-]*(/[a-z][a-z0-9-]*)*$"
    },
    "description": {
      "type": "string",
      "description": "One-line human description of the behaviour under test."
    },
    "requirements": {
      "type": "array",
      "description": "Normative requirement ids the fixture protects.",
      "items": { "type": "string", "pattern": "^REQ-[A-Z0-9-]+$" }
    },
    "input": {
      "type": "string",
      "description": "Path to the fixture input, relative to the directory that contains this record.",
      "minLength": 1
    },
    "modes": {
      "type": "array",
      "description": "Reader modes in which the stated outcome MUST hold. Inspect covers reader stages 1-7, Verify stages 1-11, Validate stages 1-12. For `outcome: pass`, acceptance is cumulative, so the record lists every mode down to the asserted level. For `outcome: fail`, the record lists only the modes whose stage ceiling reaches `stage`; a mode that stops before `stage` accepts the input.",
      "minItems": 1,
      "uniqueItems": true,
      "items": { "enum": ["inspect", "verify", "validate"] }
    },
    "outcome": {
      "enum": ["pass", "fail"],
      "description": "`pass` — the input must be accepted. `fail` — the input must be rejected."
    },
    "stage": {
      "type": "integer",
      "description": "Reader stage (1-12) at which rejection MUST occur. Required when outcome is `fail`.",
      "minimum": 1,
      "maximum": 12
    },
    "errorCode": {
      "type": "string",
      "description": "Registered error code from the OA-10 error-code registry. Required when outcome is `fail`.",
      "pattern": "^[A-Z][A-Z0-9_]*$"
    },
    "golden": {
      "type": "array",
      "description": "Byte-exact golden outputs, when the fixture pins them.",
      "items": {
        "type": "object",
        "required": ["path", "sha256"],
        "additionalProperties": false,
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the byte-exact golden output, relative to the directory that contains this record.",
            "minLength": 1
          },
          "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
        }
      }
    }
  },
  "allOf": [
    {
      "if": { "properties": { "outcome": { "const": "fail" } }, "required": ["outcome"] },
      "then": { "required": ["stage", "errorCode"] }
    }
  ]
}
"##;
