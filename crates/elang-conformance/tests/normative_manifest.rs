//! Round-trip verification of the OA-17 normative artifact manifest (T0123).
//!
//! The manifest at `specs/registry/normative-manifest.json` (reached via
//! `CARGO_MANIFEST_DIR`) is the freeze-mechanism half of OA-17: a generated,
//! RFC 8785 JCS canonical serialization of every ratified normative artifact
//! in the v0.1.0 corpus, with each artifact's SHA-256, plus the pinned digest
//! of the manifest itself (`PINNED_SPEC_CHECKSUM`).
//!
//! These tests assert the whole chain:
//!
//! 1. the committed manifest is byte-identical to a fresh regeneration
//!    (deterministic generator, no hand-editing);
//! 2. the committed bytes hash to `PINNED_SPEC_CHECKSUM`;
//! 3. every artifact's recorded digest matches the bytes on disk, and the
//!    artifact list is complete in both directions;
//! 4. the manifest excludes itself, so the digest is not circular.
//!
//! The pinned digest is **provisional**: ADR 0011 requires regeneration
//! during final T0123 once OA-14 (canonical ELD goldens) has landed. The
//! release-tag half of OA-17 stays blocked by OA-14 and is not implemented
//! here.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use elang_conformance::normative_manifest::{
    self, artifact_entries, ARTIFACTS, MANIFEST_PATH, PINNED_SPEC_CHECKSUM, RELEASE,
};

fn release_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

#[test]
fn regenerating_the_manifest_is_byte_identical() {
    let root = release_root();
    let (_value, bytes) = normative_manifest::build_manifest(&root).expect("build manifest");
    let committed = fs::read(root.join(MANIFEST_PATH)).expect("read committed manifest");
    assert_eq!(
        bytes, committed,
        "regenerated manifest must be byte-identical to the committed one"
    );
}

#[test]
fn pinned_checksum_matches_the_committed_manifest() {
    let root = release_root();
    let committed = fs::read(root.join(MANIFEST_PATH)).expect("read committed manifest");
    let digest = sha256_hex(&committed);
    assert_eq!(
        digest, PINNED_SPEC_CHECKSUM,
        "PINNED_SPEC_CHECKSUM does not match the committed manifest bytes"
    );
}

#[test]
fn committed_manifest_verifies_against_the_artifact_corpus() {
    let root = release_root();
    normative_manifest::verify(&root).expect("manifest verifies against the corpus on disk");
}

#[test]
fn manifest_excludes_itself() {
    assert!(
        !ARTIFACTS.iter().any(|a| a.path == MANIFEST_PATH),
        "the manifest must not cover itself (circular digest)"
    );
    assert!(
        !artifact_entries()
            .iter()
            .any(|(_, path)| path == MANIFEST_PATH),
        "the expanded corpus must not cover the manifest itself (circular digest)"
    );
}

#[test]
fn artifact_paths_are_unique_and_nonempty() {
    let mut seen = BTreeSet::new();
    for a in ARTIFACTS {
        assert!(
            seen.insert(a.path),
            "duplicate artifact path in ARTIFACTS: {}",
            a.path
        );
    }
    assert!(!ARTIFACTS.is_empty(), "the corpus must not be empty");

    // The same uniqueness guarantee must hold across the *expanded* corpus
    // (ARTIFACTS plus the OA-14 canonical ELD expansion). A duplicate path
    // in the expansion would make the manifest carry two entries for one
    // file, and the per-artifact digest check would silently cover only
    // one of them.
    let mut expanded = BTreeSet::new();
    for (_id, path) in artifact_entries() {
        assert!(
            expanded.insert(path.clone()),
            "duplicate artifact path in the expanded corpus: {path}"
        );
        assert!(!path.is_empty(), "artifact path must not be empty");
    }
}

