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
use elang_integrity::{
    parse, verify_coverage, verify_one, write::canonical_bytes, write::CANONICAL_GOLDEN, Hasher,
    IntegrityEntry,
};

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

// ---- T0706/T0707: streaming SHA-256 + per-resource verify -----------------

/// Build a [`Hasher`], feed `bytes` one chunk at a time using `chunk_sizes`,
/// and return the digest.
///
/// The arithmetic in `streaming_digest` is bounded by `bytes.len()` (the
/// `min` on line 167 keeps `off + size <= bytes.len()`; the tail-slab
/// starts at `off <= bytes.len()`) so the index/slice expressions are
/// provably safe. The workspace lint targets *accidental* unchecked math
/// on untrusted input; here the input is a test-controlled `&[u8]`.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "streaming helper walks a bounded offset over test-controlled bytes; bounds are guarded by .min(bytes.len())"
)]
fn streaming_digest(bytes: &[u8], chunk_sizes: &[usize]) -> [u8; 32] {
    let mut h = Hasher::new();
    let mut off = 0;
    for &size in chunk_sizes {
        let end = (off + size).min(bytes.len());
        h.update(&bytes[off..end]);
        off = end;
    }
    if off < bytes.len() {
        h.update(&bytes[off..]);
    }
    h.finalize()
}

#[test]
fn verify_one_accepts_streaming_digest_of_known_body() {
    // "abc" split across uneven chunks must hash identically to one shot.
    let body = b"abc";
    let whole = {
        let mut h = Hasher::new();
        h.update(body);
        h.finalize()
    };
    let streamed = streaming_digest(body, &[1, 1, 1]);
    assert_eq!(whole, streamed);

    // Plug the streamed digest into an entry with a matching declared
    // digest and confirm verify_one accepts it.
    let entry = IntegrityEntry {
        path: path("Header/format.version"),
        digest: streamed,
    };
    verify_one(&entry, &streamed).expect("matching digests must pass");
}

#[test]
fn verify_one_rejects_byte_mutation_with_resource_path() {
    // Simulate a single-byte mutation in a resource body. The streaming
    // digest changes; verify_one emits INTEGRITY_HASH_MISMATCH with the
    // resource path attached (T0710: "byte mutation").
    let original = b"the quick brown fox";
    let original_digest = {
        let mut h = Hasher::new();
        h.update(original);
        h.finalize()
    };
    let mut mutated = *original;
    mutated[4] ^= 0x01; // flip a single bit of 'q'
    let mutated_digest = {
        let mut h = Hasher::new();
        h.update(&mutated);
        h.finalize()
    };
    assert_ne!(original_digest, mutated_digest);

    let entry = IntegrityEntry {
        path: path("Linguistics/phonemes.eld"),
        digest: original_digest,
    };
    let err = verify_one(&entry, &mutated_digest).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_HASH_MISMATCH");
    assert_eq!(err.diagnostic.stage, Some(11));
    assert_eq!(
        err.diagnostic.resource_path.as_deref(),
        Some("Linguistics/phonemes.eld")
    );
}

#[test]
fn verify_one_rejects_resource_swap_with_distinct_paths() {
    // Resource A's declared digest, fed with resource B's body (T0710:
    // "resource swaps"). The mismatch must surface with A's path because
    // A is the resource the entry claims to cover.
    let body_a = b"alpha payload";
    let body_b = b"beta payload -- different content entirely";
    let digest_a = {
        let mut h = Hasher::new();
        h.update(body_a);
        h.finalize()
    };
    let digest_b = {
        let mut h = Hasher::new();
        h.update(body_b);
        h.finalize()
    };
    assert_ne!(digest_a, digest_b);

    let entry = IntegrityEntry {
        path: path("Header/format.version"),
        digest: digest_a,
    };
    let err = verify_one(&entry, &digest_b).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_HASH_MISMATCH");
    assert_eq!(
        err.diagnostic.resource_path.as_deref(),
        Some("Header/format.version")
    );
}

