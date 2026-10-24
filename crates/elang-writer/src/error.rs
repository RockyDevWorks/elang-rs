//! Error type for the package writer.
//!
//! Every variant carries a registered error code from
//! `specs/registry/error-codes.md`. Container-layer failures
//! ([`BuildError::Container`]) propagate the upstream `elang_core::Error`
//! unchanged so a writer and a reader surface the same diagnostic for the
//! same underlying failure. Writer-side pre-flight checks
//! ([`BuildError::InvalidConfig`] and [`BuildError::SizeCeiling`]) reuse the
//! `LIMIT_*` family of codes that the parser stages already use, so a single
//! code family covers both read and write paths for the same ceiling.

use elang_core::Error;

/// Why a write-side size ceiling was rejected.
///
/// Each variant names one of the ceilings published in
/// `elang_core::limits` and gives the writer a stable handle to format a
/// diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeKind {
    /// Per-resource uncompressed byte ceiling.
    ResourceBytes,
    /// Sum of all uncompressed bytes across resources (8 GiB).
    UncompressedAggregate,
    /// Total compressed archive byte ceiling (6 GiB).
    ArchiveBytes,
    /// Index region byte ceiling (32 MiB). Surfaces as
    /// `INDEX_SIZE_OVER_CEILING` from the container layer.
    IndexBytes,
    /// Resource count ceiling (25 000). Surfaces as
    /// `INDEX_COUNT_OUT_OF_RANGE` from the container layer.
    ResourceCount,
}

impl SizeKind {
    /// The canonical name used in diagnostic messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ResourceBytes => "resource bytes",
            Self::UncompressedAggregate => "aggregate uncompressed bytes",
            Self::ArchiveBytes => "compressed archive bytes",
            Self::IndexBytes => "index bytes",
            Self::ResourceCount => "resource count",
        }
    }

    /// The registered error code for this ceiling.
    #[must_use]
    pub const fn ceiling_code(self) -> &'static str {
        match self {
            Self::ResourceBytes => "LIMIT_RESOURCE_BYTES",
            Self::UncompressedAggregate => "LIMIT_UNCOMPRESSED_AGGREGATE",
            Self::ArchiveBytes => "LIMIT_ARCHIVE_BYTES",
            Self::IndexBytes => "INDEX_SIZE_OVER_CEILING",
            Self::ResourceCount => "INDEX_COUNT_OUT_OF_RANGE",
        }
    }
}

/// All failures the writer can produce.
#[derive(Debug)]
pub enum BuildError {
    /// Container-layer failure (header, index, footer, frame encoding).
    ///
    /// Carries the upstream `elang_core::Error` so the diagnostic code,
    /// category, and message are exactly what the reader would produce for
    /// the same condition.
    Container(Error),

    /// Canonical serialization failed (EPF, ELD, or integrity manifest).
    Canonical(Error),

    /// Integrity manifest generation failed (sha2 path errors, missing
    /// required fields, coverage check failure).
    Integrity(Error),

    /// Pre-flight input validation failed before any byte was emitted.
    ///
    /// `code` is a registered error code; `message` is a human-readable
    /// description of what the caller did wrong. Used for: duplicate paths,
    /// non-UTF-8 format version, format version that does not match
    /// `{major.minor}.{patch}` prefix, missing required resource
    /// (EPF/ELD/integrity).
    InvalidConfig {
        /// Registered error code.
        code: &'static str,
        /// Human-readable explanation.
        message: String,
    },

    /// A size ceiling would be violated by this build.
    ///
    /// The code is always a registered `LIMIT_*` code (or
    /// `INDEX_*_OVER_CEILING` for the index region). The kind identifies
    /// which ceiling and the requested/ceiling pair lets the caller format
    /// a precise message without re-deriving it.
    SizeCeiling {
        /// Which ceiling was rejected.
        kind: SizeKind,
        /// The byte/count value the build would have produced.
        requested: u64,
        /// The published ceiling (`elang_core::limits`).
        ceiling: u64,
    },
}

impl BuildError {
    /// The registered error code for this failure, or `None` if no code is
    /// attached.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Container(e) | Self::Canonical(e) | Self::Integrity(e) => e.code(),
            Self::InvalidConfig { code, .. } => code,
            Self::SizeCeiling { kind, .. } => kind.ceiling_code(),
        }
    }
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Container(e) => write!(f, "container-layer failure: {e}"),
            Self::Canonical(e) => write!(f, "canonical serialization failed: {e}"),
            Self::Integrity(e) => write!(f, "integrity generation failed: {e}"),
            Self::InvalidConfig { code, message } => write!(f, "{code}: {message}"),
            Self::SizeCeiling {
                kind,
                requested,
                ceiling,
            } => write!(
                f,
                "{}: {} (requested {requested}, ceiling {ceiling})",
                kind.ceiling_code(),
                kind.name(),
            ),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Container(e) | Self::Canonical(e) | Self::Integrity(e) => Some(e),
            _ => None,
        }
    }
}