#[test]
fn manifest_declares_the_expected_release_and_schema() {
    let root = release_root();
    let committed = fs::read(root.join(MANIFEST_PATH)).expect("read committed manifest");
    let value: serde_json::Value = serde_json::from_slice(&committed).expect("parse manifest");
    assert_eq!(
        value.get("release").and_then(serde_json::Value::as_str),
        Some(RELEASE)
    );
    assert_eq!(
        value.get("schema").and_then(serde_json::Value::as_str),
        Some(normative_manifest::MANIFEST_SCHEMA)
    );
    let artifacts = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .expect("artifacts array");
    assert_eq!(
        artifacts.len(),
        artifact_entries().len(),
        "manifest must list every artifact exactly once"
    );
}

/// The OA-14 canonical ELD corpus must be inside the freeze: the vector
/// document plus, for every published case, the authored source, the
/// byte-exact golden and the conformance record.
///
/// ADR 0011 §6 made the OA-14 corpus the *last* step of the freeze — the
/// generator and the freeze share `canonical_goldens::CASES` as their
/// single source of truth, so this test proves the corpus scope actually
/// reached the manifest rather than merely being intended to.
#[test]
fn the_oa14_canonical_corpus_is_frozen() {
    let root = release_root();
    let committed = fs::read(root.join(MANIFEST_PATH)).expect("read committed manifest");
    let value: serde_json::Value = serde_json::from_slice(&committed).expect("parse manifest");
    let artifacts = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .expect("artifacts array");

    let frozen: BTreeSet<&str> = artifacts
        .iter()
        .filter_map(|e| e.get("path").and_then(serde_json::Value::as_str))
        .collect();

    // The vector document.
    assert!(
        frozen.contains("specs/registry/eld-canonical-vectors.json"),
        "the OA-14 vector document must be part of the freeze"
    );

    // Every case's three files.
    for case in elang_conformance::canonical_goldens::CASES {
        for file in ["expected.json", "golden/canonical.eld", "input.eld"] {
            let path = format!("fixtures/valid/canonical/{}/{file}", case.id);
            assert!(
                frozen.contains(path.as_str()),
                "OA-14 corpus file must be frozen: {path}"
            );
        }
    }
}

/// The `.sha256` sidecars are derived from the goldens that are already
/// frozen, so they must NOT be listed — a second source of truth for the
/// same digest is exactly what the freeze is meant to avoid.
#[test]
fn derived_sha256_sidecars_are_not_frozen() {
    let root = release_root();
    let committed = fs::read(root.join(MANIFEST_PATH)).expect("read committed manifest");
    let value: serde_json::Value = serde_json::from_slice(&committed).expect("parse manifest");
    let artifacts = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .expect("artifacts array");

    for entry in artifacts {
        let path = entry
            .get("path")
            .and_then(serde_json::Value::as_str)
            .expect("entry.path");
        assert!(
            !path.ends_with(".sha256"),
            "derived .sha256 sidecar must not be frozen: {path}"
        );
    }
}

#[test]
fn the_jcs_writer_emits_non_bmp_scalars_literally() {
    // RFC 8785 §3.2.2.2 escapes only `"`, `\` and control characters below
    // U+0020. A non-BMP scalar is emitted as raw UTF-8; the surrogate-pair
    // form is JSON-legal but not JCS-canonical. The current corpus is ASCII
    // only, so this pins the writer rather than the committed digest.
    let value = serde_json::json!({ "k": "\u{1F600}" });
    assert_eq!(
        normative_manifest::jcs_canonicalize(&value),
        "{\"k\":\"\u{1F600}\"}"
    );
    // The two mandatory escapes and control characters are still escaped:
    // `"` and `\` become `\"` and `\\`, U+000A becomes `\n`, U+0001 becomes
    // `\u0001`.
    let value = serde_json::json!({ "k": "a\"b\\c\n\u{1}" });
    assert_eq!(
        normative_manifest::jcs_canonicalize(&value),
        "{\"k\":\"a\\\"b\\\\c\\n\\u0001\"}"
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len().saturating_mul(2));
    for b in out {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
