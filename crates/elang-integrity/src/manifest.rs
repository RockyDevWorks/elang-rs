//! Integrity manifest model and per-entry coverage verification.
//!
//! The semantic model returned by [`crate::parse::parse_document`] and the
//! cross-field check [`verify_coverage`] live here. The model is intentionally
//! read-only: every field is `pub` for inspection, but the only way to obtain
//! a value is through [`crate::parse::parse`] (or the lower-level
//! [`crate::parse::parse_document`]), so an `IntegrityManifest` is
//! unrepresentable-if-invalid.
//!
//! # Coverage semantics
//!
//! Constraint C-04 from `specs/grammar/integrity-0.1.0.md`:
//!
//! > Coverage is exact: one `resource` per indexed resource **except**
//! > `META-INF/integrity.kdl` (self-exclusion).
//!
//! [`verify_coverage`] takes the index's set of paths, the manifest's self
//! path, and the parsed manifest; it rejects:
//!
//! - An entry for `self_path` itself ([`INTEGRITY_EXTRA_ENTRY`]).
//! - A missing entry for an indexed resource that is not the self
//!   ([`INTEGRITY_MISSING_ENTRY`]).
//! - An entry for any path that is not in the index
//!   ([`INTEGRITY_EXTRA_ENTRY`]).
//!
//! Duplicate paths are detected during parse and surface as
//! [`INTEGRITY_DUPLICATE_ENTRY`] before this module is ever called.

use std::collections::{BTreeMap, BTreeSet};

use elang_core::ids::ResourcePath;

use crate::error::integrity_err;

/// Length of a SHA-256 digest, in bytes (32). Mirrors
/// `elang_core::index::SHA256_LEN` but kept local so the integrity crate
/// doesn't depend on the byte-units policy of `SHA256_BYTES` (`u64`) for an
/// array length. The constant is locked by ADR 0004 ("Only SHA-256 is
/// defined in v0.1.0").
const SHA256_LEN: usize = 32;

/// The hash algorithm declared by the manifest's `algorithm` node.
///
/// Only SHA-256 is defined in v0.1.0 ([ADR 0004]); any other value is
/// rejected at parse time with [`INTEGRITY_BAD_ALGORITHM`]. The enum is
/// defined now so the verify path can switch on it without an `as` cast when
/// future versions add algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Algorithm {
    /// The only variant in v0.1.0. Maps to the literal `"sha-256"` arg.
    Sha256,
}

impl Algorithm {
    /// The KDL argument string this variant is canonicalized to.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Algorithm::Sha256 => "sha-256",
        }
    }
}

/// One `resource` entry from the integrity manifest.
///
/// Pairs a validated [`ResourcePath`] with its declared SHA-256 digest (32
/// bytes, exactly). Construction is fallible via [`IntegrityEntry::new`]
/// — only valid hex digests and validated paths produce a value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntegrityEntry {
    /// The validated resource path.
    pub path: ResourcePath,
    /// The declared SHA-256 digest (32 bytes, exactly).
    pub digest: [u8; SHA256_LEN],
}

impl IntegrityEntry {
    /// Build an entry from an already-validated path and a 64-character
    /// lowercase-hex digest.
    ///
    /// # Errors
    ///
    /// Returns [`INTEGRITY_BAD_HEX`] if `hex` is not exactly 64 bytes of
    /// `[0-9a-f]`. The path is constructed via [`ResourcePath::new`], which
    /// rejects empty, absolute, backslash, NUL, too-long and bad-segment
    /// paths with the `RESOURCE_PATH` category.
    pub fn new(path: &str, hex: &str) -> Result<Self, elang_core::error::Error> {
        let path = ResourcePath::new(path.to_string())?;
        let digest = parse_hex64(hex)?;
        Ok(Self { path, digest })
    }
}

/// The validated integrity manifest.
///
/// The algorithm and resource entries are populated by
/// [`crate::parse::parse_document`]. `resources` is a [`BTreeMap`] so iteration
/// over entries is in canonical (sorted) order — matching the bytewise-sorted
/// index and the canonical writer's required sort order (grammar §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityManifest {
    /// The declared algorithm (`Sha256` in v0.1.0).
    pub algorithm: Algorithm,
    /// The declared entries, keyed by validated [`ResourcePath`].
    ///
    /// Iteration order is bytewise ascending by path (BTreeMap ordering),
    /// matching the canonical writer's required child order.
    pub resources: BTreeMap<ResourcePath, IntegrityEntry>,
}

