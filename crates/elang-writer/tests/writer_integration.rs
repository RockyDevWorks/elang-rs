//! Integration tests for the elang-writer crate (Phase 13, Issue #23).
//!
//! These tests roundtrip writer output through elang-reader's 12-stage
//! pipeline (T1315) and exercise every `BuildError` variant published by
//! the writer (T1308, T1309).
//!
//! The canonical EPF / ELD bodies below are byte-identical to the inputs
//! the reader's own `__test_support::build_canonical_archive()` uses, so a
//! successful roundtrip is a strict equivalence proof between the two
//! crates' canonical serializers.

//! Test code path: `.expect()` / `.expect_err()` / panics signal fixture
//! regressions, not untrusted-input failure. The workspace lint policy
//! denies these on production code; tests live under this allow.
#![allow(clippy::expect_used, clippy::panic)]

use elang_container::ArchiveListing;
use elang_reader::Package as ReaderPackage;
use elang_writer::{write_archive, BuildError, ExtraResource, PackageConfig, Profile};

// ---------------------------------------------------------------------------
// Canonical inputs (mirrored from elang-reader::__test_support)
// ---------------------------------------------------------------------------

const FORMAT_VERSION: &str = "0.1.0";

const EPF_BYTES: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

const ELD_BYTES: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn build_minimal_config() -> PackageConfig {
    let epf = elang_epf::parse(EPF_BYTES).expect("canonical EPF parses");
    let rep = elang_repertoire::repertoire::load().expect("pinned IPA repertoire loads");
    let doc = elang_eld::parse(ELD_BYTES, &rep).expect("canonical ELD parses");
    let eld = elang_eld::inventory::from_document(&doc, &rep).expect("ELD → inventory");
    PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile: Profile::Fast,
        format_version: FORMAT_VERSION.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Happy path: full 12-stage roundtrip through elang-reader (T1315)
// ---------------------------------------------------------------------------

/// A writer-produced archive must open cleanly through the reader's
/// stages 1–7 (binary header, index, frame layout, per-frame inspect).
#[test]
fn writer_output_opens_via_reader() {
    let config = build_minimal_config();
    let bytes = write_archive(&config).expect("writer succeeds on minimal config");
    ReaderPackage::open(&bytes).expect("reader stages 1–7 accept the writer output");
}

/// Stages 8–12 (bootstrap, integrity, EPF, resource integrity, ELD
/// semantics) must also pass on a writer-produced archive.
#[test]
fn writer_output_validates_via_reader_full_pipeline() {
    let config = build_minimal_config();
    let bytes = write_archive(&config).expect("writer succeeds on minimal config");
    let pkg = ReaderPackage::open(&bytes).expect("stages 1–7");
    let _validated = pkg.validate().expect("stages 8–12");
}

/// Every supported compression profile must pass all 12 reader stages.
#[test]
fn all_compression_profiles_roundtrip() {
    for profile in [Profile::Fast, Profile::Compact, Profile::Canonical] {
        let mut config = build_minimal_config();
        config.profile = profile;
        let bytes = write_archive(&config).expect("writer succeeds for each profile");
        let pkg = ReaderPackage::open(&bytes).expect("stages 1–7");
        let _validated = pkg.validate().expect("stages 8–12");
    }
}

/// The patch may vary while the container's major/minor prefix stays fixed.
#[test]
fn matching_format_version_with_different_patch_roundtrips() {
    let mut config = build_minimal_config();
    config.format_version = "0.1.42".to_string();
    let bytes = write_archive(&config).expect("matching major/minor succeeds");
    let pkg = ReaderPackage::open(&bytes).expect("stages 1–7");
    let _validated = pkg.validate().expect("stages 8–12");
}

/// Writer output is a structurally valid binary container — the cheap
/// `ArchiveListing::parse` (stages 1–6) must succeed before any Zstd
/// decode. Useful as a quick smoke test in CI.
#[test]
fn writer_output_passes_archive_listing_parse() {
    let config = build_minimal_config();
    let bytes = write_archive(&config).expect("writer succeeds");
    let listing = ArchiveListing::parse(&bytes).expect("ArchiveListing::parse succeeds");
    let paths: Vec<&str> = listing
        .index()
        .entries()
        .iter()
        .map(|e| e.path().as_str())
        .collect();
    // Five indexed resources: mimetype, format.version, integrity.kdl,
    // package.epf, phonemes.eld.
    assert_eq!(paths.len(), 5, "indexed resource count");
    assert!(paths.contains(&"Header/format.version"));
    assert!(paths.contains(&"package.epf"));
    assert!(paths.contains(&"Linguistics/phonemes.eld"));
    assert!(paths.contains(&"META-INF/integrity.kdl"));
}

/// Extra resources (ADR 0006) must roundtrip: the index carries them, the
/// integrity manifest hashes them, and the reader's full pipeline accepts
/// them.
#[test]
fn writer_output_with_extra_resource_roundtrips() {
    let mut config = build_minimal_config();
    config.extra_resources = vec![ExtraResource {
        path: "data/license.txt".to_string(),
        canonical_bytes: b"MIT License\n".to_vec(),
    }];
    let bytes = write_archive(&config).expect("writer succeeds with extras");
    let pkg = ReaderPackage::open(&bytes).expect("stages 1–7");
    let listing = pkg.listing();
    let paths: Vec<&str> = listing
        .index()
        .entries()
        .iter()
        .map(|e| e.path().as_str())
        .collect();
    assert!(
        paths.contains(&"data/license.txt"),
        "extra resource must be indexed: {paths:?}"
    );
    let _validated = pkg.validate().expect("stages 8–12 accept the extra");
}

// ---------------------------------------------------------------------------
// Diagnostic coverage (T1308, T1309): every BuildError variant is reachable
// ---------------------------------------------------------------------------

/// Two extras that share a canonical path → `WRITER_DUPLICATE_PATH`.
#[test]
fn duplicate_extra_path_rejected() {
    let mut config = build_minimal_config();
    config.extra_resources = vec![
        ExtraResource {
            path: "data/dup.bin".to_string(),
            canonical_bytes: b"a".to_vec(),
        },
        ExtraResource {
            path: "data/dup.bin".to_string(),
            canonical_bytes: b"b".to_vec(),
        },
    ];
    let err = write_archive(&config).expect_err("duplicate path must fail");
    assert_eq!(err.code(), "WRITER_DUPLICATE_PATH");
    match err {
        BuildError::InvalidConfig { code, message } => {
            assert_eq!(code, "WRITER_DUPLICATE_PATH");
            assert!(
                message.contains("data/dup.bin"),
                "message names the offending path: {message}"
            );
        }
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}

/// A reserved path in `extra_resources` → `WRITER_INVALID_CONFIG` (writer
/// owns the four mandatory names).
#[test]
fn reserved_path_rejected() {
    let mut config = build_minimal_config();
    config.extra_resources = vec![ExtraResource {
        path: "package.epf".to_string(),
        canonical_bytes: b"x".to_vec(),
    }];
    let err = write_archive(&config).expect_err("reserved path must fail");
    assert_eq!(err.code(), "WRITER_INVALID_CONFIG");
}

/// A `format.version` that is not `{major.minor}.{patch}` with ASCII
/// digits → `WRITER_INVALID_FORMAT_VERSION`.
#[test]
fn bad_format_version_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "not-a-version".to_string();
    let err = write_archive(&config).expect_err("bad format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// Major/minor must match the header text exactly, including no leading zeros.
#[test]
fn mismatched_format_version_prefix_rejected() {
    for version in ["1.1.0", "0.2.0", "00.1.0", "0.01.0", "00.01.0"] {
        let mut config = build_minimal_config();
        config.format_version = version.to_string();
        let err = write_archive(&config).expect_err("mismatched prefix must fail");
        assert!(matches!(
            err,
            BuildError::InvalidConfig {
                code: "WRITER_INVALID_FORMAT_VERSION",
                ..
            }
        ));
    }
}

/// An empty `format.version` is rejected (the v0.1.0 container requires
/// the prefix).
#[test]
fn empty_format_version_rejected() {
    let mut config = build_minimal_config();
    config.format_version = String::new();
    let err = write_archive(&config).expect_err("empty format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

// ---------------------------------------------------------------------------
// Additional format-version coverage for `validate_format_version` branches
// not exercised by `bad_format_version_rejected` / `empty_format_version_rejected`.
// Each case names the specific branch it covers so a future refactor that
// drops one of the rejection paths fails the matching test by name.
// ---------------------------------------------------------------------------

/// `format_version` longer than `MAX_RESOURCE_PATH_BYTES` bytes is rejected
/// (the `format_version.len() > MAX_RESOURCE_PATH_BYTES` branch).
#[test]
fn format_version_over_path_length_ceiling_rejected() {
    use elang_core::limits::MAX_RESOURCE_PATH_BYTES;
    let mut config = build_minimal_config();
    // 1024-byte ceiling + 1 = 1025 ASCII digits, still well-formed semver.
    config.format_version = "1".repeat(MAX_RESOURCE_PATH_BYTES + 1);
    let err = write_archive(&config).expect_err("over-long format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` that is not in Unicode NFC is rejected (the
/// `!is_nfc(version)` branch — an NFD-decomposed string).
#[test]
fn format_version_not_in_nfc_rejected() {
    let mut config = build_minimal_config();
    // '0' '1' '0' 'e' + U+0301 (combining acute) is NFD-decomposed,
    // NOT NFC — `is_ascii_digits` would accept every component but
    // `is_nfc` rejects the whole string. Build it from code units so
    // the source file itself stays NFC (no literal non-NFC string
    // embedded in the test source, which would be a different lint
    // failure).
    let decomposed = String::from("0.1.0e") + "\u{0301}";
    assert!(
        !elang_unicode::is_nfc(&decomposed),
        "fixture must actually be non-NFC"
    );
    config.format_version = decomposed;
    let err = write_archive(&config).expect_err("non-NFC format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` with no `.` separator is rejected (the missing-minor
/// branch — `parts.next()` returns `None` after the major).
#[test]
fn format_version_missing_minor_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0".to_string();
    let err = write_archive(&config).expect_err("format_version with no minor must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` with only one dot is rejected (the missing-patch branch).
#[test]
fn format_version_missing_patch_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0.1".to_string();
    let err = write_archive(&config).expect_err("format_version with no patch must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` with more than three dot-separated components is
/// rejected (the `parts.next().is_some()` branch).
#[test]
fn format_version_with_four_components_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0.1.0.0".to_string();
    let err = write_archive(&config).expect_err("four-component format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` with non-ASCII-digit characters is rejected (the
/// `is_ascii_digits` branch — `not-a-version` is the only pre-existing
/// case; this one confirms the empty-component edge `0.1.` also falls
/// through the same check).
#[test]
fn format_version_with_empty_patch_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0.1.".to_string();
    let err = write_archive(&config).expect_err("empty-patch format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` whose middle (minor) component is empty is rejected
/// (the slice-match `[major, minor, patch]` arm captures `0..0` because
/// `"0..0".split('.')` yields `["0", "", "0"]`). The slice match now
/// exposes an explicit "components must not be empty" message rather than
/// the previous generic "must be ASCII digits" surface.
#[test]
fn format_version_with_empty_minor_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0..0".to_string();
    let err = write_archive(&config).expect_err("empty-minor format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` whose major component is empty is rejected (the
/// `..0` shape still parses as a 3-element slice with an empty major).
#[test]
fn format_version_with_empty_major_rejected() {
    let mut config = build_minimal_config();
    config.format_version = ".1.0".to_string();
    let err = write_archive(&config).expect_err("empty-major format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

/// `format_version` whose three components are non-empty but contain
/// non-ASCII-digit characters (`"0.1.x"`) is rejected by the
/// `is_ascii_digits` check, which fires AFTER the empty-component
/// check. The previous pre-PR test `bad_format_version_rejected`
/// covered `"not-a-version"` (which the slice match now routes through
/// the missing-minor arm); this one pins the post-match digits branch.
#[test]
fn format_version_with_non_digit_component_rejected() {
    let mut config = build_minimal_config();
    config.format_version = "0.1.x".to_string();
    let err = write_archive(&config).expect_err("non-digit format_version must fail");
    assert_eq!(err.code(), "WRITER_INVALID_FORMAT_VERSION");
}

// ---------------------------------------------------------------------------
// Extra-resource surface coverage
// ---------------------------------------------------------------------------

/// An extra resource whose path violates `ResourcePath::new` rules
/// (absolute path) is rejected by the integrity-manifest build loop
/// with a registered `PATH_*` code propagated as a `BuildError::Container`.
#[test]
fn invalid_extra_path_rejected_via_resource_path() {
    let mut config = build_minimal_config();
    config.extra_resources = vec![ExtraResource {
        path: "/absolute/path.bin".to_string(),
        canonical_bytes: b"x".to_vec(),
    }];
    let err = write_archive(&config).expect_err("absolute-path extra must fail");
    // Category is `ResourcePath`, routed to `BuildError::Container` by the
    // `From<Error>` impl — the registered code surfaces unchanged.
    assert_eq!(err.code(), "PATH_ABSOLUTE");
}

/// A resource-count ceiling violation (≥ `MAX_RESOURCES + 1`) is rejected
/// before any byte is emitted, surfacing as `INDEX_COUNT_OUT_OF_RANGE` via
/// the `SizeKind::ResourceCount` arm of `BuildError::SizeCeiling`.
///
/// The check runs in O(n) over `extra_resources`, so the fixture is cheap
/// to build: 24 996 small extras + the 5 mandatory resources = 25 001,
/// one over `MAX_RESOURCES = 25_000`.
#[test]
fn resource_count_over_ceiling_rejected() {
    use elang_core::limits::MAX_RESOURCES;
    let mut config = build_minimal_config();
    // 24_996 extras + 5 mandatory = 25_001, just over the ceiling.
    let extras: usize = (MAX_RESOURCES as usize) - 5 + 1;
    config.extra_resources = (0..extras)
        .map(|i| ExtraResource {
            // Variable-width numeric suffix keeps each path unique and
            // well under the 1024-byte path ceiling.
            path: format!("data/extras/x{i:06}.bin"),
            canonical_bytes: b"x".to_vec(),
        })
        .collect();
    assert_eq!(config.extra_resources.len() + 5, MAX_RESOURCES as usize + 1);
    let err = write_archive(&config).expect_err("over-ceiling resource count must fail");
    assert_eq!(err.code(), "INDEX_COUNT_OUT_OF_RANGE");
}

/// Two extras that share a canonical path are rejected with
/// `WRITER_DUPLICATE_PATH`. The previous implementation of
/// `PackageConfig::validate_paths` used a `windows(2)` adjacent-pair scan
/// and so let non-adjacent duplicates slip through to a defensive
/// post-sort check; that scan was replaced with a `HashSet`-based full
/// pass, so this test now exercises the input-validation path directly
/// rather than the (removed) post-sort dedup helper.
#[test]
fn duplicate_extra_path_rejected_via_validate_paths() {
    let mut config = build_minimal_config();
    // Three extras, two of which share `data/a.bin`. The duplicate is
    // non-adjacent in the vector, which the previous `windows(2)`
    // implementation missed entirely.
    config.extra_resources = vec![
        ExtraResource {
            path: "data/a.bin".to_string(),
            canonical_bytes: b"a".to_vec(),
        },
        ExtraResource {
            path: "data/b.bin".to_string(),
            canonical_bytes: b"b".to_vec(),
        },
        ExtraResource {
            path: "data/a.bin".to_string(),
            canonical_bytes: b"a2".to_vec(),
        },
    ];
    let err = write_archive(&config).expect_err("duplicate path must fail");
    assert_eq!(err.code(), "WRITER_DUPLICATE_PATH");
}
