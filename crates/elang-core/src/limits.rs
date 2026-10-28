//! Centralized resource and parser limits (REQ-CORE-MODEL-004, REQ-LIMIT-*).
//!
//! Every budget in the steering contract (§3.5, §10) lives here as a single
//! documented constant so parsers and readers read from one source of truth
//! rather than hard-coding magic numbers. Units are stated in each doc comment.

// --- Binary container ceilings (steering §3.5) ---------------------------

/// Maximum compressed archive size, in bytes (6 GiB).
pub const MAX_COMPRESSED_ARCHIVE_BYTES: u64 = 6 * 1024 * 1024 * 1024;
/// Recommended compressed archive size warning threshold, in bytes (2 GiB).
pub const RECOMMENDED_COMPRESSED_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum aggregate uncompressed resource bytes across the archive (8 GiB).
pub const MAX_UNCOMPRESSED_AGGREGATE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Maximum number of resources in an archive.
pub const MAX_RESOURCES: u32 = 25_000;
/// Maximum serialized index size, in bytes (32 MiB).
pub const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
/// Minimum serialized index size, in bytes (16-byte index header).
pub const MIN_INDEX_BYTES: u64 = 16;
/// Maximum Zstd window size, in bytes (128 MiB).
pub const MAX_ZSTD_WINDOW_BYTES: u64 = 128 * 1024 * 1024;

// --- Per-resource uncompressed maxima (steering §2, §3.5) ----------------

/// Max bytes for `Header/format.version` and `mimetype` resources.
pub const MAX_BOOTSTRAP_RESOURCE_BYTES: u64 = 64;
/// Max uncompressed bytes for `package.epf` (1 MiB).
pub const MAX_EPF_BYTES: u64 = 1024 * 1024;
/// Max uncompressed bytes for `META-INF/integrity.kdl` (16 MiB).
pub const MAX_INTEGRITY_BYTES: u64 = 16 * 1024 * 1024;
/// Max uncompressed bytes for the phoneme `.eld` resource (1 GiB).
pub const MAX_ELD_BYTES: u64 = 1024 * 1024 * 1024;
/// Default max uncompressed bytes for an undeclared generic resource (1 GiB).
pub const MAX_GENERIC_RESOURCE_BYTES: u64 = 1024 * 1024 * 1024;

// --- Resource path (steering §3.4) ---------------------------------------

/// Maximum resource path length, in UTF-8 bytes.
pub const MAX_RESOURCE_PATH_BYTES: usize = 1024;

// --- KDL parser budgets (steering §10) -----------------------------------

/// Maximum KDL syntactic nesting depth.
pub const MAX_KDL_NESTING_DEPTH: usize = 64;
/// Maximum encoded token size, in bytes (1 MiB).
pub const MAX_ENCODED_TOKEN_BYTES: usize = 1024 * 1024;
/// Maximum decoded string size, in bytes (1 MiB).
pub const MAX_DECODED_STRING_BYTES: usize = 1024 * 1024;
/// Maximum node/property name length, in UTF-8 bytes.
pub const MAX_NAME_BYTES: usize = 256;
/// Maximum positional arguments per node.
pub const MAX_ARGS_PER_NODE: usize = 256;
/// Maximum properties per node.
pub const MAX_PROPS_PER_NODE: usize = 256;
/// Maximum nodes per resource.
pub const MAX_NODES_PER_RESOURCE: u64 = 2_000_000;
/// Maximum nodes per archive.
pub const MAX_NODES_PER_ARCHIVE: u64 = 10_000_000;
/// Maximum size of a single comment, in bytes (1 MiB).
pub const MAX_COMMENT_BYTES: usize = 1024 * 1024;
/// Maximum total comment bytes per resource (16 MiB).
pub const MAX_COMMENT_BYTES_PER_RESOURCE: u64 = 16 * 1024 * 1024;
/// Maximum decoded string bytes per resource (256 MiB).
pub const MAX_DECODED_STRING_BYTES_PER_RESOURCE: u64 = 256 * 1024 * 1024;
/// Maximum decoded string bytes per archive (1 GiB).
pub const MAX_DECODED_STRING_BYTES_PER_ARCHIVE: u64 = 1024 * 1024 * 1024;

