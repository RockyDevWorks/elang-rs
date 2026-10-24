//! Builder configuration types for the package writer.
//!
//! [`PackageConfig`] is the single struct a caller fills in. It is
//! deliberately config-shaped (not fluent-builder-shaped) because the
//! payload is fixed (`EPF` + `ELD` + zero+ extra resources + a compression
//! profile + a pinned format version) and validation is atomic — every
//! required field is known up front, so a single `write_archive(&cfg)` call
//! can fail fast with a precise diagnostic before emitting any byte.

use std::collections::HashSet;

use elang_container::frame::Profile;
use elang_core::model::eld::PhonemeInventory;
use elang_epf::model::EpfPackage;

use crate::error::BuildError;

/// One additional permitted resource (ADR 0006,
/// `specs/registry/resource-policy-v0.1.0.md`).
///
/// `path` is the canonical UTF-8, NFC path the archive index will carry.
/// `canonical_bytes` is the exact byte sequence the integrity manifest will
/// hash and the Zstd encoder will compress — callers must canonicalize
/// before handing the bytes to the writer (e.g. via the relevant crate's
/// `canon::to_bytes`). The writer does not re-canonicalize extras; it only
/// sorts them and writes them.
#[derive(Debug, Clone)]
pub struct ExtraResource {
    /// Canonical path (NFC, relative, ≤ `MAX_RESOURCE_PATH_BYTES`).
    pub path: String,
    /// Bytes the index/manifest/integrity will reference.
    pub canonical_bytes: Vec<u8>,
}

/// The single struct [`crate::write_archive`] consumes.
///
/// Required fields are the four mandatory resources (`package.epf`,
/// `Linguistics/phonemes.eld`, `META-INF/integrity.kdl` is computed
/// internally, and `Header/format.version` is the pinned string). The
/// writer does not invent extra required resources; the caller may add
/// zero or more [`ExtraResource`] entries under the v0.1.0 resource
/// policy (ADR 0006).
///
/// `profile` selects the Zstd compression profile; `format_version` is the
/// pinned semver-shaped string written into `Header/format.version`.
#[derive(Debug, Clone)]
pub struct PackageConfig {
    /// The validated `package.epf` body. Already parsed and schema-checked.
    pub epf: EpfPackage,

    /// The validated `Linguistics/phonemes.eld` body. Already parsed and
    /// repertoire-checked.
    pub eld: PhonemeInventory,

    /// Additional permitted resources. May be empty.
    pub extra_resources: Vec<ExtraResource>,

    /// Zstd compression profile for every frame.
    pub profile: Profile,

    /// Pinned semver-shaped string for `Header/format.version`. Must be
    /// NFC, must encode `{major.minor}.{patch}` with ASCII digits; the
    /// binary header's `{major.minor}` prefix is checked against the
    /// resource content by the reader's bootstrap stage.
    pub format_version: String,
}

