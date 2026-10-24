//! Shared test fixture for the integrated reader pipeline.
//!
//! Builds an in-memory ELANG v0.1.0 archive with **real** Zstandard frames
//! from a small set of canonical resource payloads. The archive is fully
//! self-consistent: every indexed resource has a manifest entry whose
//! SHA-256 matches the decompressed bytes, the bootstrap resources agree
//! with the binary header, and the EPF `phonemes` path resolves to an
//! indexed `.eld` resource.
//!
//! Tests include `#[allow(clippy::unwrap_used, ...)]` at the top because
//! fixture building is a controlled-environment operation — a panic
//! signals a fixture bug, not an untrusted-input failure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test fixture builder; a panic signals a fixture bug, not untrusted-input failure"
)]

use elang_container::footer::FOOTER_LEN;
use elang_container::frame::{encode, Profile};
use elang_container::header::{Header, HEADER_LEN};
use elang_container::index::{ENTRY_FORMAT, INDEX_HEADER_LEN, INDEX_MAGIC, INDEX_VERSION};
use sha2::{Digest, Sha256};

/// Canonical EPF payload. Uses the minimal five top-level nodes with a
/// single phoneme resource reference.
pub const EPF_PAYLOAD: &[u8] =
    b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Reader Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

/// Canonical mimetype.
pub const MIMETYPE_PAYLOAD: &[u8] = b"application/vnd.elang";

/// Canonical `format.version`.
pub const FORMAT_VERSION_PAYLOAD: &[u8] = b"0.1.0\n";

/// Canonical ELD payload — minimal two-group inventory.
pub const ELD_PAYLOAD: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

/// Canonical integrity manifest payload.
///
/// Each entry's SHA-256 is the SHA-256 of the *uncompressed* bytes (per
/// steering §6 / T0706). `META-INF/integrity.kdl` itself is excluded
/// (T1206 self-exclusion).
pub fn integrity_payload(digests: &[(String, [u8; 32])]) -> Vec<u8> {
    let mut out = String::from("integrity {\n    algorithm \"sha-256\"\n");
    for (path, digest) in digests {
        let hex: String = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join("");
        out.push_str(&format!("    resource \"{path}\" sha256=\"{hex}\"\n"));
    }
    out.push_str("}\n");
    out.into_bytes()
}

/// One resource spec for the in-memory archive builder.
#[derive(Clone)]
pub struct Spec {
    pub path: &'static str,
    pub uncompressed: Vec<u8>,
}

impl Spec {
    pub fn new(path: &'static str, uncompressed: &'static [u8]) -> Self {
        Self {
            path,
            uncompressed: uncompressed.to_vec(),
        }
    }

    /// Build a `Spec` from an owned byte buffer (e.g. a manifest computed at
    /// runtime). Use this when the bytes do not have a `'static` lifetime.
    pub fn owned(path: &'static str, uncompressed: Vec<u8>) -> Self {
        Self { path, uncompressed }
    }
}

/// A fully built archive ready to feed into the reader.
pub struct BuiltArchive {
    pub bytes: Vec<u8>,
    /// Decompressed resources keyed by their canonical path, in sorted
    /// (index) order. Excludes `META-INF/integrity.kdl` so the manifest's
    /// declared SHA-256s can be verified directly.
    pub resources: Vec<(String, Vec<u8>)>,
}

