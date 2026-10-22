//! Indexed-resource types (REQ-CORE-MODEL-003, REQ-CORE-MODEL-005).
//!
//! An archive index maps each resource path to where its bytes live and what
//! they must hash to. Those values come from untrusted bytes, so every field is
//! validated at construction: a [`ResourceEntry`] can only be built if its
//! offsets are consistent, its sizes fit the archive ceilings, and its path is
//! a legal [`ResourcePath`].
//!
//! This module owns the *entry* only. Parsing the serialized index, checking
//! path ordering and de-duplication, and comparing against the footer hash
//! belong to the container layer (Phase 3), which drives these constructors.

use crate::bounded::{BoundedCounter, ByteBudget};
use crate::checked::checked_range;
use crate::error::{Category, Error};
use crate::ids::ResourcePath;
use crate::limits::{MAX_GENERIC_RESOURCE_BYTES, MAX_RESOURCES, MAX_UNCOMPRESSED_AGGREGATE_BYTES};

/// Byte length of a SHA-256 digest, as a compile-time constant array length.
pub const SHA256_LEN: usize = 32;

/// Byte length of a SHA-256 digest, in bytes.
pub const SHA256_BYTES: u64 = SHA256_LEN as u64;

/// One resource as described by the archive index.
///
/// Guarantees on construction:
/// - `path` is a validated [`ResourcePath`] (NFC-checked by the caller first).
/// - `offset + compressed_size <= archive_size` (checked, never wrapped).
/// - `uncompressed_size` is within the aggregate ceiling.
/// - `digest` is exactly [`SHA256_BYTES`] long.
///
/// `compressed_size` is *not* required to be non-zero: an empty resource is
/// legal and is encoded as a zero-length frame. `offset` is allowed to equal
/// `archive_size` in that case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry {
    path: ResourcePath,
    offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    digest: [u8; SHA256_LEN],
}

impl ResourceEntry {
    /// Build an entry, validating the offset range against `archive_size`.
    ///
    /// Returns `INDEX_RANGE_OUT_OF_BOUNDS` if `offset + compressed_size`
    /// overflows or exceeds the archive length — the two failure modes an
    /// attacker controls directly.
    pub fn new(
        path: ResourcePath,
        offset: u64,
        compressed_size: u64,
        uncompressed_size: u64,
        digest: [u8; SHA256_LEN],
        archive_size: u64,
    ) -> Result<Self, Error> {
        // Reject an out-of-bounds or wrapping range. `checked_add` covers the
        // overflow case (`u64::MAX + n`) that a plain comparison would miss;
        // the `archive_size` bound catches everything below that.
        if offset
            .checked_add(compressed_size)
            .is_none_or(|end| end > archive_size)
        {
            return Err(err(
                "INDEX_RANGE_OUT_OF_BOUNDS",
                "resource byte range exceeds the archive",
            ));
        }
        if uncompressed_size > MAX_GENERIC_RESOURCE_BYTES {
            return Err(err(
                "INDEX_SIZE_OVER_CEILING",
                "uncompressed resource size exceeds the per-resource ceiling",
            ));
        }
        Ok(Self {
            path,
            offset,
            compressed_size,
            uncompressed_size,
            digest,
        })
    }

    /// The validated resource path.
    #[must_use]
    pub fn path(&self) -> &ResourcePath {
        &self.path
    }

    /// Byte offset of the resource's frame within the archive.
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Length of the compressed frame, in bytes.
    #[must_use]
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Declared length after decompression, in bytes.
    #[must_use]
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    /// Expected SHA-256 digest of the decompressed bytes.
    #[must_use]
    pub fn digest(&self) -> &[u8; SHA256_LEN] {
        &self.digest
    }

    /// Convert the byte range into an in-bounds slice range for a buffer of
    /// `archive_size` bytes, using checked arithmetic.
    ///
    /// Delegates to [`checked_range`], so this cannot wrap. Construction
    /// already guarantees success; `None` is retained so the container layer
    /// can compose with its own bounds check without re-deriving the
    /// arithmetic.
    #[must_use]
    pub fn byte_range(&self, archive_size: u64) -> Option<(usize, usize)> {
        checked_range(self.offset, self.compressed_size, archive_size)
    }

    /// Account for this resource against an aggregate uncompressed budget and
    /// return a counter primed with its own declared size.
    ///
    /// This is the single call the decode loop needs: the aggregate budget is
    /// debited first (so a bomb is rejected before any allocation), then the
    /// per-resource counter is returned to bound the stream.
    pub fn reserve(&self, aggregate: &mut ByteBudget) -> Result<BoundedCounter, Error> {
        aggregate
            .consume(self.uncompressed_size)
            .map_err(|e| Error::new(e.code(), Category::Limit, e.diagnostic.message))?;
        Ok(BoundedCounter::new(
            self.uncompressed_size,
            Some(self.uncompressed_size),
        ))
    }
}

