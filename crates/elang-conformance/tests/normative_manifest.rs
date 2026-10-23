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
    self, ARTIFACTS, MANIFEST_PATH, PINNED_SPEC_CHECKSUM, RELEASE,
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
        ARTIFACTS.len(),
        "manifest must list every artifact exactly once"
    );
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