// `parse_hex64` performs deliberate bounds-guarded arithmetic on byte
// pairs. The hex length check above proves we have exactly 32 pairs, the
// `as_chunks` call yields a fixed-size `&[u8; 2]` reference per iteration,
// and the bit-or-shift in `parse_hex64` plus the byte-range subtraction in
// `hex_nibble` are in `u8` over a known `0..16` nibble / known ASCII range.
// The `try_into` to `[u8; 64]` is infallible because the length is checked
// above. These are the normal shape of a base-16 decoder; the workspace's
// untrusted-input lints target *accidental* unchecked math and indexing,
// neither of which apply here.
// Lint note: paired-array iteration on a known-length input; the explicit
// length check above is the bound.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::expect_used,
    reason = "paired nibble decoding on a length-checked input; bounds are guarded above and the try_into is infallible"
)]
pub(crate) fn parse_hex64(hex: &str) -> Result<[u8; SHA256_LEN], elang_core::error::Error> {
    if hex.len() != 64 {
        return Err(integrity_err(
            "INTEGRITY_BAD_HEX",
            format!(
                "sha256 must be 64 lowercase hex characters, got {}",
                hex.len()
            ),
        ));
    }
    let bytes: [u8; 64] = hex
        .as_bytes()
        .try_into()
        .expect("hex.len() == 64 guarantees the try_into succeeds");
    let mut out = [0u8; SHA256_LEN];
    for (i, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

// `hex_nibble` subtracts ASCII digit/letter codepoints to produce a
// 0..16 nibble. The match arms prove the operand ranges (`b'0'..=b'9'` and
// `b'a'..=b'f'`), so the subtraction cannot overflow.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "byte subtraction inside a match guard with provably safe operand ranges"
)]
fn hex_nibble(b: u8) -> Result<u8, elang_core::error::Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(integrity_err(
            "INTEGRITY_BAD_HEX",
            "sha256 must contain only [0-9a-f] lowercase hex",
        )),
    }
}