/// The full archive index: validated entries plus the aggregate size they
/// imply.
///
/// Construction enforces the resource-count ceiling and reserves the aggregate
/// uncompressed budget, so an index that exists is an index that fits inside
/// the steering ceilings (steering §3.5).
#[derive(Debug, Clone)]
pub struct ResourceIndex {
    entries: Vec<ResourceEntry>,
    total_uncompressed: u64,
}

impl ResourceIndex {
    /// Build an index from validated entries.
    ///
    /// Rejects more than [`MAX_RESOURCES`] entries and any aggregate
    /// uncompressed total above [`MAX_UNCOMPRESSED_AGGREGATE_BYTES`].
    pub fn new(entries: Vec<ResourceEntry>) -> Result<Self, Error> {
        if entries.len() > MAX_RESOURCES as usize {
            return Err(limit_err(
                "LIMIT_RESOURCES_ARCHIVE",
                "index declares more resources than the archive ceiling",
            ));
        }
        let mut total: u64 = 0;
        for e in &entries {
            total = total.checked_add(e.uncompressed_size()).ok_or_else(|| {
                limit_err(
                    "LIMIT_UNCOMPRESSED_AGGREGATE",
                    "aggregate uncompressed size overflows",
                )
            })?;
            if total > MAX_UNCOMPRESSED_AGGREGATE_BYTES {
                return Err(limit_err(
                    "LIMIT_UNCOMPRESSED_AGGREGATE",
                    "aggregate uncompressed size exceeds the archive ceiling",
                ));
            }
        }
        Ok(Self {
            entries,
            total_uncompressed: total,
        })
    }

    /// The validated entries, in index order.
    #[must_use]
    pub fn entries(&self) -> &[ResourceEntry] {
        &self.entries
    }

    /// Number of resources in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total declared uncompressed bytes across all resources.
    #[must_use]
    pub fn total_uncompressed(&self) -> u64 {
        self.total_uncompressed
    }

    /// Borrow an entry by its path.
    #[must_use]
    pub fn get(&self, path: &ResourcePath) -> Option<&ResourceEntry> {
        self.entries.iter().find(|e| e.path() == path)
    }
}

/// Build an `INDEX`-category error.
///
/// Index-structure failures (bad range, size over the per-resource ceiling)
/// are index errors. Budget ceilings use [`limit_err`] instead, because the
/// registry assigns every `LIMIT_`-prefixed code the `LIMIT` category.
fn err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Index, msg)
}