#[test]
fn verify_one_rejects_empty_body_against_nonempty_digest() {
    // Empty bodies are valid streams but produce the canonical empty
    // SHA-256. A declared digest of any non-empty body must fail.
    let empty = Hasher::new().finalize();
    let nonzero = {
        let mut h = Hasher::new();
        h.update(b"x");
        h.finalize()
    };
    let entry = IntegrityEntry {
        path: path("package.epf"),
        digest: nonzero,
    };
    let err = verify_one(&entry, &empty).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_HASH_MISMATCH");
}

// ---- T0708: footer index hash separation ----------------------------------

#[test]
fn parse_rejects_footer_hash_property_on_resource() {
    // The integrity manifest must NOT carry the footer index hash. The
    // parser's C-05 rule (no unknown properties on `resource`) rejects any
    // such attempt. This guards ADR 0004's "footer hash is separate"
    // decision structurally.
    let input = "\
integrity {
    algorithm \"sha-256\"
    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" footer_hash=\"deadbeef\"
}
";
    let err = parse(input.as_bytes()).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    assert!(
        err.diagnostic.message.contains("footer_hash"),
        "message should name the rejected property: {}",
        err.diagnostic.message
    );
}

#[test]
fn parse_rejects_footer_hash_as_algorithm_value() {
    // Belt-and-braces: the `algorithm` arg slot is the only place a hash
    // literal could conceivably hide. C-02 forbids anything other than
    // "sha-256".
    let input = "\
integrity {
    algorithm \"footer-hash\"
    resource \"a\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";
    let err = parse(input.as_bytes()).unwrap_err();
    assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_ALGORITHM");
}

// ---- T0709: canonical writer end-to-end ----------------------------------

#[test]
fn canonical_writer_emits_golden_vector() {
    // Round-trip the golden fixture through the canonical writer and
    // assert byte-for-byte equality with the locked fixture (T0709).
    let m = parse(GOLDEN.as_bytes()).expect("golden must parse");
    let bytes = canonical_bytes(&m);
    assert_eq!(
        bytes, CANONICAL_GOLDEN,
        "writer must reproduce the canonical vector byte-for-byte"
    );
}

#[test]
fn canonical_writer_roundtrip_re_parsing() {
    // write → parse must produce the same manifest (idempotence of the
    // canonical profile, REQ-FRAME-010).
    let m1 = parse(GOLDEN.as_bytes()).expect("golden must parse");
    let bytes = canonical_bytes(&m1);
    let m2 = parse(&bytes).expect("written bytes must re-parse");
    assert_eq!(m1, m2);
}

#[test]
fn canonical_writer_digest_matches_verify_one() {
    // End-to-end: compute the SHA-256 of an arbitrary resource body, build
    // a manifest, write it, re-parse, then verify_one with the computed
    // digest. Confirms the writer + parser + verify chain is internally
    // consistent.
    let body = b"some resource content";
    let mut hasher = Hasher::new();
    hasher.update(body);
    let digest = hasher.finalize();
    let hex = elang_integrity::write::CANONICAL_GOLDEN; // borrow module path
    let _ = hex; // (silence unused-import warning if any)

    // Build a 2-entry manifest (algorithm + 1 resource, plus mimetype/package.epf
    // placeholders that the writer will emit in sorted order).
    let input = format!(
        "integrity {{\n    algorithm \"sha-256\"\n    resource \"mimetype\" sha256=\"{hex_digest}\"\n    resource \"Header/format.version\" sha256=\"{hex_digest}\"\n}}\n",
        hex_digest = digest_hex(&digest),
    );
    let manifest = parse(input.as_bytes()).expect("must parse");
    let bytes = canonical_bytes(&manifest);
    let reparsed = parse(&bytes).expect("written bytes must re-parse");

    // The re-parsed manifest's entry for "Header/format.version" must
    // verify against the digest we computed above.
    let entry = reparsed
        .resources
        .values()
        .find(|e| e.path.as_str() == "Header/format.version")
        .expect("entry present");
    verify_one(entry, &digest).expect("declared digest matches computed");
}

fn digest_hex(d: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in d {
        let _ = std::fmt::Write::write_fmt(&mut s, format_args!("{b:02x}"));
    }
    s
}