impl PackageConfig {
    /// Reject reserved paths and duplicate paths in `extra_resources`
    /// before the build loop catches the same problem. The writer itself
    /// owns the five mandatory paths (`Header/mimetype`,
    /// `Header/format.version`, `Linguistics/phonemes.eld`,
    /// `META-INF/integrity.kdl`, `package.epf`) and cannot collide with
    /// caller-supplied paths because those names are reserved.
    ///
    /// Duplicate detection uses a [`HashSet`] rather than the
    /// `windows(2)` adjacent-pair scan a Vec would offer, so a duplicate
    /// path anywhere in `extra_resources` (not just back-to-back) is
    /// rejected up front. The post-sort `Index::with_entries` call in
    /// `write_archive` then becomes a single source of truth on the
    /// aggregate uniqueness invariant.
    pub(crate) fn validate_paths(&self) -> Result<(), BuildError> {
        use crate::error::BuildError as E;
        const RESERVED: &[&str] = &[
            "Header/mimetype",
            "Header/format.version",
            "Linguistics/phonemes.eld",
            "META-INF/integrity.kdl",
            "package.epf",
        ];
        let mut seen: HashSet<&str> = HashSet::with_capacity(self.extra_resources.len());
        for extra in &self.extra_resources {
            if RESERVED.contains(&extra.path.as_str()) {
                return Err(E::InvalidConfig {
                    code: "WRITER_INVALID_CONFIG",
                    message: format!(
                        "extra_resource path {:?} is reserved by the writer",
                        extra.path
                    ),
                });
            }
            if !seen.insert(extra.path.as_str()) {
                return Err(E::InvalidConfig {
                    code: "WRITER_DUPLICATE_PATH",
                    message: format!("two extra_resources share the path {:?}", extra.path),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! Unit tests for [`PackageConfig::validate_paths`].
    //!
    //! The integration test in `tests/writer_integration.rs` exercises one
    //! reserved-path case and one duplicate case end-to-end; this module
    //! covers the full reserved-path set and verifies the empty-extras
    //! happy path that integration tests never reach (because every
    //! integration fixture passes `Vec::new()` and so short-circuits at the
    //! loop entry).

    use super::*;
    use crate::error::BuildError;

    // Canonical inputs (mirrored from tests/writer_integration.rs).
    const EPF_BYTES: &[u8] =
        b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";
    const ELD_BYTES: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

    fn minimal_config(extras: Vec<ExtraResource>) -> PackageConfig {
        let epf = elang_epf::parse(EPF_BYTES).expect("canonical EPF parses");
        let rep = elang_repertoire::repertoire::load().expect("pinned IPA repertoire loads");
        let doc = elang_eld::parse(ELD_BYTES, &rep).expect("canonical ELD parses");
        let eld = elang_eld::inventory::from_document(&doc, &rep).expect("ELD → inventory");
        PackageConfig {
            epf,
            eld,
            extra_resources: extras,
            profile: Profile::Fast,
            format_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn validate_paths_accepts_no_extras() {
        let cfg = minimal_config(Vec::new());
        assert!(cfg.validate_paths().is_ok());
    }

    #[test]
    fn validate_paths_accepts_unique_non_reserved_extras() {
        let cfg = minimal_config(vec![
            ExtraResource {
                path: "a/b.bin".to_string(),
                canonical_bytes: vec![1],
            },
            ExtraResource {
                path: "c/d.bin".to_string(),
                canonical_bytes: vec![2],
            },
        ]);
        assert!(cfg.validate_paths().is_ok());
    }

    #[test]
    fn validate_paths_rejects_every_reserved_path() {
        // The writer owns five mandatory names; none of them may appear in
        // `extra_resources`. Each one must surface as
        // `WRITER_INVALID_CONFIG` with the offending path in the message.
        for reserved in [
            "Header/mimetype",
            "Header/format.version",
            "Linguistics/phonemes.eld",
            "META-INF/integrity.kdl",
            "package.epf",
        ] {
            let cfg = minimal_config(vec![ExtraResource {
                path: reserved.to_string(),
                canonical_bytes: vec![0],
            }]);
            match cfg.validate_paths() {
                Err(BuildError::InvalidConfig { code, message }) => {
                    assert_eq!(code, "WRITER_INVALID_CONFIG");
                    assert!(
                        message.contains(reserved),
                        "message should name the offending path {reserved}: {message}"
                    );
                }
                Err(other) => panic!("expected InvalidConfig for {reserved}, got {other:?}"),
                Ok(()) => panic!("expected error for reserved path {reserved}"),
            }
        }
    }

    #[test]
    fn validate_paths_rejects_duplicate_extras() {
        let cfg = minimal_config(vec![
            ExtraResource {
                path: "data/x.bin".to_string(),
                canonical_bytes: vec![1],
            },
            ExtraResource {
                path: "data/x.bin".to_string(),
                canonical_bytes: vec![2],
            },
        ]);
        match cfg.validate_paths() {
            Err(BuildError::InvalidConfig { code, message }) => {
                assert_eq!(code, "WRITER_DUPLICATE_PATH");
                assert!(
                    message.contains("data/x.bin"),
                    "message should name the duplicated path: {message}"
                );
            }
            Err(other) => panic!("expected InvalidConfig, got {other:?}"),
            Ok(()) => panic!("expected duplicate-path error"),
        }
    }
}
