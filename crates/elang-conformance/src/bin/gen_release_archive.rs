//! `gen_release_archive` — Phase 16 release archival metadata (T1619).
//!
//! Records the exact bytes a v0.1.0 release depends on:
//!
//! * the **resolved dependency set** (`Cargo.lock` and its digest),
//! * the **pinned Unicode / IPA reference data** and its digests,
//! * the **conformance fixture hashes** for every published fixture,
//! * the **normative manifest** and its pinned checksum.
//!
//! # What this is and is not
//!
//! This archive records **integrity**: it pins the SHA-256 of every
//! input the release was built from, so a consumer can prove the bytes
//! they hold are the bytes the release used. It does **not** establish
//! publisher authenticity — that requires a signed tag or an
//! out-of-band trust mechanism, and is explicitly out of scope for the
//! v0.1.0 release. See `docs/release/v0.1.0/README.md`.
//!
//! # Where the outputs go
//!
//! | Path | Contents |
//! |---|---|
//! | `Cargo.lock.sha256` | Digest of the resolved dependency lockfile |
//! | `docs/release/v0.1.0/dependencies.txt` | Every resolved dependency, name + version |
//! | `docs/release/v0.1.0/repertoire.sha256` | Digests of the pinned Unicode / IPA data |
//! | `docs/release/v0.1.0/conformance-hashes.txt` | Digests of every published conformance fixture |
//! | `docs/release/v0.1.0/ARCHIVE.md` | Human-readable index of the above |
//!
//! # Idempotence
//!
//! Every write goes through `write_if_changed`, so re-running the
//! generator on an unchanged tree rewrites nothing.

#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use elang_conformance::binary_vectors::{hex, sha256};

const ARCHIVE_DIR: &str = "docs/release/v0.1.0";
const LOCKFILE: &str = "Cargo.lock";
const LOCKFILE_SHA: &str = "Cargo.lock.sha256";

/// The pinned Unicode / IPA reference data the release depends on.
///
/// These are the *normative inputs* — the repertoire schema and data,
/// and the raw upstream sources they were generated from. The registry
/// files that publish derived vectors are covered by the normative
/// manifest and are listed separately below.
const REPERTOIRE_FILES: &[&str] = &[
    "specs/schema/ipa-v1.json",
    "specs/schema/elang-ipa-repertoire-v1.schema.json",
    "specs/sources/ucd-18.0.0/UnicodeData.txt",
    "specs/sources/ucd-18.0.0/CompositionExclusions.txt",
    "specs/sources/ucd-18.0.0/DerivedGeneralCategory.txt",
    "specs/sources/ucd-18.0.0/DerivedNormalizationProps.txt",
    "specs/sources/ucd-18.0.0/NormalizationTest.txt",
    "specs/sources/ucd-18.0.0/SOURCES.md",
    "specs/sources/unitipa-ipa-chart-2020.pdf",
    "specs/sources/SOURCES.md",
];

/// The registry files that publish derived vectors and registries.
const REGISTRY_FILES: &[&str] = &[
    "specs/registry/normative-manifest.json",
    "specs/registry/binary-reference-vectors.json",
    "specs/registry/eld-canonical-vectors.json",
    "specs/registry/error-codes.json",
    "specs/registry/ipa-tie-bar-vectors.json",
];