impl From<Error> for BuildError {
    fn from(err: Error) -> Self {
        match err.category() {
            elang_core::Category::Integrity => Self::Integrity(err),
            elang_core::Category::EpfSchema | elang_core::Category::EldSchema => {
                Self::Canonical(err)
            }
            _ => Self::Container(err),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! Unit tests for the writer error surface (Phase 13, T1308/T1309).
    //!
    //! Every [`BuildError`] variant and every [`SizeKind`] value is exercised
    //! here. Integration tests in `tests/writer_integration.rs` cover the
    //! variants that surface through the public `write_archive` entry point;
    //! this module covers the variant-level behaviour (Display formatting,
    //! `source()` propagation, `code()` extraction, the `From<Error>` router,
    //! and `SizeKind`'s name/code lookups) which the integration tests never
    //! reach on their own.

    use super::*;
    use elang_core::Error;
    use std::error::Error as _;

    // ---- SizeKind ----

    #[test]
    fn size_kind_name_for_every_variant() {
        assert_eq!(SizeKind::ResourceBytes.name(), "resource bytes");
        assert_eq!(
            SizeKind::UncompressedAggregate.name(),
            "aggregate uncompressed bytes"
        );
        assert_eq!(SizeKind::ArchiveBytes.name(), "compressed archive bytes");
        assert_eq!(SizeKind::IndexBytes.name(), "index bytes");
        assert_eq!(SizeKind::ResourceCount.name(), "resource count");
    }

    #[test]
    fn size_kind_ceiling_code_for_every_variant() {
        assert_eq!(
            SizeKind::ResourceBytes.ceiling_code(),
            "LIMIT_RESOURCE_BYTES"
        );
        assert_eq!(
            SizeKind::UncompressedAggregate.ceiling_code(),
            "LIMIT_UNCOMPRESSED_AGGREGATE"
        );
        assert_eq!(SizeKind::ArchiveBytes.ceiling_code(), "LIMIT_ARCHIVE_BYTES");
        assert_eq!(
            SizeKind::IndexBytes.ceiling_code(),
            "INDEX_SIZE_OVER_CEILING"
        );
        assert_eq!(
            SizeKind::ResourceCount.ceiling_code(),
            "INDEX_COUNT_OUT_OF_RANGE"
        );
    }

    // ---- BuildError::code() ----

    #[test]
    fn code_passes_through_for_container_canonical_integrity() {
        let e = BuildError::Container(Error::new("FOO", elang_core::Category::Container, "x"));
        assert_eq!(e.code(), "FOO");
        let e = BuildError::Canonical(Error::new("BAR", elang_core::Category::EpfSchema, "x"));
        assert_eq!(e.code(), "BAR");
        let e = BuildError::Integrity(Error::new("BAZ", elang_core::Category::Integrity, "x"));
        assert_eq!(e.code(), "BAZ");
    }

    #[test]
    fn code_returns_the_attached_code_for_invalid_config() {
        let e = BuildError::InvalidConfig {
            code: "WRITER_DUPLICATE_PATH",
            message: "test".to_string(),
        };
        assert_eq!(e.code(), "WRITER_DUPLICATE_PATH");
    }

    #[test]
    fn code_returns_the_ceiling_code_for_size_ceiling() {
        let e = BuildError::SizeCeiling {
            kind: SizeKind::ResourceBytes,
            requested: 100,
            ceiling: 50,
        };
        assert_eq!(e.code(), "LIMIT_RESOURCE_BYTES");
    }

    // ---- BuildError::Display ----

    #[test]
    fn display_prefixes_container_failures() {
        // `FRAME_BAD_MAGIC` is a real registered code in `specs/registry/`,
        // so the conformance scanner accepts this fixture.
        let e = BuildError::Container(Error::new(
            "FRAME_BAD_MAGIC",
            elang_core::Category::Frame,
            "frame magic mismatch",
        ));
        let rendered = format!("{e}");
        assert!(
            rendered.contains("container-layer failure"),
            "expected container prefix: {rendered}"
        );
        assert!(
            rendered.contains("FRAME_BAD_MAGIC"),
            "should embed upstream code: {rendered}"
        );
    }

    #[test]
    fn display_prefixes_canonical_failures() {
        // Test fixture, not a real code — keep it lowercase so the
        // registry-completeness scanner does not flag it as an
        // unregistered code emission.
        let e = BuildError::Canonical(Error::new(
            "epf_canonical_fixture",
            elang_core::Category::EpfSchema,
            "bad value",
        ));
        let rendered = format!("{e}");
        assert!(
            rendered.contains("canonical serialization failed"),
            "expected canonical prefix: {rendered}"
        );
    }

    #[test]
    fn display_prefixes_integrity_failures() {
        // Test fixture, not a real code — keep it lowercase.
        let e = BuildError::Integrity(Error::new(
            "integrity_fixture",
            elang_core::Category::Integrity,
            "missing entry",
        ));
        let rendered = format!("{e}");
        assert!(
            rendered.contains("integrity generation failed"),
            "expected integrity prefix: {rendered}"
        );
    }

    #[test]
    fn display_renders_invalid_config_as_code_message() {
        let e = BuildError::InvalidConfig {
            code: "WRITER_DUPLICATE_PATH",
            message: "two extras share the path \"x\"".to_string(),
        };
        assert_eq!(
            format!("{e}"),
            "WRITER_DUPLICATE_PATH: two extras share the path \"x\""
        );
    }

    #[test]
    fn display_renders_size_ceiling_with_requested_and_ceiling() {
        let e = BuildError::SizeCeiling {
            kind: SizeKind::UncompressedAggregate,
            requested: 9_000_000_000,
            ceiling: 8_589_934_592,
        };
        let rendered = format!("{e}");
        assert!(
            rendered.contains("LIMIT_UNCOMPRESSED_AGGREGATE"),
            "expected code: {rendered}"
        );
        assert!(
            rendered.contains("aggregate uncompressed bytes"),
            "expected ceiling name: {rendered}"
        );
        assert!(
            rendered.contains("9000000000") && rendered.contains("8589934592"),
            "expected requested/ceiling numbers: {rendered}"
        );
    }

    // ---- BuildError::source() ----

    #[test]
    fn source_returns_some_for_upstream_carriers() {
        // Use a lowercase fixture code so the registry-completeness scanner
        // does not flag this as an unregistered code emission.
        let inner = Error::new("source_fixture", elang_core::Category::Container, "y");
        let e = BuildError::Container(inner);
        let s = e.source().expect("Container should expose source");
        // `source()` returns `&dyn std::error::Error`; verify by Display.
        let rendered = format!("{s}");
        assert!(
            rendered.contains("source_fixture"),
            "source should carry the inner code: {rendered}"
        );

        let e = BuildError::Canonical(Error::new("x", elang_core::Category::EpfSchema, "y"));
        assert!(e.source().is_some());

        let e = BuildError::Integrity(Error::new("x", elang_core::Category::Integrity, "y"));
        assert!(e.source().is_some());
    }

    #[test]
    fn source_returns_none_for_writer_owned_variants() {
        let e = BuildError::InvalidConfig {
            code: "WRITER_INVALID_CONFIG",
            message: "x".to_string(),
        };
        assert!(e.source().is_none());

        let e = BuildError::SizeCeiling {
            kind: SizeKind::ResourceBytes,
            requested: 1,
            ceiling: 0,
        };
        assert!(e.source().is_none());
    }

    // ---- From<Error> for BuildError ----

    #[test]
    fn from_integrity_category_routes_to_integrity_variant() {
        let err = Error::new("X", elang_core::Category::Integrity, "y");
        let be: BuildError = err.into();
        assert!(
            matches!(be, BuildError::Integrity(_)),
            "Integrity category must route to BuildError::Integrity"
        );
    }

    #[test]
    fn from_epf_and_eld_schema_categories_route_to_canonical() {
        for cat in [
            elang_core::Category::EpfSchema,
            elang_core::Category::EldSchema,
        ] {
            let err = Error::new("X", cat, "y");
            let be: BuildError = err.into();
            assert!(
                matches!(be, BuildError::Canonical(_)),
                "{cat:?} must route to BuildError::Canonical"
            );
        }
    }

    #[test]
    fn from_other_categories_route_to_container() {
        // Sample one from each non-handled category to prove the default arm.
        for cat in [
            elang_core::Category::Container,
            elang_core::Category::Index,
            elang_core::Category::Frame,
            elang_core::Category::ResourcePath,
            elang_core::Category::Unicode,
            elang_core::Category::Ipa,
            elang_core::Category::Roman,
            elang_core::Category::When,
            elang_core::Category::Limit,
            elang_core::Category::Canonical,
        ] {
            let err = Error::new("X", cat, "y");
            let be: BuildError = err.into();
            assert!(
                matches!(be, BuildError::Container(_)),
                "{cat:?} must route to BuildError::Container (default arm)"
            );
        }
    }
}
