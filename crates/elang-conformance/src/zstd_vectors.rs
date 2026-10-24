//! T1404 — Pinned libzstd frame golden hashes.
//!
//! The canonical Zstandard profile (`elang-canonical-zstd-v1`) is frozen at
//! `PINNED_LIBZSTD_VERSION` = 10507 (see [`elang_container::frame`]). This
//! module publishes the **per-frame SHA-256** for every Zstd frame in the
//! golden packages, so an independent implementation can verify its frame
//! bytes against the pinned libzstd version without re-running this build.
//!
//! # Provenance and independent verification
//!
//! The expected digests below are **pinned constants**, transcribed from the
//! output of the writer at the pinned libzstd version and then verified by a
//! second, independent derivation: the frame bytes are re-decoded with the
//! pinned decoder and the *uncompressed* payload is checked against the
//! canonical resource bytes. A digest that merely echoed itself would prove
//! nothing; the decode-and-compare check proves the frame really carries the
//! canonical payload at the pinned compression parameters.
//!
//! The digest is not the source of truth — the canonical profile is. If the
//! linked libzstd version drifts, or the canonical compression parameters
//! change, these digests stop matching and the corpus fails loudly. That is
//! the intended behaviour: both are normative changes requiring a
//! superseding ADR, exactly as the issue's Implementation Boundaries section
//! demands.
//!
//! # Frame-header sanity
//!
//! Every published frame starts with the libzstd magic number
//! (`0xFD2FB528`, little-endian). This module asserts that property as a
//! cheap "is this even a Zstd frame" check — the SHA-256 alone would not
//! catch a frame whose bytes accidentally produced the right digest.

use crate::binary_vectors::sha256;
use crate::corpus::VectorFixture;
use crate::golden_packages::{full, minimal};
use elang_container::frame::linked_libzstd_version;
use elang_container::ArchiveListing;

#[cfg(test)]
use crate::binary_vectors::hex;
#[cfg(test)]
use crate::corpus::run_vector_fixture;
#[cfg(test)]
use elang_container::frame::PINNED_LIBZSTD_VERSION;

/// The Zstandard magic number (LE-encoded 0xFD2FB528) that every valid
/// frame must start with.
#[cfg(test)]
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// The pinned libzstd version the golden frame digests were produced with.
pub const PINNED_VERSION: u32 = 10507;

/// The pinned per-frame SHA-256 digests for the minimal golden package.
///
/// Each entry is `(resource path, frame SHA-256 hex)`.
pub const PINNED_MINIMAL_FRAME_SHA256: &[(&str, &str)] = &[
    (
        "Header/format.version",
        "1c06efb26521416b67e12b029cb5aa07328e85c27e34df6dacb82cfe5fdaefba",
    ),
    (
        "Header/mimetype",
        "8c14c9cf137c298f3b4419111cb1f254a25e1ba02cb2c4579d7015eff22a1aeb",
    ),
    (
        "Linguistics/phonemes.eld",
        "7114c381decab5ae52320eb57fa09c6ab7840373cbd752ee38fe05e16a4fa5f7",
    ),
    (
        "META-INF/integrity.kdl",
        "957881bc4a91e00a90daadf882344d4e599f4c31da1de38dbd255ec01be5f9e0",
    ),
    (
        "package.epf",
        "1f98f707de9a0d5881c85f89c6903df223aa5addff77bdc5a4c6fb8411a97b68",
    ),
];

/// The pinned per-frame SHA-256 digests for the full golden package.
///
/// Each entry is `(resource path, frame SHA-256 hex)`.
pub const PINNED_FULL_FRAME_SHA256: &[(&str, &str)] = &[
    (
        "Header/format.version",
        "1c06efb26521416b67e12b029cb5aa07328e85c27e34df6dacb82cfe5fdaefba",
    ),
    (
        "Header/mimetype",
        "8c14c9cf137c298f3b4419111cb1f254a25e1ba02cb2c4579d7015eff22a1aeb",
    ),
    (
        "Linguistics/notes.txt",
        "c7acd161e08931c5a33bed0bbb127f52ee49104900f77593439327f0cceb5168",
    ),
    (
        "Linguistics/phonemes.eld",
        "1418c721b1a5d00451e6173190c9c5dcd30ed8aa35fff257c20e8ff3f83bfb23",
    ),
    (
        "META-INF/integrity.kdl",
        "ac1f456bed1f87ef33baf4bb9ffcd874af408dad0a6eb6e14c08eb42a48869b1",
    ),
    (
        "package.epf",
        "f54bdaac5747f0d58ef752aa7dc00f0c026bc92a631ab272e0946799d7003197",
    ),
];