/// Build a valid archive from `specs`. Paths are sorted by raw UTF-8
/// ascending bytes (stage 5 invariant). The integrity manifest is computed
/// from the real SHA-256s of the *uncompressed* bytes.
pub fn build(specs: &[Spec]) -> BuiltArchive {
    let mut specs: Vec<&Spec> = specs.iter().collect();
    specs.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    for w in specs.windows(2) {
        if let (Some(a), Some(b)) = (w.first(), w.get(1)) {
            assert_ne!(a.path, b.path, "duplicate resource path in fixture");
        }
    }

    let mut frame_bytes = Vec::new();
    let mut decoded: Vec<(String, Vec<u8>)> = Vec::with_capacity(specs.len());
    let mut entries: Vec<(String, u64, u64, u64)> = Vec::with_capacity(specs.len());

    let mut cursor = u64::try_from(HEADER_LEN).expect("HEADER_LEN fits u64");
    for spec in &specs {
        let compressed = encode(spec.uncompressed.as_slice(), Profile::Fast).expect("zstd encode");
        entries.push((
            spec.path.to_string(),
            cursor,
            u64::try_from(compressed.len()).expect("compressed size fits u64"),
            u64::try_from(spec.uncompressed.len()).expect("uncompressed size fits u64"),
        ));
        decoded.push((spec.path.to_string(), spec.uncompressed.to_vec()));
        cursor = cursor
            .checked_add(u64::try_from(compressed.len()).expect("size fits u64"))
            .expect("cursor overflow");
        frame_bytes.extend_from_slice(&compressed);
    }
    let index_offset = cursor;

    let mut index_bytes = Vec::with_capacity(INDEX_HEADER_LEN);
    index_bytes.extend_from_slice(&INDEX_MAGIC);
    index_bytes.extend_from_slice(&INDEX_VERSION.to_le_bytes());
    index_bytes.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
    let count = u32::try_from(entries.len()).expect("resource count fits u32");
    index_bytes.extend_from_slice(&count.to_le_bytes());

    for (path, offset, compressed_size, uncompressed_size) in &entries {
        let path_bytes = path.as_bytes();
        let path_len = u16::try_from(path_bytes.len()).expect("path length fits u16");
        index_bytes.extend_from_slice(&path_len.to_le_bytes());
        index_bytes.extend_from_slice(path_bytes);
        index_bytes.extend_from_slice(&offset.to_le_bytes());
        index_bytes.extend_from_slice(&compressed_size.to_le_bytes());
        index_bytes.extend_from_slice(&uncompressed_size.to_le_bytes());
        index_bytes.push(1); // compression = zstd
    }
    let index_size = u64::try_from(index_bytes.len()).expect("index size fits u64");
    let index_sha256: [u8; 32] = Sha256::digest(&index_bytes).into();

    let mut footer = Vec::with_capacity(FOOTER_LEN);
    footer.extend_from_slice(b"ELEND001");
    footer.extend_from_slice(&index_offset.to_le_bytes());
    footer.extend_from_slice(&index_size.to_le_bytes());
    footer.extend_from_slice(&index_sha256);
    footer.extend_from_slice(&[0u8; 8]);
    assert_eq!(footer.len(), FOOTER_LEN);

    let header = Header::current();
    let mut header_buf = vec![0u8; HEADER_LEN];
    header.encode(&mut header_buf, 0).expect("header encode");

    let mut archive = Vec::new();
    archive.extend_from_slice(&header_buf);
    archive.extend_from_slice(&frame_bytes);
    archive.extend_from_slice(&index_bytes);
    archive.extend_from_slice(&footer);

    let file_size = u64::try_from(archive.len()).expect("archive size fits u64");
    let footer_len = u64::try_from(FOOTER_LEN).expect("footer fits u64");
    assert_eq!(
        index_offset
            .checked_add(index_size)
            .and_then(|s| s.checked_add(footer_len)),
        Some(file_size),
        "index_offset + index_size + FOOTER_LEN == file_size"
    );

    BuiltArchive {
        bytes: archive,
        resources: decoded,
    }
}

/// Build the manifest payload for the given resource set. Each entry's
/// SHA-256 is the SHA-256 of the *uncompressed* bytes. The
/// `META-INF/integrity.kdl` resource (if present) is excluded.
pub fn manifest_for(resources: &[(String, Vec<u8>)]) -> Vec<u8> {
    let digests: Vec<(String, [u8; 32])> = resources
        .iter()
        .filter(|(p, _)| p != "META-INF/integrity.kdl")
        .map(|(p, b)| (p.clone(), Sha256::digest(b).into()))
        .collect();
    integrity_payload(&digests)
}