/// Build a `LIMIT`-category error.
fn limit_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Limit, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn path(s: &str) -> ResourcePath {
        ResourcePath::new(s).unwrap()
    }

    fn digest() -> [u8; 32] {
        [7u8; 32]
    }

    fn entry(offset: u64, csize: u64, usize_: u64) -> ResourceEntry {
        ResourceEntry::new(path("a.eld"), offset, csize, usize_, digest(), 1000).unwrap()
    }

    #[test]
    fn accepts_in_bounds_entry() {
        let e = entry(10, 100, 500);
        assert_eq!(e.offset(), 10);
        assert_eq!(e.compressed_size(), 100);
        assert_eq!(e.uncompressed_size(), 500);
        assert_eq!(e.byte_range(1000), Some((10, 110)));
        assert_eq!(e.digest().len(), 32);
    }

    #[test]
    fn rejects_range_past_archive_end() {
        let e = ResourceEntry::new(path("a.eld"), 950, 100, 500, digest(), 1000);
        assert_eq!(e.unwrap_err().code(), "INDEX_RANGE_OUT_OF_BOUNDS");
    }

    #[test]
    fn rejects_offset_past_archive_end() {
        let e = ResourceEntry::new(path("a.eld"), 1001, 0, 0, digest(), 1000);
        assert_eq!(e.unwrap_err().code(), "INDEX_RANGE_OUT_OF_BOUNDS");
    }

    #[test]
    fn rejects_overflowing_range() {
        // offset + compressed_size overflows u64 rather than wrapping.
        let e = ResourceEntry::new(path("a.eld"), u64::MAX, 2, 0, digest(), u64::MAX);
        assert_eq!(e.unwrap_err().code(), "INDEX_RANGE_OUT_OF_BOUNDS");
    }

    #[test]
    fn allows_zero_length_resource_at_archive_end() {
        let e = ResourceEntry::new(path("empty.txt"), 1000, 0, 0, digest(), 1000);
        assert!(e.is_ok());
        assert_eq!(e.unwrap().byte_range(1000), Some((1000, 1000)));
    }

    #[test]
    fn rejects_uncompressed_over_per_resource_ceiling() {
        // The per-resource ceiling is the generic-resource limit (1 GiB), not
        // the archive-wide compressed ceiling (6 GiB). A 2 GiB resource is
        // under the archive ceiling but over the per-resource one, so it must
        // be rejected here.
        let over = MAX_GENERIC_RESOURCE_BYTES + 1;
        assert!(
            over < MAX_UNCOMPRESSED_AGGREGATE_BYTES,
            "this value is over the per-resource ceiling but under the aggregate one, \
             which is the whole point of the test"
        );
        let e = ResourceEntry::new(path("big.eld"), 0, 10, over, digest(), 1000);
        assert_eq!(e.unwrap_err().code(), "INDEX_SIZE_OVER_CEILING");
    }

    #[test]
    fn accepts_uncompressed_at_per_resource_ceiling() {
        let e = ResourceEntry::new(
            path("max.eld"),
            0,
            10,
            MAX_GENERIC_RESOURCE_BYTES,
            digest(),
            1000,
        );
        assert!(e.is_ok(), "the ceiling itself must be accepted");
    }

    #[test]
    fn index_rejects_too_many_resources() {
        let entries: Vec<ResourceEntry> = (0..=MAX_RESOURCES)
            .map(|i| {
                ResourceEntry::new(path(&format!("r{i}.eld")), 0, 0, 0, digest(), 1000).unwrap()
            })
            .collect();
        let idx = ResourceIndex::new(entries);
        let e = idx.unwrap_err();
        assert_eq!(e.code(), "LIMIT_RESOURCES_ARCHIVE");
        assert_eq!(e.category(), Category::Limit);
    }

    #[test]
    fn index_rejects_aggregate_over_ceiling() {
        // Each entry sits exactly at the per-resource ceiling (so the
        // per-resource check passes) and the index holds one more of them than
        // the aggregate ceiling allows. This is the only way to reach the
        // aggregate check — a single oversized entry is caught earlier.
        let per = MAX_GENERIC_RESOURCE_BYTES;
        let needed = (MAX_UNCOMPRESSED_AGGREGATE_BYTES / per) + 1;
        let entries: Vec<ResourceEntry> = (0..needed)
            .map(|i| {
                ResourceEntry::new(path(&format!("r{i}.eld")), 0, 10, per, digest(), u64::MAX)
                    .expect("each entry sits at the per-resource ceiling, so it passes")
            })
            .collect();
        let idx = ResourceIndex::new(entries);
        let e = idx.unwrap_err();
        assert_eq!(e.code(), "LIMIT_UNCOMPRESSED_AGGREGATE");
        assert_eq!(e.category(), Category::Limit);
    }

    #[test]
    fn index_accepts_exactly_at_aggregate_ceiling() {
        let per = MAX_GENERIC_RESOURCE_BYTES;
        let needed = MAX_UNCOMPRESSED_AGGREGATE_BYTES / per;
        let entries: Vec<ResourceEntry> = (0..needed)
            .map(|i| {
                ResourceEntry::new(path(&format!("r{i}.eld")), 0, 10, per, digest(), u64::MAX)
                    .unwrap()
            })
            .collect();
        let idx = ResourceIndex::new(entries);
        assert!(
            idx.is_ok(),
            "a total exactly at the aggregate ceiling must be accepted"
        );
        assert_eq!(
            idx.unwrap().total_uncompressed(),
            MAX_UNCOMPRESSED_AGGREGATE_BYTES
        );
    }

    #[test]
    fn index_accepts_within_limits_and_lookups() {
        let idx = ResourceIndex::new(vec![entry(0, 10, 100), entry(10, 10, 200)]).unwrap();
        assert_eq!(idx.len(), 2);
        assert!(!idx.is_empty());
        assert_eq!(idx.total_uncompressed(), 300);
        assert_eq!(idx.get(&path("a.eld")).unwrap().offset(), 0);
        assert!(idx.get(&path("missing.eld")).is_none());
    }

    #[test]
    fn index_accepts_exactly_at_resource_ceiling() {
        let entries: Vec<ResourceEntry> = (0..MAX_RESOURCES)
            .map(|i| {
                ResourceEntry::new(path(&format!("r{i}.eld")), 0, 0, 0, digest(), 1000).unwrap()
            })
            .collect();
        let idx = ResourceIndex::new(entries);
        assert!(idx.is_ok());
        assert_eq!(idx.unwrap().len(), MAX_RESOURCES as usize);
    }

    #[test]
    fn reserve_debits_aggregate_and_primes_counter() {
        let mut agg = ByteBudget::new(1000);
        let mut counter = entry(0, 10, 400).reserve(&mut agg).unwrap();
        assert_eq!(agg.used(), 400);
        // The counter is capped at the declared size: overproduction fails.
        assert!(counter.record(400).is_ok());
        assert!(counter.record(1).is_err());
    }

    #[test]
    fn reserve_rejects_bomb_before_allocating() {
        let mut agg = ByteBudget::new(100);
        // Declares 400 bytes against a 100-byte aggregate budget.
        let r = entry(0, 10, 400).reserve(&mut agg);
        assert!(r.is_err());
        assert_eq!(agg.used(), 0, "a rejected reserve must not debit");
    }
}