// --- Semantic budgets (steering §8, §10) ---------------------------------

/// Maximum semantic references per archive.
pub const MAX_SEMANTIC_REFERENCES: u64 = 10_000_000;
/// Maximum semantic traversal depth.
pub const MAX_SEMANTIC_TRAVERSAL_DEPTH: usize = 128;
/// Maximum retained diagnostics (truncation must not mask failure).
pub const MAX_RETAINED_DIAGNOSTICS: usize = 100;

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_have_expected_values() {
        assert_eq!(MAX_COMPRESSED_ARCHIVE_BYTES, 6_442_450_944);
        assert_eq!(MAX_UNCOMPRESSED_AGGREGATE_BYTES, 8_589_934_592);
        assert_eq!(MAX_RESOURCES, 25_000);
        assert_eq!(MAX_ZSTD_WINDOW_BYTES, 134_217_728);
        assert_eq!(MAX_RESOURCE_PATH_BYTES, 1024);
        assert_eq!(MAX_KDL_NESTING_DEPTH, 64);
        assert_eq!(MAX_RETAINED_DIAGNOSTICS, 100);
        // REQ-LIMIT-008 — semantic references per archive and the
        // semantic traversal depth ceiling.
        assert_eq!(MAX_SEMANTIC_REFERENCES, 10_000_000);
        assert_eq!(MAX_SEMANTIC_TRAVERSAL_DEPTH, 128);
    }

    #[test]
    fn index_range_is_consistent() {
        assert!(MIN_INDEX_BYTES < MAX_INDEX_BYTES);
        assert_eq!(MIN_INDEX_BYTES, 16);
        assert_eq!(MAX_INDEX_BYTES, 33_554_432);
    }

    /// Every multi-byte constant in this module is asserted at its literal
    /// value so a cargo-mutants `* → +` or `* → /` substitution on any
    /// factor becomes a test failure. The values mirror the doc comments
    /// above each constant (GiB / MiB / KiB conversions of the byte budgets
    /// from steering §3.5, §10, §11).
    #[test]
    fn per_resource_and_budget_ceilings_have_expected_values() {
        // Binary container ceilings (steering §3.5).
        assert_eq!(RECOMMENDED_COMPRESSED_ARCHIVE_BYTES, 2_147_483_648);
        assert_eq!(MAX_INDEX_BYTES, 33_554_432);
        // Per-resource uncompressed maxima (steering §2, §3.5).
        assert_eq!(MAX_BOOTSTRAP_RESOURCE_BYTES, 64);
        assert_eq!(MAX_EPF_BYTES, 1_048_576);
        assert_eq!(MAX_INTEGRITY_BYTES, 16_777_216);
        assert_eq!(MAX_ELD_BYTES, 1_073_741_824);
        assert_eq!(MAX_GENERIC_RESOURCE_BYTES, 1_073_741_824);
        // KDL parser budgets (steering §10).
        assert_eq!(MAX_ENCODED_TOKEN_BYTES, 1_048_576);
        assert_eq!(MAX_DECODED_STRING_BYTES, 1_048_576);
        assert_eq!(MAX_NAME_BYTES, 256);
        assert_eq!(MAX_ARGS_PER_NODE, 256);
        assert_eq!(MAX_PROPS_PER_NODE, 256);
        assert_eq!(MAX_NODES_PER_RESOURCE, 2_000_000);
        assert_eq!(MAX_NODES_PER_ARCHIVE, 10_000_000);
        assert_eq!(MAX_COMMENT_BYTES, 1_048_576);
        assert_eq!(MAX_COMMENT_BYTES_PER_RESOURCE, 16_777_216);
        assert_eq!(MAX_DECODED_STRING_BYTES_PER_RESOURCE, 268_435_456);
        assert_eq!(MAX_DECODED_STRING_BYTES_PER_ARCHIVE, 1_073_741_824);
    }
}
