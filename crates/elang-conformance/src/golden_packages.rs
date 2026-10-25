//! The valid `.elang` golden packages (T1402, T1403).
//!
//! Two reference packages are published via [`crate::golden_packages::minimal`]
//! and [`crate::golden_packages::full`]:
//!
//! * `minimal` — the smallest semantically complete package: one EPF, one
//!   ELD with one consonant and one vowel, one integrity manifest, plus the
//!   two header resources. It is the "does the format work at all" reference.
//! * `full` — a full-featured package exercising every container feature
//!   the v0.1.0 format defines: multiple groups, multiple allophones, an
//!   extra permitted resource under ADR 0006, and a non-ASCII IPA inventory.
//!
//! # Why the expectations are generated, not written
//!
//! Every byte-level value in this module — the header bytes, the index
//! bytes, the footer bytes and the SHA-256 digests — is produced by running
//! the production writer and then *reading the bytes back out of the
//! archive it produced*. Nothing is transcribed by hand, so the published
//! values cannot drift from the bytes they describe.
//!
//! The *reason* those bytes are correct is the normative specification
//! (steering §3 for the container layout, §9 for the canonical ELD profile).
//! This module is the evidence that the implementation agrees with the
//! specification; it is not the specification. An independent
//! implementation that disagrees with these bytes is not thereby wrong — it
//! is then a disagreement with the spec, which is exactly the conversation
//! a conformance corpus is supposed to enable.
//!
//! # Determinism
//!
//! The archives are byte-identical on every run, on every platform. The
//! writer sorts resources by raw UTF-8 bytes, the Zstandard encoder is
//! pinned (`PINNED_LIBZSTD_VERSION`), and the integrity manifest is a
//! deterministic function of the resource bytes. `T1418` proves this by
//! building the same logical package from shuffled source orders and
//! comparing the resulting bytes.

use elang_container::footer::FOOTER_LEN;
use elang_container::header::HEADER_LEN;
use elang_container::index::INDEX_HEADER_LEN;
use elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES;
use elang_reader::Package as ReaderPackage;
use elang_writer::{write_archive, ExtraResource, PackageConfig, Profile};

use crate::binary_vectors::{hex, sha256};

/// The pinned format version for every corpus package.
pub const FORMAT_VERSION: &str = "0.1.0";

/// The pinned mimetype resource content.
pub const MIMETYPE: &str = "application/vnd.elang";

/// The minimal package's canonical EPF body.
///
/// The EPF declares the `0.1.0` schema version, the `1.0.0` language
/// version, a URN identifier and a manifest pointing at the phoneme
/// resource.
pub const MINIMAL_EPF: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Conformance Minimal\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

/// The minimal package's canonical ELD body: one consonant, one vowel.
pub const MINIMAL_ELD: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

/// The full-featured package's canonical EPF body.
pub const FULL_EPF: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:conformance-full:1.0.0\"\n    name \"Conformance Full\"\n    description \"Full-featured reference package exercising every container feature.\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

/// The full-featured package's canonical ELD body.
///
/// Exercises: multiple groups (consonants, vowels, nasals), multiple
/// allophones (both bare and fielded), a nested `when` condition tree with
/// `group`/`or` nesting, an allophone whose romanization differs from its
/// parent's (the "inherited romanization" rule), and non-ASCII IPA
/// (combining diacritic, tie bar, tie-bar-folded pair).
pub const FULL_ELD: &str = "phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n            allophone \"p\u{02b0}\" {\n                roman \"ph\"\n            }\n        }\n        phoneme \"t\" {\n            roman \"t\"\n            allophone \"t\u{02b0}\" {\n                roman \"th\"\n            }\n        }\n        phoneme \"t\u{0361}\u{0283}\" {\n            roman \"tsh\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n        phoneme \"i\" {\n            roman \"i\"\n        }\n        phoneme \"\u{0259}\" {\n            roman \"uh\"\n        }\n    }\n    nasals {\n        phoneme \"\u{014b}\" {\n            roman \"ng\"\n        }\n    }\n}\n";

/// The extra resource bundled into the full package (ADR 0006).
///
/// A permitted extra resource proves the container carries resources beyond
/// the five mandatory ones, and that they participate in the integrity
/// manifest and the index exactly like the mandatory ones.
pub const FULL_EXTRA_PATH: &str = "Linguistics/notes.txt";