/// Build the canonical happy-path archive. The integrity manifest is
/// computed from the real SHA-256s and used as the resource bytes for
/// `META-INF/integrity.kdl`.
pub fn build_canonical() -> BuiltArchive {
    // Step 1: build the archive with placeholder bytes for the manifest so
    // we know the offset/length/size constraints are satisfied.
    let placeholder = b"integrity {\n}\n";
    let specs = vec![
        Spec::new("Header/mimetype", MIMETYPE_PAYLOAD),
        Spec::new("Header/format.version", FORMAT_VERSION_PAYLOAD),
        Spec::new("META-INF/integrity.kdl", placeholder),
        Spec::new("package.epf", EPF_PAYLOAD),
        Spec::new("Linguistics/phonemes.eld", ELD_PAYLOAD),
    ];
    let built_first = build(&specs);

    // Step 2: compute the manifest payload from the *other* resources
    // (excluding the manifest itself) and re-build with that as the
    // manifest content.
    let manifest_bytes = manifest_for(&built_first.resources);
    let manifest_for_built = manifest_bytes.clone();
    let specs2 = vec![
        Spec::new("Header/mimetype", MIMETYPE_PAYLOAD),
        Spec::new("Header/format.version", FORMAT_VERSION_PAYLOAD),
        Spec::owned("META-INF/integrity.kdl", manifest_bytes),
        Spec::new("package.epf", EPF_PAYLOAD),
        Spec::new("Linguistics/phonemes.eld", ELD_PAYLOAD),
    ];
    let built = build(&specs2);

    // Step 3: validate that the manifest's digests match the real resource
    // bytes — the fixture is consistent.
    let recomputed = manifest_for(&built.resources);
    assert_eq!(
        recomputed, manifest_for_built,
        "manifest is self-consistent"
    );
    built
}

/// Look up a resource in a built archive by its canonical path.
#[allow(dead_code, reason = "used by other test files")]
pub fn resource<'a>(archive: &'a BuiltArchive, path: &str) -> Option<&'a [u8]> {
    archive
        .resources
        .iter()
        .find(|(p, _)| p == path)
        .map(|(_, b)| b.as_slice())
}

/// Build an archive from `specs` and sign its integrity manifest with
/// `manifest_override` (an externally-supplied, already-serialised
/// manifest payload). This lets a test trigger stage-11
/// `INTEGRITY_HASH_MISMATCH` by giving the archive a manifest that
/// disagrees with the actual resource bytes — no internal hacks required.
#[allow(
    dead_code,
    reason = "used by failure tests; flagged dead by success-only test runs"
)]
pub fn build_with_manifest(specs: &[Spec], manifest_override: &[u8]) -> BuiltArchive {
    let mut specs: Vec<Spec> = specs.to_vec();
    // Replace whatever `META-INF/integrity.kdl` spec exists with the
    // externally-supplied manifest payload.
    specs.retain(|s| s.path != "META-INF/integrity.kdl");
    specs.push(Spec::owned(
        "META-INF/integrity.kdl",
        manifest_override.to_vec(),
    ));
    build(&specs)
}

/// Build a canonical archive but replace one resource's uncompressed
/// content before signing the integrity manifest. The returned archive's
/// `META-INF/integrity.kdl` reflects the **replaced** content (so stage 9
/// passes), so a downstream tamper to the resource's *bytes* (without
/// rebuilding the manifest) trips stage 11.
#[allow(dead_code, reason = "used by other test files")]
pub fn build_with_swapped(path: &str, new_content: Vec<u8>) -> BuiltArchive {
    let placeholder = b"integrity {\n}\n";
    let mut specs = vec![
        Spec::new("Header/mimetype", MIMETYPE_PAYLOAD),
        Spec::new("Header/format.version", FORMAT_VERSION_PAYLOAD),
        Spec::new("META-INF/integrity.kdl", placeholder),
        Spec::new("package.epf", EPF_PAYLOAD),
        Spec::new("Linguistics/phonemes.eld", ELD_PAYLOAD),
    ];
    for spec in &mut specs {
        if spec.path == path {
            spec.uncompressed = new_content.clone();
        }
    }
    let built_first = build(&specs);

    let manifest_bytes = manifest_for(&built_first.resources);
    let manifest_for_built = manifest_bytes.clone();
    let mut specs2 = specs;
    if let Some(slot) = specs2
        .iter_mut()
        .find(|s| s.path == "META-INF/integrity.kdl")
    {
        slot.uncompressed = manifest_bytes;
    }
    let built = build(&specs2);

    let recomputed = manifest_for(&built.resources);
    assert_eq!(
        recomputed, manifest_for_built,
        "manifest is self-consistent for {path}"
    );
    built
}