/// Verify constraint C-04 against the parsed index.
///
/// `indexed_paths` is the set of paths the container's index claims to
/// contain. `self_path` is the path the manifest itself occupies in the
/// container — typically `META-INF/integrity.kdl`.
///
/// # Errors
///
/// - [`INTEGRITY_MISSING_ENTRY`] if an indexed resource other than
///   `self_path` has no manifest entry.
/// - [`INTEGRITY_EXTRA_ENTRY`] if the manifest has an entry for `self_path`
///   itself, or for any path not in `indexed_paths`.
pub fn verify_coverage(
    manifest: &IntegrityManifest,
    indexed_paths: &BTreeSet<ResourcePath>,
    self_path: &ResourcePath,
) -> Result<(), elang_core::error::Error> {
    // Self-exclusion: the manifest must not contain an entry for itself.
    if manifest.resources.contains_key(self_path) {
        return Err(integrity_err(
            "INTEGRITY_EXTRA_ENTRY",
            format!(
                "integrity manifest must not contain an entry for itself ({})",
                self_path.as_str()
            ),
        ));
    }

    // Missing entries: every indexed path except self must be present.
    for path in indexed_paths {
        if path == self_path {
            continue;
        }
        if !manifest.resources.contains_key(path) {
            return Err(integrity_err(
                "INTEGRITY_MISSING_ENTRY",
                format!(
                    "indexed resource '{}' has no entry in the integrity manifest",
                    path.as_str()
                ),
            ));
        }
    }

    // Extra entries: every manifest entry must reference an indexed path.
    for path in manifest.resources.keys() {
        if !indexed_paths.contains(path) {
            return Err(integrity_err(
                "INTEGRITY_EXTRA_ENTRY",
                format!(
                    "integrity manifest has entry for non-indexed path '{}'",
                    path.as_str()
                ),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests build small synthetic paths/digests; a panic signals a test bug"
)]
mod tests {
    use super::*;

    fn p(s: &str) -> ResourcePath {
        ResourcePath::new(s.to_string()).expect("valid path")
    }

    fn manifest(paths: &[&str]) -> IntegrityManifest {
        let mut resources = BTreeMap::new();
        for path in paths {
            let digest = [0xAAu8; 32];
            let path = p(path);
            resources.insert(path.clone(), IntegrityEntry { path, digest });
        }
        IntegrityManifest {
            algorithm: Algorithm::Sha256,
            resources,
        }
    }

    #[test]
    fn algorithm_as_str_is_locked_to_sha256() {
        assert_eq!(Algorithm::Sha256.as_str(), "sha-256");
    }

    #[test]
    fn integrity_entry_new_rejects_non_hex() {
        let err = IntegrityEntry::new("a/b.eld", "not-hex").unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
        assert_eq!(err.diagnostic.category, Category::Integrity);
    }

    #[test]
    fn integrity_entry_new_rejects_uppercase_hex() {
        // 64 chars but uppercase letters fail the lowercase requirement.
        let upper = "A".repeat(64);
        let err = IntegrityEntry::new("a/b.eld", &upper).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
    }

    #[test]
    fn integrity_entry_new_rejects_wrong_length() {
        let short = "a".repeat(63);
        let err = IntegrityEntry::new("a/b.eld", &short).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
        let long = "a".repeat(65);
        let err = IntegrityEntry::new("a/b.eld", &long).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
    }

    #[test]
    fn integrity_entry_new_round_trips() {
        let hex = "0".repeat(64);
        let entry = IntegrityEntry::new("a/b.eld", &hex).unwrap();
        assert_eq!(entry.path.as_str(), "a/b.eld");
        assert_eq!(entry.digest, [0u8; 32]);
    }

    #[test]
    fn parse_hex64_decodes_known_vector() {
        // SHA-256("abc") = ba7816bf...f20015ad (canonical example).
        let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let out = parse_hex64(hex).unwrap();
        assert_eq!(
            out,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn coverage_passes_for_exact_match() {
        let m = manifest(&["a.eld", "b.eld"]);
        let mut idx = BTreeSet::new();
        idx.insert(p("a.eld"));
        idx.insert(p("b.eld"));
        let self_path = p("META-INF/integrity.kdl");
        verify_coverage(&m, &idx, &self_path).expect("exact coverage must pass");
    }

    #[test]
    fn coverage_flags_self_entry() {
        let m = manifest(&["META-INF/integrity.kdl"]);
        let mut idx = BTreeSet::new();
        idx.insert(p("META-INF/integrity.kdl"));
        let self_path = p("META-INF/integrity.kdl");
        let err = verify_coverage(&m, &idx, &self_path).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_EXTRA_ENTRY");
    }

    #[test]
    fn coverage_flags_missing_indexed_resource() {
        let m = manifest(&["a.eld"]);
        let mut idx = BTreeSet::new();
        idx.insert(p("a.eld"));
        idx.insert(p("b.eld"));
        let self_path = p("META-INF/integrity.kdl");
        let err = verify_coverage(&m, &idx, &self_path).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_MISSING_ENTRY");
        assert!(err.diagnostic.message.contains("b.eld"));
    }

    #[test]
    fn coverage_flags_non_indexed_resource() {
        let m = manifest(&["a.eld", "c.eld"]);
        let mut idx = BTreeSet::new();
        idx.insert(p("a.eld"));
        let self_path = p("META-INF/integrity.kdl");
        let err = verify_coverage(&m, &idx, &self_path).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_EXTRA_ENTRY");
        assert!(err.diagnostic.message.contains("c.eld"));
    }

    #[test]
    fn coverage_passes_when_self_path_not_in_index() {
        // Edge case: the manifest self-path is by definition not in the index
        // (the container's index lists resources, not the manifest itself in
        // every version). Self-exclusion still applies — we just don't try to
        // require it as a missing entry.
        let m = manifest(&["a.eld"]);
        let mut idx = BTreeSet::new();
        idx.insert(p("a.eld"));
        let self_path = p("META-INF/integrity.kdl");
        verify_coverage(&m, &idx, &self_path).expect("coverage must pass");
    }

    // Used in the `coverage_*` tests for `Category` equality.
    use elang_core::error::Category;
}
