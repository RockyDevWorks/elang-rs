//! Coverage-closure tests for `elang-writer`.
//!
//! These tests drive the two writer-side ceilings and rejections that the
//! round-trip integration tests never reach:
//!
//! * the per-resource uncompressed-byte ceiling, tripped by an integrity
//!   manifest whose canonical serialization grows past
//!   `MAX_INTEGRITY_BYTES`, and
//! * the container-level NFC rejection of a resource path, which the writer
//!   only sees when it builds the index entries.
//!
//! Test code path: `.expect()` / `.expect_err()` / panics signal fixture
//! regressions, not untrusted-input failure. The workspace lint policy
//! denies these on production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use elang_core::limits::MAX_INTEGRITY_BYTES;
use elang_writer::{write_archive, BuildError, ExtraResource, PackageConfig, Profile, SizeKind};

// ---------------------------------------------------------------------------
// Canonical inputs (mirrored from tests/writer_integration.rs)
// ---------------------------------------------------------------------------

const FORMAT_VERSION: &str = "0.1.0";

const EPF_BYTES: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

const ELD_BYTES: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

/// Build a config around the given extra resources.
fn build_config(extra_resources: Vec<ExtraResource>) -> PackageConfig {
    let epf = elang_epf::parse(EPF_BYTES).expect("canonical EPF parses");
    let rep = elang_repertoire::repertoire::load().expect("pinned IPA repertoire loads");
    let doc = elang_eld::parse(ELD_BYTES, &rep).expect("canonical ELD parses");
    let eld = elang_eld::inventory::from_document(&doc, &rep).expect("ELD → inventory");
    PackageConfig {
        epf,
        eld,
        extra_resources,
        profile: Profile::Fast,
        format_version: FORMAT_VERSION.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Per-resource ceiling: the integrity manifest
// ---------------------------------------------------------------------------

/// Number of extra resources used to inflate the integrity manifest.
const EXTRA_COUNT: u32 = 16_000;

/// A unique, valid resource path of exactly 1024 UTF-8 bytes (the path
/// ceiling). The decimal index is zero-padded so every path has the same
/// length and they sort in index order.
fn long_path(index: u32) -> String {
    format!("data/{index:0>1019}")
}

/// The integrity manifest lists every resource except itself, so its
/// canonical size grows with the number (and path length) of the extra
/// resources. Enough of them push the manifest past its 16 MiB
/// per-resource ceiling, and the writer must report that as a
/// per-resource byte ceiling rather than emitting an archive.
#[test]
fn an_oversized_integrity_manifest_trips_the_per_resource_ceiling() {
    let mut extras = Vec::with_capacity(EXTRA_COUNT as usize);
    for index in 0..EXTRA_COUNT {
        extras.push(ExtraResource {
            path: long_path(index),
            canonical_bytes: vec![b'x'],
        });
    }
    let config = build_config(extras);

    let err = write_archive(&config).expect_err("the integrity manifest exceeds its ceiling");
    match err {
        BuildError::SizeCeiling {
            kind,
            requested,
            ceiling,
        } => {
            assert_eq!(kind, SizeKind::ResourceBytes);
            assert_eq!(ceiling, MAX_INTEGRITY_BYTES);
            assert!(
                requested > MAX_INTEGRITY_BYTES,
                "requested {requested} must exceed the ceiling {ceiling}"
            );
        }
        other => panic!("expected a size-ceiling failure, got {other:?}"),
    }
}

/// The same build succeeds once the manifest fits under the ceiling, which
/// proves the rejection above is about the manifest size and not about the
/// resource count.
#[test]
fn a_small_extra_resource_set_still_builds() {
    let config = build_config(vec![ExtraResource {
        path: "data/notes.txt".to_string(),
        canonical_bytes: b"hello".to_vec(),
    }]);
    let bytes = write_archive(&config).expect("a small manifest fits under the ceiling");
    assert!(!bytes.is_empty());
}

// ---------------------------------------------------------------------------
// Index-entry construction: the NFC path check
// ---------------------------------------------------------------------------

/// `ResourcePath` validates the *shape* of a path but not its Unicode
/// normalization; the container's index entry is the layer that enforces
/// NFC. A decomposed path therefore survives the writer's own path
/// validation and is rejected when the index entry is built.
#[test]
fn a_non_nfc_resource_path_is_rejected_when_the_index_entry_is_built() {
    // "cafe" + U+0301 COMBINING ACUTE ACCENT — the NFD spelling of "café",
    // which is not in NFC because `e` + U+0301 composes to U+00E9.
    let decomposed = "data/caf\u{0065}\u{0301}.bin";
    let config = build_config(vec![ExtraResource {
        path: decomposed.to_string(),
        canonical_bytes: vec![b'x'],
    }]);

    let err = write_archive(&config).expect_err("a non-NFC resource path is rejected");
    assert_eq!(err.code(), "PATH_NOT_NFC", "unexpected failure: {err:?}");
}