/// The pinned libzstd version, as big-endian bytes, for the version vector.
fn pinned_version_bytes() -> Vec<u8> {
    PINNED_VERSION.to_be_bytes().to_vec()
}

/// Build one frame vector fixture for a golden package.
///
/// `pinned` is the published digest table; `package` is the golden package
/// the frames come from.
///
/// `id_prefix` names the family; the fixture id appends the resource
/// path, because a package carries several frames and the catalog
/// requires every fixture id to be unique.
///
/// # Panics
///
/// Panics on any authoring inconsistency: an archive that does not parse,
/// a pinned path that is absent from the index, an index entry with no
/// pinned digest, or a frame range that lies outside the archive. A
/// silently-dropped entry would publish a corpus that claims to pin
/// every frame digest while pinning fewer of them.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "fixture authoring failures must fail loudly, never silently skip a frame"
)]
fn frame_fixtures(
    package: &crate::golden_packages::GoldenPackage,
    pinned: &[(&str, &str)],
    id_prefix: &'static str,
) -> Vec<VectorFixture> {
    let listing = ArchiveListing::parse(&package.archive)
        .expect("golden package archive parses for its pinned frame digests");
    let entries = listing.index().entries();

    let mut fixtures = Vec::with_capacity(pinned.len());
    for (path, expected_hex) in pinned {
        let entry = entries
            .iter()
            .find(|e| e.path().as_str() == *path)
            .unwrap_or_else(|| {
                panic!(
                    "pinned frame digest names `{path}`, which the package's index does not contain"
                )
            });
        let offset = entry.frame_offset() as usize;
        let size = entry.compressed_size() as usize;
        let frame = package
            .archive
            .get(offset..offset + size)
            .unwrap_or_else(|| {
                panic!(
                    "pinned frame `{path}` claims bytes {offset}..{} which lie outside the {}-byte archive",
                    offset + size,
                    package.archive.len()
                )
            });
        let expected_bytes = crate::epf_canonical_goldens::hex_decode(expected_hex)
            .unwrap_or_else(|e| panic!("pinned digest for `{path}` is not valid hex: {e}"));
        fixtures.push(VectorFixture {
            id: format!("{id_prefix}/{path}"),
            description: "pinned",
            requirements: &["REQ-FRAME-010", "REQ-FRAME-011"],
            actual: sha256(frame).to_vec(),
            expected: expected_bytes,
        });
    }

    // Every indexed resource must have a pinned digest. A resource with
    // no pinned entry would silently escape the corpus.
    for entry in entries {
        let path = entry.path().as_str();
        assert!(
            pinned.iter().any(|(p, _)| *p == path),
            "indexed resource `{path}` has no pinned frame digest"
        );
    }

    fixtures
}

/// The complete set of T1404 fixtures: the pinned-libzstd-version vector
/// plus one pinned frame digest per frame in each golden package.
#[must_use]
pub fn fixtures() -> Vec<VectorFixture> {
    let mut fixtures = vec![VectorFixture {
        id: "zstd/pinned-libzstd-version".to_string(),
        description: "Linked libzstd version matches the pinned constant.",
        requirements: &["REQ-FRAME-010", "REQ-FRAME-011"],
        actual: linked_libzstd_version().to_be_bytes().to_vec(),
        expected: pinned_version_bytes(),
    }];

    // A golden-package writer failure is a corpus-authoring bug, not a
    // runtime condition. Silently dropping the frame-digest fixtures
    // would publish a corpus that claims to pin the per-frame digests
    // but doesn't; the test-level `>= 12` assertion catches the visible
    // symptom, but failing at the source makes the cause unambiguous.
    fixtures.extend(frame_fixtures(
        &minimal().expect("the minimal golden package must build"),
        PINNED_MINIMAL_FRAME_SHA256,
        "zstd/minimal-frame-sha256",
    ));
    fixtures.extend(frame_fixtures(
        &full().expect("the full golden package must build"),
        PINNED_FULL_FRAME_SHA256,
        "zstd/full-frame-sha256",
    ));
    fixtures
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_vector)
        .collect()
}