fn main() -> ExitCode {
    let root = match repo_root() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot locate the repository root: {e}");
            return ExitCode::FAILURE;
        }
    };

    // --- 1. The lockfile digest -----------------------------------------
    let lock_path = root.join(LOCKFILE);
    let lock_bytes = match fs::read(&lock_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {LOCKFILE}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let lock_digest = hex(&sha256(&lock_bytes));
    let lock_line = format!("{lock_digest}  {LOCKFILE}\n");
    if let Err(e) = write_if_changed(&root.join(LOCKFILE_SHA), &lock_line) {
        eprintln!("cannot write {LOCKFILE_SHA}: {e}");
        return ExitCode::FAILURE;
    }

    // --- 2. The resolved dependency set ---------------------------------
    let deps = match parse_lockfile(&lock_bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cannot parse {LOCKFILE}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut dep_lines = String::new();
    dep_lines.push_str("# ELANG v0.1.0 — resolved dependency set\n");
    dep_lines.push_str(&format!(
        "# Generated from {LOCKFILE}; sha256 {lock_digest}\n"
    ));
    dep_lines
        .push_str("# Integrity only: these digests do not establish publisher authenticity.\n");
    dep_lines.push_str("#\n# name version source\n");
    for dep in &deps {
        dep_lines.push_str(&format!("{} {} {}\n", dep.name, dep.version, dep.source));
    }
    if let Err(e) = write_if_changed(&root.join(ARCHIVE_DIR).join("dependencies.txt"), &dep_lines) {
        eprintln!("cannot write dependencies.txt: {e}");
        return ExitCode::FAILURE;
    }

    // --- 3. The pinned repertoire data ------------------------------------
    let mut rep_lines = String::new();
    rep_lines.push_str("# ELANG v0.1.0 — pinned Unicode 18.0 / IPA 2020 reference data\n");
    rep_lines
        .push_str("# Integrity only: these digests do not establish publisher authenticity.\n");
    rep_lines.push_str("#\n");
    for rel in REPERTOIRE_FILES {
        match digest_of(&root, rel) {
            Ok(d) => rep_lines.push_str(&format!("{d}  {rel}\n")),
            Err(e) => {
                eprintln!("cannot digest {rel}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    rep_lines.push_str("#\n# Published registry files\n");
    for rel in REGISTRY_FILES {
        match digest_of(&root, rel) {
            Ok(d) => rep_lines.push_str(&format!("{d}  {rel}\n")),
            Err(e) => {
                eprintln!("cannot digest {rel}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(e) = write_if_changed(
        &root.join(ARCHIVE_DIR).join("repertoire.sha256"),
        &rep_lines,
    ) {
        eprintln!("cannot write repertoire.sha256: {e}");
        return ExitCode::FAILURE;
    }

    // --- 4. The conformance fixture hashes -------------------------------
    let index_path = root.join("fixtures/corpus_index.json");
    let index_bytes = match fs::read(&index_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read fixtures/corpus_index.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let index: serde_json::Value = match serde_json::from_slice(&index_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot parse fixtures/corpus_index.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut conf_lines = String::new();
    conf_lines.push_str("# ELANG v0.1.0 — published conformance fixture hashes\n");
    conf_lines
        .push_str("# Integrity only: these digests do not establish publisher authenticity.\n");
    conf_lines.push_str("#\n");
    let mut count = 0usize;
    if let Some(fixtures) = index.get("fixtures").and_then(serde_json::Value::as_array) {
        for fixture in fixtures {
            let published = fixture
                .get("published")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !published {
                continue;
            }
            let id = fixture
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            let sha = fixture
                .get("sha256")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            conf_lines.push_str(&format!("{sha}  {id}\n"));
            count = count.saturating_add(1);
        }
    }
    if let Err(e) = write_if_changed(
        &root.join(ARCHIVE_DIR).join("conformance-hashes.txt"),
        &conf_lines,
    ) {
        eprintln!("cannot write conformance-hashes.txt: {e}");
        return ExitCode::FAILURE;
    }

    // --- 5. The human-readable index --------------------------------------
    let archive_md = render_archive_md(&deps, count, &lock_digest);
    if let Err(e) = write_if_changed(&root.join(ARCHIVE_DIR).join("ARCHIVE.md"), &archive_md) {
        eprintln!("cannot write ARCHIVE.md: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!(
        "archived {} dependencies and {} conformance fixtures; wrote {ARCHIVE_DIR}/ARCHIVE.md",
        deps.len(),
        count
    );
    ExitCode::SUCCESS
}

/// The repository root, resolved from the crate manifest directory.
fn repo_root() -> Result<std::path::PathBuf, String> {
    let manifest = std::env!("CARGO_MANIFEST_DIR");
    let mut p = std::path::PathBuf::from(manifest);
    p.push("../..");
    p.canonicalize().map_err(|e| e.to_string())
}

/// One resolved dependency.
struct Dependency {
    name: String,
    version: String,
    source: String,
}

/// Parse the `Cargo.lock` TOML into a flat dependency list.
///
/// The lockfile is parsed with a small, purpose-built scanner rather
/// than a TOML crate: the workspace deliberately has no TOML
/// dependency, and the lockfile's structure is fixed and simple (a
/// `[[package]]` table per entry with `name`, `version` and an
/// optional `source`).
fn parse_lockfile(bytes: &[u8]) -> Result<Vec<Dependency>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut deps = Vec::new();
    let mut current: Option<(String, String, Option<String>)> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            if let Some(entry) = current.take() {
                deps.push(Dependency {
                    name: entry.0,
                    version: entry.1,
                    source: entry.2.unwrap_or_else(|| "path".to_string()),
                });
            }
            current = Some((String::new(), String::new(), None));
            continue;
        }
        if trimmed == "[" {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').to_string();
        let Some(entry) = current.as_mut() else {
            continue;
        };
        match key {
            "name" => entry.0 = value,
            "version" => entry.1 = value,
            "source" => entry.2 = Some(value),
            _ => continue,
        }
    }
    if let Some(entry) = current.take() {
        deps.push(Dependency {
            name: entry.0,
            version: entry.1,
            source: entry.2.unwrap_or_else(|| "path".to_string()),
        });
    }

    deps.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(deps)
}

/// The SHA-256 of a repository-relative file, as lowercase hex.
fn digest_of(root: &Path, rel: &str) -> Result<String, String> {
    let bytes = fs::read(root.join(rel)).map_err(|e| e.to_string())?;
    Ok(hex(&sha256(&bytes)))
}

fn render_archive_md(deps: &[Dependency], fixture_count: usize, lock_digest: &str) -> String {
    let mut out = String::new();
    out.push_str("# ELANG v0.1.0 — release archive\n\n");
    out.push_str("> Generated by `gen_release_archive`. Every digest below attests **integrity** — that the bytes are the ones the release used. None of them attests **publisher authenticity**; that requires a signed tag or an out-of-band trust mechanism, and v0.1.0 does not ship one.\n\n");

    out.push_str("## Contents\n\n");
    out.push_str("| File | What it records |\n|---|---|\n");
    // This file is written to `docs/release/v0.1.0/`, so a relative link
    // to a repository-root file needs three `../` segments, not two.
    out.push_str("| [`Cargo.lock.sha256`](../../../Cargo.lock.sha256) | Digest of the resolved dependency lockfile |\n");
    out.push_str("| [`dependencies.txt`](dependencies.txt) | Every resolved dependency: name, version, registry source |\n");
    out.push_str("| [`repertoire.sha256`](repertoire.sha256) | Digests of the pinned Unicode 18.0 / IPA 2020 data and the published registry files |\n");
    out.push_str("| [`conformance-hashes.txt`](conformance-hashes.txt) | Digests of every published conformance fixture |\n");
    out.push_str("| `reproducible-build-evidence.md` | Produced by the `reproducible-build` CI workflow at run time; **not present in a source checkout** |\n");
    out.push_str("| [`README.md`](README.md) | The v0.1.0 release notes |\n\n");

    out.push_str("## Dependency lockfile\n\n");
    out.push_str(&format!(
        "- **File:** `Cargo.lock`\n- **SHA-256:** `{lock_digest}`\n- **Resolved packages:** {}\n\n",
        deps.len()
    ));

    out.push_str("## Conformance fixtures\n\n");
    out.push_str(&format!(
        "- **Published fixtures:** {fixture_count}\n- **Index:** [`fixtures/corpus_index.json`](../../../fixtures/corpus_index.json)\n- **Schema:** [`fixtures/expected-result.schema.json`](../../../fixtures/expected-result.schema.json)\n\n"
    ));

    out.push_str("## Regenerating this archive\n\n");
    out.push_str("```sh\ncargo run -p elang-conformance --bin gen_release_archive\n```\n\n");
    out.push_str(
        "The generator is idempotent: re-running it on an unchanged tree rewrites nothing.\n\n",
    );

    out.push_str("## Limits of this archive\n\n");
    out.push_str("- The digests prove **what** the bytes are, not **who** produced them.\n");
    out.push_str("- The archive covers the *inputs* to the release (dependencies, normative data, conformance fixtures). The release binaries themselves are produced by the `reproducible-build` workflow, which records their reproducibility evidence separately.\n");
    out.push_str("- No artifact in this archive is signed, and v0.1.0 makes no publisher-authenticity claim.\n");

    out
}

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return Ok(());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
