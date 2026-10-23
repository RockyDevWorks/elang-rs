//! Integration tests for `elang-integrity` (T0705, partial T0710).
//!
//! Exercises the public API as the reader SDK and package writer will:
//! parse → validate → coverage check. Per-resource SHA-256 verification and
//! the canonical writer land in the follow-up PRs (T0706–T0709).
//!
//! The unit tests in `src/parse.rs` and `src/manifest.rs` cover the grammar
//! in isolation; this file covers the composed pipeline and the boundaries
//! the issue's acceptance criteria call out (golden roundtrip, coverage
//! pass / fail, manifest self-exclusion).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration tests assert exact bytes/codes; a panic signals a regression"
)]
#![allow(
    clippy::panic,
    reason = "test assertion macros require panic to signal failure"
)]

use std::collections::BTreeSet;

use elang_core::ids::ResourcePath;
use elang_integrity::{parse, verify_coverage};

/// The canonical golden manifest. Byte-for-byte stable; matches the source
/// shape that the canonical writer (T0709) must reproduce.
const GOLDEN: &str = "\
integrity {
    algorithm \"sha-256\"
    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"Linguistics/phonemes.eld\" sha256=\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"
    resource \"mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";

fn path(s: &str) -> ResourcePath {
    ResourcePath::new(s.to_string()).expect("valid path")
}

#[test]
fn golden_vector_roundtrips_via_coverage_check() {
    let manifest = parse(GOLDEN.as_bytes()).expect("golden must parse");
    let mut indexed = BTreeSet::new();
    indexed.insert(path("Header/format.version"));
    indexed.insert(path("Linguistics/phonemes.eld"));
    indexed.insert(path("mimetype"));
    indexed.insert(path("package.epf"));
    let self_path = path("META-INF/integrity.kdl");
    verify_coverage(&manifest, &indexed, &self_path).expect("exact coverage must pass");
}

#[test]
fn coverage_rejects_extra_manifest_entry() {
    // The manifest has package.epf; the index does not. The coverage rule
    // requires every manifest entry to be an indexed path — this is an
    // EXTRA_ENTRY, not a MISSING_ENTRY.
    let manifest = parse(GOLDEN.as_bytes()).expect("golden must parse");
    let mut indexed = BTreeSet::new();
    indexed.insert(path("Header/format.version"));
    indexed.insert(path("Linguistics/phonemes.eld"));
    indexed.insert(path("mimetype"));
    // package.epf deliberately omitted from the index
    let self_path = path("META-INF/integrity.kdl");
    let err = verify_coverage(&manifest, &indexed, &self_path).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_EXTRA_ENTRY");
    assert!(err.diagnostic.message.contains("package.epf"));
}

#[test]
fn coverage_rejects_missing_indexed_resource() {
    // The index has package.epf but the manifest does not. This is the
    // MISSING_ENTRY direction.
    let input = "\
integrity {
    algorithm \"sha-256\"
    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"Linguistics/phonemes.eld\" sha256=\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"
    resource \"mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";
    let manifest = parse(input.as_bytes()).expect("must parse");
    let mut indexed = BTreeSet::new();
    indexed.insert(path("Header/format.version"));
    indexed.insert(path("Linguistics/phonemes.eld"));
    indexed.insert(path("mimetype"));
    indexed.insert(path("package.epf"));
    let self_path = path("META-INF/integrity.kdl");
    let err = verify_coverage(&manifest, &indexed, &self_path).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_MISSING_ENTRY");
    assert!(err.diagnostic.message.contains("package.epf"));
}

#[test]
fn coverage_rejects_self_entry() {
    // Build a manifest that erroneously covers itself.
    let input = "\
integrity {
    algorithm \"sha-256\"
    resource \"META-INF/integrity.kdl\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";
    let manifest = parse(input.as_bytes()).expect("must parse");
    let mut indexed = BTreeSet::new();
    indexed.insert(path("META-INF/integrity.kdl"));
    let self_path = path("META-INF/integrity.kdl");
    let err = verify_coverage(&manifest, &indexed, &self_path).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_EXTRA_ENTRY");
    assert!(err.diagnostic.message.contains("integrity.kdl"));
}

#[test]
fn coverage_rejects_non_indexed_entry() {
    // Build a manifest that covers a path the index does not contain.
    let input = "\
integrity {
    algorithm \"sha-256\"
    resource \"not-in-index.eld\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";
    let manifest = parse(input.as_bytes()).expect("must parse");
    let indexed = BTreeSet::new();
    let self_path = path("META-INF/integrity.kdl");
    let err = verify_coverage(&manifest, &indexed, &self_path).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_EXTRA_ENTRY");
    assert!(err.diagnostic.message.contains("not-in-index.eld"));
}

#[test]
fn parse_rejects_non_nfc_path_arg() {
    // Composed char (NFC violation): "é" can be represented as either
    // U+00E9 (precomposed) or U+0065 U+0301 (decomposed). The decomposed
    // form is not in NFC and must be rejected.
    let input = "\
integrity {
    algorithm \"sha-256\"
    resource \"e\u{0301}.eld\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";
    let err = parse(input.as_bytes()).unwrap_err();
    // The exact code is whatever elang-unicode reports; we only assert the
    // category.
    assert_eq!(
        err.diagnostic.category,
        elang_core::error::Category::Unicode
    );
}

#[test]
fn parse_returns_algorithm_and_resource_count() {
    let manifest = parse(GOLDEN.as_bytes()).expect("golden must parse");
    assert_eq!(manifest.algorithm, elang_integrity::Algorithm::Sha256);
    assert_eq!(manifest.resources.len(), 4);
}
