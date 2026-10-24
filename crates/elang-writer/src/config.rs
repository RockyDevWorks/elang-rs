//! Builder configuration types for the package writer.
//!
//! [`PackageConfig`] is the single struct a caller fills in. It is
//! deliberately config-shaped (not fluent-builder-shaped) because the
//! payload is fixed (`EPF` + `ELD` + zero+ extra resources + a compression
//! profile + a pinned format version) and validation is atomic — every
//! required field is known up front, so a single `write_archive(&cfg)` call
//! can fail fast with a precise diagnostic before emitting any byte.

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
    /// Reject duplicate paths in `extra_resources` before the build loop
    /// catches the same problem. The writer itself owns the four mandatory
    /// paths (`Header/format.version`, `package.epf`,
    /// `Linguistics/phonemes.eld`, `META-INF/integrity.kdl`) and cannot
    /// collide with caller-supplied paths because those four names are
    /// reserved.
    pub(crate) fn validate_paths(&self) -> Result<(), BuildError> {
        use crate::error::BuildError as E;
        const RESERVED: &[&str] = &[
            "Header/mimetype",
            "Header/format.version",
            "Linguistics/phonemes.eld",
            "META-INF/integrity.kdl",
            "package.epf",
        ];
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
        }
        for pair in self.extra_resources.windows(2) {
            if let [prev, next] = pair {
                if prev.path == next.path {
                    return Err(E::InvalidConfig {
                        code: "WRITER_DUPLICATE_PATH",
                        message: format!("two extra_resources share the path {:?}", prev.path),
                    });
                }
            }
        }
        Ok(())
    }
}