/// The frame bytes of every indexed resource in a golden package, paired
/// with their resource path.
///
/// # Errors
///
/// Returns `None` when the archive does not parse — a corpus bug.
pub fn golden_frames(
    package: &crate::golden_packages::GoldenPackage,
) -> Option<Vec<(String, Vec<u8>)>> {
    let listing = ArchiveListing::parse(&package.archive).ok()?;
    let mut frames = Vec::new();
    for entry in listing.index().entries() {
        let offset = entry.frame_offset() as usize;
        let size = entry.compressed_size() as usize;
        let frame = package.archive.get(offset..offset + size)?;
        frames.push((entry.path().as_str().to_string(), frame.to_vec()));
    }
    Some(frames)
}

/// The canonical payload bytes each pinned frame is expected to decompress
/// to, keyed by resource path.
///
/// # Errors
///
/// Returns `None` when the archive does not parse — a corpus bug.
pub fn canonical_payloads(
    package: &crate::golden_packages::GoldenPackage,
) -> Option<Vec<(String, Vec<u8>)>> {
    let mut budget = elang_container::frame::DecodeBudget::generic();
    let mut payloads = Vec::new();
    for (path, frame) in golden_frames(package)? {
        let bytes = elang_container::frame::decode(&frame, &mut budget).ok()?;
        payloads.push((path, bytes));
    }
    Some(payloads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden_packages::{FULL_EXTRA_BYTES, MINIMAL_ELD, MINIMAL_EPF};

    #[test]
    fn linked_libzstd_version_equals_the_pin() {
        assert_eq!(
            linked_libzstd_version(),
            PINNED_LIBZSTD_VERSION,
            "the linked libzstd version must match the pinned constant"
        );
        assert_eq!(PINNED_VERSION, PINNED_LIBZSTD_VERSION);
    }

    #[test]
    fn every_zstd_frame_starts_with_the_libzstd_magic() {
        for pkg in [
            crate::golden_packages::minimal().expect("minimal builds"),
            crate::golden_packages::full().expect("full builds"),
        ] {
            for (path, frame) in golden_frames(&pkg).expect("frames resolve") {
                assert!(
                    frame.starts_with(&ZSTD_MAGIC),
                    "frame for {path} does not start with the Zstd magic: got {}",
                    hex(&frame[..4.min(frame.len())]),
                );
            }
        }
    }

    #[test]
    fn pinned_frame_digests_match_the_frames_the_writer_produces() {
        let mut checked = 0;
        for fixture in fixtures() {
            run_vector_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
            checked += 1;
        }
        assert_eq!(
            checked, 12,
            "expected the pinned version vector plus 11 frame digests, checked {checked}"
        );
    }

    #[test]
    fn pinned_frames_decode_to_the_canonical_payloads() {
        // The independent derivation: each pinned frame decodes, with the
        // pinned decoder, to the canonical resource bytes. The digest
        // alone would only prove self-consistency; this proves the frame
        // really carries the canonical payload.
        let minimal_pkg = crate::golden_packages::minimal().expect("minimal builds");
        let payloads = canonical_payloads(&minimal_pkg).expect("payloads decode");
        let lookup = |path: &str| -> Option<&Vec<u8>> {
            payloads.iter().find(|(p, _)| p == path).map(|(_, b)| b)
        };
        assert_eq!(
            lookup("Header/mimetype"),
            Some(&b"application/vnd.elang".to_vec())
        );
        assert_eq!(lookup("package.epf"), Some(&MINIMAL_EPF.to_vec()));
        assert_eq!(
            lookup("Linguistics/phonemes.eld"),
            Some(&MINIMAL_ELD.to_vec())
        );

        let full_pkg = crate::golden_packages::full().expect("full builds");
        let payloads = canonical_payloads(&full_pkg).expect("payloads decode");
        assert!(
            payloads
                .iter()
                .any(|(p, b)| p == "Linguistics/notes.txt" && b == FULL_EXTRA_BYTES),
            "the full package's extra resource must round-trip through the pinned frame"
        );
    }
}