/// The extra resource's canonical bytes.
pub const FULL_EXTRA_BYTES: &[u8] = b"Reference notes for the full conformance package.\n";

/// The published byte-level and digest-level expectations for one golden
/// package.
#[derive(Debug, Clone)]
pub struct GoldenPackage {
    /// Stable fixture id.
    pub id: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Normative requirement ids this package protects.
    pub requirements: &'static [&'static str],
    /// The complete archive bytes.
    pub archive: Vec<u8>,
    /// SHA-256 of the complete archive.
    pub archive_sha256: String,
    /// The 32 header bytes.
    pub header: Vec<u8>,
    /// SHA-256 of the header bytes.
    pub header_sha256: String,
    /// The exact serialized index bytes.
    pub index: Vec<u8>,
    /// SHA-256 of the index bytes (the value the footer carries).
    pub index_sha256: String,
    /// The 64 footer bytes.
    pub footer: Vec<u8>,
    /// SHA-256 of the footer bytes.
    pub footer_sha256: String,
    /// The indexed resource paths, in index order.
    pub paths: Vec<String>,
}

impl GoldenPackage {
    /// Build the published expectations by running the writer and reading
    /// the produced bytes back out.
    ///
    /// The header / index / footer segments are located by re-parsing the
    /// archive with the production container parser, so the published
    /// segments are exactly the bytes a reader will see — not a
    /// reconstruction of what they "should" be.
    fn from_archive(
        id: &'static str,
        description: &'static str,
        requirements: &'static [&'static str],
        archive: Vec<u8>,
    ) -> GoldenPackage {
        let listing =
            elang_container::ArchiveListing::parse(&archive).expect("golden archive parses");
        let index_bytes_ref = listing.index();
        let index_offset = listing.footer().index_offset();
        let index_size = listing.footer().index_size();

        let header = archive
            .get(..HEADER_LEN)
            .expect("archive has a header")
            .to_vec();
        let index = archive
            .get(index_offset as usize..(index_offset + index_size) as usize)
            .expect("archive has an index segment")
            .to_vec();
        let footer_start = (index_offset + index_size) as usize;
        let footer = archive
            .get(footer_start..footer_start + FOOTER_LEN)
            .expect("archive has a footer")
            .to_vec();

        let paths = index_bytes_ref
            .entries()
            .iter()
            .map(|e| e.path().as_str().to_string())
            .collect();

        GoldenPackage {
            id,
            description,
            requirements,
            archive_sha256: hex(&sha256(&archive)),
            header_sha256: hex(&sha256(&header)),
            index_sha256: hex(&sha256(&index)),
            footer_sha256: hex(&sha256(&footer)),
            archive,
            header,
            index,
            footer,
            paths,
        }
    }

    /// The byte offset at which the index segment starts.
    pub fn index_offset(&self) -> usize {
        self.archive.len() - FOOTER_LEN - self.index.len()
    }
}

