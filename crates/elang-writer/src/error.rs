//! Error type for the package writer.
//!
//! Every variant carries a registered error code from
//! `specs/registry/error-codes.md`. Container-layer failures
//! ([`BuildError::Container`], [`BuildError::Frame`], [`BuildError::Index`])
//! propagate the upstream `elang_core::Error` unchanged so a writer and a
//! reader surface the same diagnostic for the same underlying failure.
//! Writer-side pre-flight checks ([`BuildError::InvalidConfig`] and
//! [`BuildError::SizeCeiling`]) reuse the `LIMIT_*` family of codes that the
//! parser stages already use, so a single code family covers both read and
//! write paths for the same ceiling.

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