/// Build the minimal package's writer configuration.
///
/// # Errors
///
/// Returns a human-readable message when the canonical EPF or ELD bodies
/// fail to parse or validate. Those are corpus bugs, not runtime
/// conditions.
fn minimal_config() -> Result<PackageConfig, String> {
    let epf = elang_epf::parse(MINIMAL_EPF).map_err(|e| format!("minimal EPF: {e}"))?;
    let rep = elang_repertoire::repertoire::load().map_err(|e| format!("repertoire: {e:?}"))?;
    let doc = elang_eld::parse(MINIMAL_ELD, &rep).map_err(|e| format!("minimal ELD: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("minimal ELD inventory: {e}"))?;
    Ok(PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile: GOLDEN_PROFILE,
        format_version: FORMAT_VERSION.to_string(),
    })
}

/// Build the full-featured package's writer configuration.
///
/// # Errors
///
/// Returns a human-readable message when the canonical EPF or ELD bodies
/// fail to parse or validate.
fn full_config() -> Result<PackageConfig, String> {
    let epf = elang_epf::parse(FULL_EPF).map_err(|e| format!("full EPF: {e}"))?;
    let rep = elang_repertoire::repertoire::load().map_err(|e| format!("repertoire: {e:?}"))?;
    let doc = elang_eld::parse(FULL_ELD.as_bytes(), &rep).map_err(|e| format!("full ELD: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("full ELD inventory: {e}"))?;
    Ok(PackageConfig {
        epf,
        eld,
        extra_resources: vec![ExtraResource {
            path: FULL_EXTRA_PATH.to_string(),
            canonical_bytes: FULL_EXTRA_BYTES.to_vec(),
        }],
        profile: GOLDEN_PROFILE,
        format_version: FORMAT_VERSION.to_string(),
    })
}

/// Build the minimal golden package.
///
/// # Errors
///
/// Returns a human-readable message when the canonical inputs fail to parse
/// or the writer rejects the configuration.
pub fn minimal() -> Result<GoldenPackage, String> {
    let config = minimal_config()?;
    let archive = write_archive(&config).map_err(|e| format!("write minimal: {e}"))?;
    Ok(GoldenPackage::from_archive(
        "valid/minimal",
        "Smallest semantically complete .elang package (T1402).",
        &[
            // The five mandatory resources and their content contracts.
            "REQ-PKG-001",
            "REQ-PKG-002",
            "REQ-PKG-003",
            // Pin exact bytes and digests for the reference archive.
            "REQ-CONF-002",
            // Open / verify / validate against the 12-stage ladder.
            "REQ-READER-002",
            // The minimal EPF / ELD shape.
            "REQ-EPF-001",
            "REQ-ELD-001",
        ],
        archive,
    ))
}

/// Build the full-featured golden package.
///
/// # Errors
///
/// Returns a human-readable message when the canonical inputs fail to parse
/// or the writer rejects the configuration.
pub fn full() -> Result<GoldenPackage, String> {
    let config = full_config()?;
    let archive = write_archive(&config).map_err(|e| format!("write full: {e}"))?;
    Ok(GoldenPackage::from_archive(
        "valid/full",
        "Full-featured .elang package: multiple groups, allophones, conditions, extra resource (T1402).",
        &[
            // The minimal-package requirements all still hold.
            "REQ-PKG-001",
            "REQ-PKG-002",
            "REQ-PKG-003",
            "REQ-CONF-002",
            "REQ-READER-002",
            "REQ-EPF-001",
            "REQ-ELD-001",
            // Multiple groups in canonical order (consonants, vowels, custom).
            "REQ-CANON-001",
            // The writer built this from a validated EPF + ELD + a
            // permitted extra resource, then integrity-hashed the exact
            // final bytes.
            "REQ-WRITER-001",
            "REQ-WRITER-002",
        ],
        archive,
    ))
}

/// Both golden packages, in publication order.
///
/// # Errors
///
/// Returns a human-readable message when either package cannot be built.
pub fn all() -> Result<Vec<GoldenPackage>, String> {
    Ok(vec![minimal()?, full()?])
}

/// The Zstandard compression profile every golden package is built with.
/// The Zstandard profile the golden packages are built with.
///
/// This MUST be the canonical profile (`elang-canonical-zstd-v1`).
/// `Profile::Fast` is level 3 and explicitly *not* canonical: pinning
/// its bytes would publish a corpus that no conforming implementation
/// could reproduce, because a conforming implementation compresses
/// with the locked canonical parameters (REQ-FRAME-010/011,
/// REQ-CONF-001).
pub const GOLDEN_PROFILE: Profile = Profile::Canonical;

/// The reader's bootstrap decode budget for `Header/format.version`.
///
/// The golden packages' format version must fit inside it — the same
/// ceiling the writer enforces (T1308).
pub const GOLDEN_FORMAT_VERSION_BUDGET: u64 = MAX_BOOTSTRAP_RESOURCE_BYTES;

/// The index header length, re-exported for the corpus documentation.
pub const GOLDEN_INDEX_HEADER_LEN: usize = INDEX_HEADER_LEN;

// ----- Source-pinned digests for the golden packages ------------------
//
// These digests are the *expected* values baked into the source tree. A
// regression in the production writer that changes any header / index /
// footer byte (or the layout they sit in) trips the
// `published_digests_match_the_pinned_constants` test below.
//
// They were captured with the `capture_pinned_digests_for_provenance`
// probe, which is itself `#[ignore]`d and never part of the running
// suite. To regenerate after a deliberate writer change, run
// `cargo test -p elang-conformance --lib -- --ignored
// capture_pinned_digests_for_provenance`, copy the printed digests into
// these constants, and amend the PR.

/// SHA-256 (hex) of the minimal package's archive bytes.
pub const MINIMAL_ARCHIVE_SHA256: &str =
    "fa28ce47b5d9cf81d13fde0d716e84c8444c6167350e2d1f6c4bf78031151cf7";
/// SHA-256 (hex) of the minimal package's 32-byte binary header.
pub const MINIMAL_HEADER_SHA256: &str =
    "ee15caa934ffa9220a103c5b83276df6c4e13120d2caa51089febb0c55900bdf";
/// SHA-256 (hex) of the minimal package's index segment.
pub const MINIMAL_INDEX_SHA256: &str =
    "862c450e6177de0f8ef9e88246823e05329c21d6d160f694516a8b66995fd57a";
/// SHA-256 (hex) of the minimal package's 32-byte footer.
pub const MINIMAL_FOOTER_SHA256: &str =
    "42708594e10e1c3f48670a1954586fcca97d3167c0ef62f83e66cd86a4e49efa";

/// SHA-256 (hex) of the full package's archive bytes.
pub const FULL_ARCHIVE_SHA256: &str =
    "c81016352a7924924c7bfbcbb0e296fe9c638a756998d71c4bccd4a015897788";
/// SHA-256 (hex) of the full package's 32-byte binary header.
pub const FULL_HEADER_SHA256: &str =
    "ee15caa934ffa9220a103c5b83276df6c4e13120d2caa51089febb0c55900bdf";
/// SHA-256 (hex) of the full package's index segment.
pub const FULL_INDEX_SHA256: &str =
    "141fa2690c5966ab6e77b9061fc27c89d837adb5af8ea59da03c280f9f1514f0";
/// SHA-256 (hex) of the full package's 32-byte footer.
pub const FULL_FOOTER_SHA256: &str =
    "596aa0a7df2924260d464fd30b2e2ae1007e76fd436d53da9c97151b73f3b709";

/// Assert that a golden package opens, verifies and validates.
///
/// Used by the corpus runner to confirm the positive fixtures are accepted
/// at the [`crate::corpus::Level::Validate`] ceiling.
///
/// # Errors
///
/// Returns the reader's diagnostic when the archive does not survive the
/// full pipeline.
pub fn assert_validates(pkg: &GoldenPackage) -> Result<(), String> {
    let reader = ReaderPackage::open(&pkg.archive).map_err(|e| {
        format!(
            "{}: stages 1-7 failed: code={} stage={:?} message={}",
            pkg.id, e.diagnostic.code, e.diagnostic.stage, e.diagnostic.message
        )
    })?;
    reader.validate().map(|_| ()).map_err(|e| {
        format!(
            "{}: stages 8-12 failed: code={} stage={:?} message={}",
            pkg.id, e.diagnostic.code, e.diagnostic.stage, e.diagnostic.message
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_package_is_a_valid_archive() {
        let pkg = minimal().expect("minimal package builds");
        assert_validates(&pkg).expect("minimal package validates");
    }

    #[test]
    fn full_package_is_a_valid_archive() {
        let pkg = full().expect("full package builds");
        assert_validates(&pkg).expect("full package validates");
    }

    #[test]
    fn minimal_package_indexes_exactly_the_five_mandatory_resources() {
        let pkg = minimal().expect("minimal package builds");
        assert_eq!(
            pkg.paths,
            vec![
                "Header/format.version",
                "Header/mimetype",
                "Linguistics/phonemes.eld",
                "META-INF/integrity.kdl",
                "package.epf",
            ]
        );
    }

    #[test]
    fn full_package_indexes_the_extra_resource() {
        let pkg = full().expect("full package builds");
        assert!(pkg.paths.contains(&FULL_EXTRA_PATH.to_string()));
        assert_eq!(pkg.paths.len(), 6);
    }

    #[test]
    fn package_bytes_are_deterministic_across_builds() {
        let a = minimal().expect("builds");
        let b = minimal().expect("builds");
        assert_eq!(a.archive, b.archive, "archive bytes differ between builds");
        assert_eq!(a.archive_sha256, b.archive_sha256);
        assert_eq!(a.index_sha256, b.index_sha256);
    }

    #[test]
    fn header_index_footer_segments_are_contiguous_in_the_archive() {
        for pkg in all().expect("packages build") {
            assert_eq!(pkg.header.len(), HEADER_LEN);
            assert_eq!(pkg.footer.len(), FOOTER_LEN);
            assert!(pkg.index.len() > INDEX_HEADER_LEN);

            let index_offset = pkg.index_offset();
            assert_eq!(
                &pkg.archive[index_offset..index_offset + pkg.index.len()],
                pkg.index.as_slice(),
                "index segment is at the published offset"
            );
            let footer_offset = index_offset + pkg.index.len();
            assert_eq!(
                &pkg.archive[footer_offset..footer_offset + FOOTER_LEN],
                pkg.footer.as_slice(),
                "footer segment follows the index"
            );
            assert_eq!(
                index_offset + pkg.index.len() + FOOTER_LEN,
                pkg.archive.len(),
                "archive ends exactly after the footer"
            );
        }
    }

    #[test]
    fn published_digests_match_the_pinned_constants() {
        // Source-pinned digests (see the comment block above the
        // `MINIMAL_*_SHA256` / `FULL_*_SHA256` constants). A regression
        // in the production writer that drifts any header / index /
        // footer byte fails this test with the actual vs expected
        // digests in the failure message — no rebuilding, no
        // re-running probes.
        let min = minimal().expect("minimal package builds");
        assert_eq!(min.archive_sha256, MINIMAL_ARCHIVE_SHA256);
        assert_eq!(min.header_sha256, MINIMAL_HEADER_SHA256);
        assert_eq!(min.index_sha256, MINIMAL_INDEX_SHA256);
        assert_eq!(min.footer_sha256, MINIMAL_FOOTER_SHA256);
        assert_eq!(min.index_sha256.len(), 64);
        assert_eq!(min.archive_sha256.len(), 64);

        let full = full().expect("full package builds");
        assert_eq!(full.archive_sha256, FULL_ARCHIVE_SHA256);
        assert_eq!(full.header_sha256, FULL_HEADER_SHA256);
        assert_eq!(full.index_sha256, FULL_INDEX_SHA256);
        assert_eq!(full.footer_sha256, FULL_FOOTER_SHA256);
        assert_eq!(full.index_sha256.len(), 64);
        assert_eq!(full.archive_sha256.len(), 64);
    }

    #[test]
    #[ignore = "one-shot probe to capture pinned digests for the source-constant assertions; not part of the running suite"]
    fn capture_pinned_digests_for_provenance() {
        let min = minimal().expect("minimal package builds");
        eprintln!(
            "MIN_ARCHIVE_SHA256 = \"{}\";\nMIN_HEADER_SHA256 = \"{}\";\nMIN_INDEX_SHA256  = \"{}\";\nMIN_FOOTER_SHA256 = \"{}\";\n",
            min.archive_sha256, min.header_sha256, min.index_sha256, min.footer_sha256,
        );
        let full = full().expect("full package builds");
        eprintln!(
            "FULL_ARCHIVE_SHA256 = \"{}\";\nFULL_HEADER_SHA256 = \"{}\";\nFULL_INDEX_SHA256  = \"{}\";\nFULL_FOOTER_SHA256 = \"{}\";\n",
            full.archive_sha256, full.header_sha256, full.index_sha256, full.footer_sha256,
        );
        panic!("intentional: digests printed to stderr above; copy into source-pinned constants");
    }

    #[test]
    fn format_version_fits_the_bootstrap_budget() {
        assert!(
            FORMAT_VERSION.len() as u64 <= GOLDEN_FORMAT_VERSION_BUDGET,
            "golden format version must fit the reader's bootstrap decode budget"
        );
    }

    /// An archive that does not even open must be reported as a
    /// stage 1–7 failure, not as a generic error.
    #[test]
    fn coverage_gap_assert_validates_reports_an_archive_that_does_not_open() {
        let mut pkg = minimal().expect("minimal package builds");
        pkg.archive = vec![0u8; 16];
        let err = assert_validates(&pkg).expect_err("garbage must not open");
        assert!(err.contains("stages 1-7 failed"), "unexpected error: {err}");
    }

    /// An archive that opens and verifies but fails the semantic stage
    /// must be reported as a stage 8–12 failure.
    #[test]
    fn coverage_gap_assert_validates_reports_a_package_that_fails_validation() {
        // A stage-12 negative: the container is structurally perfect
        // and digest-correct, and only the semantic content is wrong.
        let fixture = crate::negatives::validate_negatives()
            .into_iter()
            .next()
            .expect("the corpus publishes stage-12 negatives");
        let mut pkg = minimal().expect("minimal package builds");
        pkg.archive = fixture.archive;
        let err = assert_validates(&pkg).expect_err("a stage-12 negative must not validate");
        assert!(
            err.contains("stages 8-12 failed"),
            "unexpected error: {err}"
        );
    }
}
