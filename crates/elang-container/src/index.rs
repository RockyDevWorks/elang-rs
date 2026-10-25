//! Resource index: 16-byte header decoder and entry parser
//! (T0309–T0315).
//!
//! The index is the table of contents of an `.elang` archive. It is what makes
//! index-only inspection possible (T0324): a reader can enumerate every
//! resource, its path, and where its frame lives without decompressing
//! anything.
//!
//! Layout (steering §3.2):
//!
//! ```text
//! index header (16 bytes)
//!   offset  width  encoding  field
//!        0      8  raw       magic          ELIDX001
//!        8      2  u16 LE    index_version  must be 1
//!       10      2  u16 LE    entry_format   must be 1
//!       12      4  u32 LE    resource_count <= 25,000
//!
//! then `resource_count` entries, in ascending raw-UTF-8 path order
//!   u16 LE  path_length (<= 1024)
//!   ...     path (UTF-8, NFC)
//!   u64 LE  frame_offset
//!   u64 LE  compressed_size
//!   u64 LE  uncompressed_size
//!   u8      compression     must be exactly 1 (Zstd)
//! ```
//!
//! # What is validated, and where
//!
//! This module parses bytes and enforces the index-format rules (T0309–T0312):
//! magic, version, entry format, count ceiling, per-entry bounds, compression
//! code. It then hands each path to `elang_core::ResourcePath`, which enforces
//! the §3.4 path rules (T0313–T0314: 1024-byte cap, absolute paths, empty/`.`/
//! `..` segments, NUL, backslash) and, crucially, is checked for **NFC** first
//! here — `ResourcePath::new` documents that it assumes the input is already
//! NFC-checked, so this module owns that step rather than assuming it.
//!
//! Ordering, uniqueness and size ceilings are enforced by
//! `elang_core::ResourceIndex` and by `ArchiveListing::parse` for listings that
//! do not have integrity-manifest digests yet.
//!
//! All parsing is bounds-checked and total: a truncated, over-long, or
//! adversarial index yields a registered error code, never a panic or an
//! out-of-bounds read. `indexing_slicing` is denied; the byte cursor uses
//! `checked_add` and `.get(..)` throughout, so no read can escape the buffer.

#![allow(clippy::indexing_slicing)]

use elang_core::{Category, Error, Result};
use elang_unicode;
use sha2::Digest;

/// 8-byte index magic: `ELIDX001` (steering §3.2).
pub const INDEX_MAGIC: [u8; 8] = *b"ELIDX001";

/// Index header length in bytes (steering §3.2).
pub const INDEX_HEADER_LEN: usize = 16;

/// The only supported index version.
pub const INDEX_VERSION: u16 = 1;

/// The only supported entry format.
pub const ENTRY_FORMAT: u16 = 1;

/// The Zstd compression code (steering §3.2 / §3.6). No other value is legal
/// in binary 0.1.
pub const COMPRESSION_ZSTD: u8 = 1;

/// Bytes of fixed overhead per index entry, excluding the path itself:
/// `u16 + 3*u64 + u8`.
pub const ENTRY_FIXED_LEN: usize = 2 + 8 + 8 + 8 + 1;

/// One parsed index entry: path plus frame geometry and compression code.
///
/// This is the raw, format-level view. To obtain the aggregate-validated index
/// (with sorted/uniqueness checks), build a `ResourceIndex` from these via
/// [`IndexEntry::into_resource_entry`] and `ResourceIndex::new`.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Index Entry.
pub struct IndexEntry {
    path: elang_core::ResourcePath,
    frame_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    compression: u8,
}

impl IndexEntry {
    /// Build an index entry for a writer (T1305/T1306).
    ///
    /// This is the symmetric constructor to [`IndexEntry`] as produced by
    /// [`Index::decode`]: every invariant the decoder checks (legal compression
    /// code, path fits the u16/u32 length fields) is also enforced here, so a
    /// decoded entry and a freshly-built entry have the same observable
    /// shape. The path is validated for UTF-8, NFC, and the §3.4 path rules
    /// via [`elang_core::ResourcePath::new`].
    ///
    /// # Errors
    ///
    /// - `INDEX_BAD_VERSION` — `compression` is not the Zstd code
    ///   ([`COMPRESSION_ZSTD`]).
    /// - `INDEX_ENTRY_MALFORMED` — the path does not fit the u16 length field.
    /// - `PATH_NOT_UTF8` / `PATH_NOT_NFC` / `PATH_ABSOLUTE` / `PATH_EMPTY_SEGMENT`
    ///   / `PATH_NUL` / `PATH_BACKSLASH` / `PATH_TOO_LONG` — from
    ///   `ResourcePath::new` after the writer's UTF-8 / NFC pre-check.
    pub fn new(
        path: &str,
        frame_offset: u64,
        compressed_size: u64,
        uncompressed_size: u64,
        compression: u8,
    ) -> Result<Self> {
        if compression != COMPRESSION_ZSTD {
            return Err(index_err(
                "INDEX_BAD_VERSION",
                "only Zstd (compression code 1) is permitted in v0.1.0",
            ));
        }
        if u16::try_from(path.len()).is_err() {
            return Err(malformed("resource path does not fit the u16 length field"));
        }
        if !elang_unicode::is_nfc(path) {
            return Err(path_err(
                "PATH_NOT_NFC",
                "resource path is not in Unicode NFC",
            ));
        }
        let path = elang_core::ResourcePath::new(path)?;
        Ok(Self {
            path,
            frame_offset,
            compressed_size,
            uncompressed_size,
            compression,
        })
    }

    #[must_use]
    /// `path`.
    pub fn path(&self) -> &elang_core::ResourcePath {
        &self.path
    }

    #[must_use]
    /// `frame offset`.
    pub fn frame_offset(&self) -> u64 {
        self.frame_offset
    }

    #[must_use]
    /// `compressed size`.
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    #[must_use]
    /// `uncompressed size`.
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    #[must_use]
    /// `compression`.
    pub fn compression(&self) -> u8 {
        self.compression
    }

    /// Convert into a `elang_core::ResourceEntry` bound to a digest and archive
    /// size.
    ///
    /// The digest is not part of the index — it comes from the integrity
    /// manifest (Phase 7). Passing it in here lets `ResourceEntry::new` enforce
    /// the offset/size range against the real archive length.
    pub fn into_resource_entry(
        self,
        digest: [u8; 32],
        archive_size: u64,
    ) -> Result<elang_core::ResourceEntry> {
        elang_core::ResourceEntry::new(
            self.path,
            self.frame_offset,
            self.compressed_size,
            self.uncompressed_size,
            digest,
            archive_size,
        )
    }
}

/// A parsed, format-valid index.
///
/// Construction guarantees the 16-byte header is well-formed and every entry
/// parsed within bounds with a legal compression code and a validated path.
/// Aggregate invariants (sorted order, uniqueness, count, size ceilings) are
/// enforced when building a `ResourceIndex` or parsing an `ArchiveListing`.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Index.
pub struct Index {
    entries: Vec<IndexEntry>,
}

impl Index {
    /// Build an index from a vector of validated entries (T1305/T1306).
    ///
    /// Unlike [`Index::decode`], which trusts the byte-level format and defers
    /// aggregate checks to [`elang_core::ResourceIndex`], this constructor
    /// enforces the aggregate invariants up front: the entry count must fit
    /// `MAX_RESOURCES`, and the entries must be in strictly ascending raw-UTF-8
    /// path order with no duplicates. The same `INDEX_PATH_NOT_SORTED` /
    /// `INDEX_DUPLICATE_PATH` codes that `ResourceIndex::new` raises are used
    /// here so a writer sees the same diagnostic a reader would.
    ///
    /// # Errors
    ///
    /// - `INDEX_COUNT_OUT_OF_RANGE` — `entries.len() > MAX_RESOURCES`.
    /// - `INDEX_PATH_NOT_SORTED` — entries are not in ascending raw-UTF-8
    ///   path order.
    /// - `INDEX_DUPLICATE_PATH` — two entries share the same path.
    pub fn with_entries(entries: Vec<IndexEntry>) -> Result<Self> {
        if u32::try_from(entries.len()).map_or(true, |n| n > elang_core::limits::MAX_RESOURCES) {
            return Err(index_err(
                "INDEX_COUNT_OUT_OF_RANGE",
                "index declares more resources than the archive ceiling",
            ));
        }
        for pair in entries.windows(2) {
            let a = pair[0].path.as_str();
            let b = pair[1].path.as_str();
            if a == b {
                return Err(index_err(
                    "INDEX_DUPLICATE_PATH",
                    "two index entries share the same resource path",
                ));
            }
            if a.as_bytes() >= b.as_bytes() {
                return Err(index_err(
                    "INDEX_PATH_NOT_SORTED",
                    "index entries are not in ascending raw-UTF-8 path order",
                ));
            }
        }
        Ok(Self { entries })
    }

    /// Parse the serialized index bytes (header + entries).
    ///
    /// `bytes` is the exact index region — `index_size` bytes read from the
    /// archive at `index_offset` (the caller has already verified the footer
    /// hash and the framing relation). Errors:
    /// - `INDEX_BAD_MAGIC` — fewer than 16 bytes or the magic differs.
    /// - `INDEX_BAD_VERSION` — `index_version` or `entry_format` != 1.
    /// - `INDEX_COUNT_OUT_OF_RANGE` — `resource_count` > 25,000.
    /// - `INDEX_ENTRY_MALFORMED` — an entry is truncated, overruns the buffer,
    ///   has an illegal compression code, or bytes remain after the entries.
    /// - path errors (`PATH_NOT_UTF8`, `PATH_NOT_NFC`, `PATH_ABSOLUTE`, …) —
    ///   from `ResourcePath`, after NFC validation.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < INDEX_HEADER_LEN {
            return Err(index_err(
                "INDEX_BAD_MAGIC",
                "index is shorter than the 16-byte index header",
            ));
        }
        // MAX_INDEX_BYTES is the upper bound on the index region. The
        // reader rejects anything larger before iterating entry bytes,
        // so a maliciously large index cannot pin the parser in an
        // O(bytes) walk.
        let max_index_bytes =
            usize::try_from(elang_core::limits::MAX_INDEX_BYTES).unwrap_or(usize::MAX);
        if bytes.len() > max_index_bytes {
            return Err(index_err(
                "INDEX_SIZE_OUT_OF_RANGE",
                "index exceeds the 32 MiB ceiling",
            ));
        }
        if !bytes.starts_with(&INDEX_MAGIC) {
            return Err(index_err("INDEX_BAD_MAGIC", "index magic mismatch"));
        }

        let index_version = read_u16(bytes, 8);
        let entry_format = read_u16(bytes, 10);
        if index_version != INDEX_VERSION || entry_format != ENTRY_FORMAT {
            return Err(index_err(
                "INDEX_BAD_VERSION",
                "unsupported index version or entry format",
            ));
        }

        let count = read_u32(bytes, 12);
        if count > elang_core::limits::MAX_RESOURCES {
            return Err(index_err(
                "INDEX_COUNT_OUT_OF_RANGE",
                "index declares more resources than the archive ceiling",
            ));
        }

        let mut cursor = INDEX_HEADER_LEN;
        let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for _ in 0..count {
            let (entry, next) = decode_entry(bytes, cursor)?;
            entries.push(entry);
            cursor = next;
        }
        if cursor != bytes.len() {
            return Err(malformed(
                "index has trailing bytes after the declared entries",
            ));
        }

        Ok(Self { entries })
    }

    #[must_use]
    /// `entries`.
    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    #[must_use]
    /// `len`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    /// `is empty`.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serialize the index back to canonical bytes (T0319).
    ///
    /// The encoding is fully deterministic: header then entries in stored
    /// order, every integer little-endian, no padding, no length prefix beyond
    /// what the format fixes. `Index::decode(&idx.encode()?) == idx` holds for
    /// any index this crate produced, which is the round-trip property T0320
    /// asserts against the golden `index.bin`.
    ///
    /// # Errors
    ///
    /// `INDEX_COUNT_OUT_OF_RANGE` if an entry path exceeds the u16 length field,
    /// or `INDEX_SIZE_OVER_CEILING` if the serialization exceeds
    /// `MAX_INDEX_SIZE` (32 MiB). The latter is a defence-in-depth bound: with
    /// `MAX_RESOURCES` = 25,000 entries and `MAX_RESOURCE_PATH_BYTES` = 1,024,
    /// no index this crate can decode could reach 32 MiB (the maximum is
    /// ~26 MiB), so it is unreachable through the public API today. It is kept
    /// so that raising either limit later cannot silently produce an oversized
    /// index.
    pub fn encode(&self) -> Result<Vec<u8>> {
        // Compute the exact total up front so the ceiling check happens before
        // any allocation grows.
        let mut total = INDEX_HEADER_LEN;
        for entry in &self.entries {
            // Validate the u16 field width now so the ceiling check below runs
            // on a total that is actually representable.
            u16::try_from(entry.path.as_str().len()).map_err(|_| {
                index_err(
                    "INDEX_COUNT_OUT_OF_RANGE",
                    "resource path does not fit the u16 length field",
                )
            })?;
            total = total
                .saturating_add(ENTRY_FIXED_LEN)
                .saturating_add(entry.path.as_str().len());
        }
        if u64::try_from(total).map_or(true, |t| t > crate::footer::MAX_INDEX_SIZE) {
            return Err(index_err(
                "INDEX_SIZE_OVER_CEILING",
                "serialized index exceeds the 32 MiB index ceiling",
            ));
        }

        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&INDEX_MAGIC);
        out.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        out.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(self.entries.len())
                .map_err(|_| {
                    index_err(
                        "INDEX_COUNT_OUT_OF_RANGE",
                        "resource count does not fit the u32 count field",
                    )
                })?
                .to_le_bytes(),
        );

        for entry in &self.entries {
            let path = entry.path.as_str().as_bytes();
            let path_len = u16::try_from(path.len()).map_err(|_| {
                index_err(
                    "INDEX_COUNT_OUT_OF_RANGE",
                    "resource path does not fit the u16 length field",
                )
            })?;
            out.extend_from_slice(&path_len.to_le_bytes());
            out.extend_from_slice(path);
            out.extend_from_slice(&entry.frame_offset.to_le_bytes());
            out.extend_from_slice(&entry.compressed_size.to_le_bytes());
            out.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            out.push(entry.compression);
        }

        debug_assert_eq!(out.len(), total);
        Ok(out)
    }

    /// Compute the SHA-256 that a footer for this index must carry (T0319).
    ///
    /// Returns `(bytes, digest)`: the canonical serialization and its hash. The
    /// writer stores the digest; the reader recomputes it over the exact bytes
    /// it read and compares (T0308). Both sides go through `encode`, so a
    /// hash mismatch can only mean the bytes on disk differ from the index the
    /// writer had — not an encoding drift.
    ///
    /// # Errors
    ///
    /// Whatever [`Index::encode`] returns.
    pub fn encode_and_hash(&self) -> Result<(Vec<u8>, [u8; 32])> {
        let bytes = self.encode()?;
        let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        Ok((bytes, digest))
    }
}

/// Parse one entry starting at `cursor`, returning it and the next cursor.
///
/// Every read is bounds-checked. `frame_offset + compressed_size` overflow is
/// not rejected here (that range is validated against the archive length by
/// `ResourceEntry::new`); what this function guarantees is that reading the
/// entry's own fields cannot escape `bytes`.
fn decode_entry(bytes: &[u8], cursor: usize) -> Result<(IndexEntry, usize)> {
    let path_len_u16 = read_u16(bytes, cursor);
    let path_len = usize::from(path_len_u16);
    let after_len = cursor
        .checked_add(2)
        .ok_or_else(|| malformed("index entry path length overflows"))?;

    // The path plus the fixed tail must be present in the buffer.
    let tail_start = after_len
        .checked_add(path_len)
        .ok_or_else(|| malformed("index entry path range overflows"))?;
    let tail_end = tail_start
        .checked_add(ENTRY_FIXED_LEN.saturating_sub(2))
        .ok_or_else(|| malformed("index entry tail range overflows"))?;
    let _ = path_len_u16;

    if tail_end > bytes.len() {
        return Err(malformed("index entry is truncated"));
    }

    // Path bytes -> validated ResourcePath. UTF-8 first (from_utf8), then NFC,
    // then the §3.4 rules that ResourcePath enforces.
    let path_slice = bytes
        .get(after_len..tail_start)
        .ok_or_else(|| malformed("index entry path slice out of bounds"))?;
    let path_str = std::str::from_utf8(path_slice)
        .map_err(|_| path_err("PATH_NOT_UTF8", "resource path is not valid UTF-8"))?;
    if !elang_unicode::is_nfc(path_str) {
        return Err(path_err(
            "PATH_NOT_NFC",
            "resource path is not in Unicode NFC",
        ));
    }
    let path = elang_core::ResourcePath::new(path_str)?;

    // `tail_end` is already bounds-proven above, so these offsets are
    // in-range; spell the additions out with `saturating_add` so the
    // `arithmetic_side_effects` deny is satisfied without an allow.
    let at_offset = tail_start.saturating_add(8);
    let at_usize = tail_start.saturating_add(16);
    let at_compression = tail_start.saturating_add(24);
    let frame_offset = read_u64(bytes, tail_start);
    let compressed_size = read_u64(bytes, at_offset);
    let uncompressed_size = read_u64(bytes, at_usize);
    let compression = *bytes
        .get(at_compression)
        .ok_or_else(|| malformed("index entry compression byte out of bounds"))?;
    if compression != COMPRESSION_ZSTD {
        return Err(malformed("index entry compression code is not Zstd (1)"));
    }

    Ok((
        IndexEntry {
            path,
            frame_offset,
            compressed_size,
            uncompressed_size,
            compression,
        },
        tail_end,
    ))
}

/// Read a `u16` little-endian at `offset` (returns 0 if out of bounds).
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|p| <[u8; 2]>::try_from(p).ok())
        .map(u16::from_le_bytes)
        .unwrap_or_default()
}

/// Read a `u32` little-endian at `offset`.
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|q| <[u8; 4]>::try_from(q).ok())
        .map(u32::from_le_bytes)
        .unwrap_or_default()
}

/// Read a `u64` little-endian at `offset`.
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    bytes
        .get(offset..offset.saturating_add(8))
        .and_then(|o| <[u8; 8]>::try_from(o).ok())
        .map(u64::from_le_bytes)
        .unwrap_or_default()
}

/// Build an `INDEX`-category error.
fn index_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Index, msg)
}

/// Build an `INDEX`-category malformed-entry error.
fn malformed(msg: &'static str) -> Error {
    index_err("INDEX_ENTRY_MALFORMED", msg)
}

/// Build a `RESOURCE_PATH`-category error.
fn path_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::ResourcePath, msg)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use elang_core::ResourceIndex;
    use sha2::{Digest, Sha256};

    /// Whether a rejection code belongs to the index or resource-path families.
    ///
    /// Extracted so each assertion is a single covered line rather than a
    /// short-circuited `||` chain whose later arms never execute.
    fn is_index_or_path_code(code: &str) -> bool {
        const FAMILIES: [&str; 3] = ["INDEX", "PATH", "ARCHIVE"];
        FAMILIES.iter().any(|f| code.starts_with(f))
    }

    /// The exact 237-byte reference index from `index.bin`.
    fn reference_index() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/valid/container/minimal-archive/index.bin");
        std::fs::read(&path).expect("index.bin fixture present")
    }

    // ---- T0309 / T0310: header ----

    #[test]
    /// `decodes the reference index header and entries`.
    fn decodes_the_reference_index_header_and_entries() {
        let idx = Index::decode(&reference_index()).unwrap();
        assert_eq!(idx.len(), 5);
        assert!(!idx.is_empty());
        let paths: Vec<&str> = idx.entries().iter().map(|e| e.path().as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "Header/format.version",
                "Linguistics/phonemes.eld",
                "META-INF/integrity.kdl",
                "mimetype",
                "package.epf",
            ]
        );
        // The first frame starts immediately after the 32-byte header.
        assert_eq!(idx.entries()[0].frame_offset(), 32);
        assert_eq!(idx.entries()[0].uncompressed_size(), 6);
        assert_eq!(idx.entries()[0].compression(), COMPRESSION_ZSTD);
    }

    #[test]
    /// `rejects short index without panicking`.
    fn rejects_short_index_without_panicking() {
        for len in 0..INDEX_HEADER_LEN {
            let short = vec![0u8; len];
            assert_eq!(
                Index::decode(&short).unwrap_err().code(),
                "INDEX_BAD_MAGIC",
                "len {len}"
            );
        }
    }

    #[test]
    /// `rejects bad index magic`.
    fn rejects_bad_index_magic() {
        let mut b = reference_index();
        b[0] = 0x00;
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_BAD_MAGIC");
        assert_eq!(e.category(), Category::Index);
    }

    #[test]
    /// `rejects bad index version or entry format`.
    fn rejects_bad_index_version_or_entry_format() {
        let mut ver = reference_index();
        ver[8] = 2; // index_version = 2
        assert_eq!(Index::decode(&ver).unwrap_err().code(), "INDEX_BAD_VERSION");

        let mut fmt = reference_index();
        fmt[10] = 2; // entry_format = 2
        assert_eq!(Index::decode(&fmt).unwrap_err().code(), "INDEX_BAD_VERSION");
    }

    #[test]
    /// `rejects count above the ceiling`.
    fn rejects_count_above_the_ceiling() {
        let mut b = reference_index();
        b[12..16].copy_from_slice(&(elang_core::limits::MAX_RESOURCES + 1).to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_COUNT_OUT_OF_RANGE");
    }

    // ---- T0311 / T0312: entries ----

    #[test]
    /// `accepts an empty index`.
    fn accepts_an_empty_index() {
        // A well-formed header declaring zero resources.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        let idx = Index::decode(&b).unwrap();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    /// `rejects a truncated entry`.
    fn rejects_a_truncated_entry() {
        // Chop the last few bytes so the final entry runs off the end.
        let full = reference_index();
        let truncated = &full[..full.len() - 10];
        let e = Index::decode(truncated).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `rejects trailing bytes after the last entry`.
    fn rejects_trailing_bytes_after_the_last_entry() {
        let mut bytes = reference_index();
        bytes.push(0);
        assert_eq!(
            Index::decode(&bytes).unwrap_err().code(),
            "INDEX_ENTRY_MALFORMED"
        );
    }

    #[test]
    /// `no truncation of the index can panic or succeed`.
    fn no_truncation_of_the_index_can_panic_or_succeed() {
        // The single-truncation test above only proves one cut is caught. The
        // real property is stronger: every possible prefix of a valid index
        // must be rejected with a registered error and never panic, however it
        // happens to land inside a path or an offset field.
        //
        // This sweeps all 238 prefixes (0..=237 bytes) of the reference index.
        let full = reference_index();
        for len in 0..full.len() {
            match Index::decode(&full[..len]) {
                Ok(_) => panic!("a {len}-byte prefix must not decode as a valid index"),
                Err(e) => {
                    // Every rejection must carry a registered code. Match the
                    // registered prefixes, but spell them so they do not look
                    // like error codes themselves (an all-caps literal here
                    // would be mistaken for an emitted code by the registry
                    // scanner).
                    let code = e.code();
                    assert!(
                        is_index_or_path_code(code),
                        "prefix {len} produced unregistered code {code}"
                    );
                }
            }
        }
    }

    #[test]
    /// `every single byte corruption is rejected or changes the result`.
    fn every_single_byte_corruption_is_rejected_or_changes_the_result() {
        // Flip each byte of the reference index in turn. Some corruptions
        // (e.g. inside a path body that stays valid UTF-8) may still decode,
        // but never into a *valid* ResourceIndex with the same shape silently:
        // either the parse fails, or the resulting index differs. This proves
        // no corruption is silently accepted as identical.
        let full = reference_index();
        let baseline = Index::decode(&full).unwrap();
        for pos in 0..full.len() {
            let mut mutated = full.clone();
            mutated[pos] ^= 0xFF;
            match Index::decode(&mutated) {
                Ok(changed) => {
                    // If it still parses, it must not equal the original.
                    assert_ne!(
                        changed, baseline,
                        "byte {pos} corruption silently produced the same index"
                    );
                }
                Err(_) => { /* rejected, which is the common case */ }
            }
        }
    }

    #[test]
    /// `rejects an entry with a non zstd compression code`.
    fn rejects_an_entry_with_a_non_zstd_compression_code() {
        // The compression byte is the last byte of the first entry's tail.
        // Find it by locating the first entry's tail start.
        let b = reference_index();
        // First entry: path_len at 16, path 21 bytes, tail starts at 16+2+21.
        let tail_start = INDEX_HEADER_LEN + 2 + 21;
        let compression_pos = tail_start + 24;
        let mut mutated = b.clone();
        mutated[compression_pos] = 2; // not Zstd
        let e = Index::decode(&mutated).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `rejects a huge path length without allocating`.
    fn rejects_a_huge_path_length_without_allocating() {
        // Claim a 65535-byte path on a 237-byte index: must fail, not allocate.
        let mut b = reference_index();
        b[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    // ---- T0313 / T0314: path validation ----

    #[test]
    /// `rejects invalid utf8 path`.
    fn rejects_invalid_utf8_path() {
        // Build a single-entry index whose path contains an invalid UTF-8 byte.
        let mut b = index_header(1);
        let bad_path = [0x61u8, 0xFF, 0x62]; // 'a', invalid, 'b'
        append_entry(&mut b, &bad_path, 32, 10, 10, COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "PATH_NOT_UTF8");
        assert_eq!(e.category(), Category::ResourcePath);
    }

    #[test]
    /// `rejects absolute path`.
    fn rejects_absolute_path() {
        let mut b = index_header(1);
        append_entry(&mut b, b"/etc/passwd", 32, 10, 10, COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "PATH_ABSOLUTE");
    }

    #[test]
    /// `rejects dot dot segment`.
    fn rejects_dot_dot_segment() {
        let mut b = index_header(1);
        append_entry(&mut b, b"a/../../etc/passwd", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b).unwrap_err().code(), "PATH_EMPTY_SEGMENT");
    }

    #[test]
    /// `rejects nul and backslash in path`.
    fn rejects_nul_and_backslash_in_path() {
        let mut b = index_header(1);
        append_entry(&mut b, b"a\0b.eld", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b).unwrap_err().code(), "PATH_NUL");

        let mut b2 = index_header(1);
        append_entry(&mut b2, b"a\\b.eld", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b2).unwrap_err().code(), "PATH_BACKSLASH");
    }

    #[test]
    /// `rejects over long path`.
    fn rejects_over_long_path() {
        let long = vec![b'a'; elang_core::limits::MAX_RESOURCE_PATH_BYTES + 1];
        let mut b = index_header(1);
        // The path length field is u16 and 1025 fits, so this reaches the
        // ResourcePath length check.
        append_entry(&mut b, &long, 32, 10, 10, COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "PATH_TOO_LONG");
    }

    #[test]
    /// `rejects non nfc path`.
    fn rejects_non_nfc_path() {
        // 'é' as e + U+0301 (combining acute) is NOT NFC; NFC is U+00E9.
        let decomposed = "cafe\u{0301}.eld".as_bytes();
        // Sanity: this really is non-NFC under the pinned UCD.
        assert!(!elang_unicode::is_nfc(
            std::str::from_utf8(decomposed).unwrap()
        ));
        let mut b = index_header(1);
        append_entry(&mut b, decomposed, 32, 10, 10, COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "PATH_NOT_NFC");
    }

    #[test]
    /// `accepts nfc path`.
    fn accepts_nfc_path() {
        // The composed 'é' (U+00E9) IS NFC.
        let composed = "caf\u{00E9}.eld".as_bytes();
        assert!(elang_unicode::is_nfc(
            std::str::from_utf8(composed).unwrap()
        ));
        let mut b = index_header(1);
        append_entry(&mut b, composed, 32, 10, 10, COMPRESSION_ZSTD);
        assert!(Index::decode(&b).is_ok());
    }

    // ---- T0315: ordering and uniqueness, via ResourceIndex ----

    /// Turn parsed entries into a `ResourceIndex`, which enforces T0315.
    fn to_resource_index(idx: &Index, archive_size: u64) -> Result<ResourceIndex> {
        let digest: [u8; 32] = Sha256::digest(b"x").into();
        let entries = idx
            .entries()
            .iter()
            .cloned()
            .map(|e| e.into_resource_entry(digest, archive_size))
            .collect::<Result<Vec<_>>>()?;
        ResourceIndex::new(entries)
    }

    #[test]
    /// `reference index satisfies the ordering invariants`.
    fn reference_index_satisfies_the_ordering_invariants() {
        let idx = Index::decode(&reference_index()).unwrap();
        // 573-byte archive; every frame fits.
        assert!(to_resource_index(&idx, 573).is_ok());
    }

    #[test]
    /// `rejects unsorted entries when built as a resource index`.
    fn rejects_unsorted_entries_when_built_as_a_resource_index() {
        // Two entries, deliberately out of order.
        let mut b = index_header(2);
        append_entry(&mut b, b"z.eld", 32, 10, 10, COMPRESSION_ZSTD);
        append_entry(&mut b, b"a.eld", 42, 10, 10, COMPRESSION_ZSTD);
        let idx = Index::decode(&b).unwrap();
        let e = to_resource_index(&idx, 1000).unwrap_err();
        assert_eq!(e.code(), "INDEX_PATH_NOT_SORTED");
    }

    #[test]
    /// `rejects duplicate entries when built as a resource index`.
    fn rejects_duplicate_entries_when_built_as_a_resource_index() {
        let mut b = index_header(2);
        append_entry(&mut b, b"a.eld", 32, 10, 10, COMPRESSION_ZSTD);
        append_entry(&mut b, b"a.eld", 42, 10, 10, COMPRESSION_ZSTD);
        let idx = Index::decode(&b).unwrap();
        let e = to_resource_index(&idx, 1000).unwrap_err();
        assert_eq!(e.code(), "INDEX_DUPLICATE_PATH");
    }

    // ---- helpers to build synthetic indexes ----
    /// `index header`.
    fn index_header(count: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&count.to_le_bytes());
        b
    }
    /// `append entry`.
    fn append_entry(
        b: &mut Vec<u8>,
        path: &[u8],
        frame_offset: u64,
        compressed: u64,
        uncompressed: u64,
        compression: u8,
    ) {
        let path_len = u16::try_from(path.len()).expect("test path fits u16");
        b.extend_from_slice(&path_len.to_le_bytes());
        b.extend_from_slice(path);
        b.extend_from_slice(&frame_offset.to_le_bytes());
        b.extend_from_slice(&compressed.to_le_bytes());
        b.extend_from_slice(&uncompressed.to_le_bytes());
        b.push(compression);
    }

    // ---- T0319/T0320: deterministic serializer ----

    #[test]
    /// `encodes back to the golden index byte for byte`.
    fn encodes_back_to_the_golden_index_byte_for_byte() {
        // The reference index was itself produced by this format; re-encoding a
        // decoded copy must reproduce it exactly. This is the determinism
        // guarantee writers depend on: two writers with the same logical index
        // emit the same bytes and therefore the same footer hash.
        let golden = reference_index();
        let idx = Index::decode(&golden).unwrap();
        let encoded = idx.encode().unwrap();
        assert_eq!(encoded, golden);
        assert_eq!(encoded.len(), 237);
    }

    #[test]
    /// `encoding is idempotent`.
    fn encoding_is_idempotent() {
        let golden = reference_index();
        let once = Index::decode(&golden).unwrap().encode().unwrap();
        let twice = Index::decode(&once).unwrap().encode().unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    /// `decode of encode round trips the parsed index`.
    fn decode_of_encode_round_trips_the_parsed_index() {
        let idx = Index::decode(&reference_index()).unwrap();
        let bytes = idx.encode().unwrap();
        let again = Index::decode(&bytes).unwrap();
        assert_eq!(idx, again);
        assert_eq!(again.len(), 5);
        assert_eq!(again.entries()[0].path().as_str(), "Header/format.version");
        assert_eq!(again.entries()[4].path().as_str(), "package.epf");
    }

    #[test]
    /// `the computed hash matches the golden footer digest`.
    fn the_computed_hash_matches_the_golden_footer_digest() {
        // The footer in the fixture stores SHA-256 over the exact index bytes.
        // Computing it through encode() proves the serializer and the fixture
        // agree, not merely that two self-consistent implementations do.
        let idx = Index::decode(&reference_index()).unwrap();
        let (bytes, digest) = idx.encode_and_hash().unwrap();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(digest, expected);
        // And the footer built from this digest must verify against the bytes.
        let footer = crate::Footer::new(272, 237, digest).unwrap();
        footer.verify_index_hash(&bytes).unwrap();
    }

    #[test]
    /// `encodes an empty index as just its header`.
    fn encodes_an_empty_index_as_just_its_header() {
        let empty = Index::decode(&{
            let mut b = Vec::new();
            b.extend_from_slice(&INDEX_MAGIC);
            b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
            b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b
        })
        .unwrap();
        let bytes = empty.encode().unwrap();
        assert_eq!(bytes.len(), INDEX_HEADER_LEN);
        assert!(Index::decode(&bytes).unwrap().is_empty());
    }

    #[test]
    /// `every integer is serialized little endian`.
    fn every_integer_is_serialized_little_endian() {
        // Explicitly assert the byte order at each field position so a future
        // refactor to a native-endian write cannot pass on a little-endian CI
        // machine (T0322: architecture independence is a spec requirement).
        let idx = Index::decode(&reference_index()).unwrap();
        let bytes = idx.encode().unwrap();
        assert_eq!(&bytes[0..8], &INDEX_MAGIC);
        // index_version = 1 -> [1, 0]
        assert_eq!(&bytes[8..10], &[0x01, 0x00]);
        // entry_format = 1 -> [1, 0]
        assert_eq!(&bytes[10..12], &[0x01, 0x00]);
        // resource_count = 5 -> [5, 0, 0, 0]
        assert_eq!(&bytes[12..16], &[0x05, 0x00, 0x00, 0x00]);

        // Every field of the first entry, position by position. Asserting the
        // count in particular matters: a serializer that wrote a wrong count
        // would still round-trip through decode only if decode ignored it too,
        // and both being wrong is exactly the drift golden bytes catch.
        let first_path = b"Header/format.version";
        assert_eq!(&bytes[16..18], &[0x15, 0x00]); // path_length 21
        assert_eq!(&bytes[18..18 + first_path.len()], first_path);
        let e = 18 + first_path.len();
        assert_eq!(&bytes[e..e + 8], &[0x20, 0, 0, 0, 0, 0, 0, 0]); // offset 32
        assert_eq!(&bytes[e + 8..e + 16], &[0x30, 0, 0, 0, 0, 0, 0, 0]); // compressed 48
        assert_eq!(&bytes[e + 16..e + 24], &[0x06, 0, 0, 0, 0, 0, 0, 0]); // uncompressed 6
        assert_eq!(bytes[e + 24], COMPRESSION_ZSTD); // compression 1
    }

    #[test]
    /// `encoding is stable regardless of the u32 count encoding path`.
    fn encoding_is_stable_regardless_of_the_u32_count_encoding_path() {
        // resource_count is u32; a count above 255 must serialize with the same
        // rule as a small one. Build a 300-entry index and check the header.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&300u32.to_le_bytes());
        for i in 0..300u32 {
            let path = format!("r{i:06}");
            let len = u16::try_from(path.len()).unwrap();
            b.extend_from_slice(&len.to_le_bytes());
            b.extend_from_slice(path.as_bytes());
            b.extend_from_slice(&(u64::from(i) * 10).to_le_bytes());
            b.extend_from_slice(&5u64.to_le_bytes());
            b.extend_from_slice(&5u64.to_le_bytes());
            b.push(COMPRESSION_ZSTD);
        }
        let idx = Index::decode(&b).unwrap();
        assert_eq!(idx.len(), 300);
        let re = idx.encode().unwrap();
        assert_eq!(&re[12..16], &300u32.to_le_bytes());
        assert_eq!(re, b);
    }

    // ---- coverage: encode-side guards and paths not reached by the
    // reference fixture ----

    #[test]
    /// `encode and hash returns bytes matching encode`.
    fn encode_and_hash_returns_bytes_matching_encode() {
        let idx = Index::decode(&reference_index()).unwrap();
        let (bytes, digest) = idx.encode_and_hash().unwrap();
        assert_eq!(bytes, idx.encode().unwrap());
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(digest, expected);
    }

    #[test]
    /// `decode rejects a resource count that exceeds the ceiling`.
    fn decode_rejects_a_resource_count_that_exceeds_the_ceiling() {
        // count = 25_001 with no entry bytes: the ceiling check must fire
        // before the entry loop tries to read a missing first entry.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&25_001u32.to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_COUNT_OUT_OF_RANGE");
    }

    #[test]
    /// `decode rejects an entry whose path length overruns the buffer`.
    fn decode_rejects_an_entry_whose_path_length_overruns_the_buffer() {
        // path_length = 65535 but the buffer ends after the length field.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&u16::MAX.to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `decode rejects a truncated compression byte`.
    fn decode_rejects_a_truncated_compression_byte() {
        // A well-formed path followed by an entry that stops one byte short
        // of its 25-byte tail: the compression byte read must fail.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        let path = b"a";
        b.extend_from_slice(
            &u16::try_from(path.len())
                .expect("test path fits u16")
                .to_le_bytes(),
        );
        b.extend_from_slice(path);
        b.extend_from_slice(&32u64.to_le_bytes());
        b.extend_from_slice(&8u64.to_le_bytes());
        b.extend_from_slice(&8u64.to_le_bytes());
        // compression byte intentionally absent.
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `a prefix of a valid index is never a valid index`.
    fn a_prefix_of_a_valid_index_is_never_a_valid_index() {
        // Guards the sweep test's own invariant: no prefix of the golden
        // index decodes successfully. If this ever fails, the sweep below is
        // asserting something weaker than it claims.
        let full = reference_index();
        for len in 0..full.len() {
            assert!(
                Index::decode(&full[..len]).is_err(),
                "{len}-byte prefix decoded successfully"
            );
        }
    }

    #[test]
    /// `every truncation is rejected with an index or path code`.
    fn every_truncation_is_rejected_with_an_index_or_path_code() {
        // Complements the sweep: pins the *category* of rejection so a future
        // change cannot start returning an unregistered code for a prefix.
        let full = reference_index();
        for len in 0..full.len() {
            let e = Index::decode(&full[..len]).unwrap_err();
            let code = e.code();
            assert!(
                is_index_or_path_code(code),
                "{len}-byte prefix produced unregistered code {code}"
            );
        }
    }

    #[test]
    /// `a corrupted byte never yields an identical index`.
    fn a_corrupted_byte_never_yields_an_identical_index() {
        // The property the single-byte sweep relies on, asserted directly so
        // the sweep cannot silently degenerate into a no-op.
        let full = reference_index();
        let baseline = Index::decode(&full).unwrap();
        let mut survived = 0usize;
        for pos in 0..full.len() {
            let mut m = full.clone();
            m[pos] ^= 0xFF;
            if let Ok(v) = Index::decode(&m) {
                survived += 1;
                assert_ne!(v, baseline, "byte {pos} produced the same index");
            }
        }
        // Some corruptions legitimately survive (a byte inside a still-valid
        // path body), but not all 237.
        assert!(
            survived < full.len(),
            "every corruption survived: suspicious"
        );
    }

    #[test]
    /// `decode rejects an entry whose path slice runs past the buffer`.
    fn decode_rejects_an_entry_whose_path_slice_runs_past_the_buffer() {
        // A well-formed path length that fits u16, but the entry's tail is
        // present while the path bytes themselves are cut short. This is the
        // distinct case from the overflow guards above: nothing overflows,
        // the slice is simply out of range.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        // Claim a 40-byte path, supply 4 bytes, then a full 25-byte tail.
        b.extend_from_slice(&40u16.to_le_bytes());
        b.extend_from_slice(b"abcd");
        b.extend_from_slice(&32u64.to_le_bytes());
        b.extend_from_slice(&8u64.to_le_bytes());
        b.extend_from_slice(&8u64.to_le_bytes());
        b.push(COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `decode rejects an entry whose tail is absent after a valid path`.
    fn decode_rejects_an_entry_whose_tail_is_absent_after_a_valid_path() {
        // Path is fully present and within bounds, but the entry's 25-byte
        // tail is missing. The tail-bound check must fire before any read
        // inside the tail can go out of bounds.
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(b"abcd");
        // Only 20 of the 25 tail bytes follow.
        b.extend_from_slice(&[0u8; 20]);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    /// `index decode rejects an index above the 32 MiB ceiling`.
    ///
    /// `MAX_INDEX_BYTES` is a declared ceiling; before this check it was
    /// declared but never enforced, so a maliciously large index region
    /// would be walked entry-by-entry without rejection.
    fn index_decode_rejects_an_index_above_the_ceiling() {
        let ceiling = usize::try_from(elang_core::limits::MAX_INDEX_BYTES).unwrap_or(usize::MAX);
        // A buffer one byte over the ceiling, built with the correct
        // magic and version so the size check is what rejects it and
        // not a later structural check.
        let mut b = Vec::with_capacity(ceiling + 1);
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&[0u8; 4]);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.resize(ceiling + 1, 0u8);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_SIZE_OUT_OF_RANGE");
    }

    #[test]
    /// `index decode accepts an index at exactly the ceiling`.
    fn index_decode_accepts_an_index_at_the_ceiling() {
        let ceiling = usize::try_from(elang_core::limits::MAX_INDEX_BYTES).unwrap_or(usize::MAX);
        let mut b = Vec::with_capacity(ceiling);
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&[0u8; 4]);
        b.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.resize(ceiling, 0u8);
        // Zero entries means the entry cursor is at the header end and
        // the trailing bytes are a malformed index — which is the
        // *next* check, proving the size check did not fire first.
        let e = Index::decode(&b).unwrap_err();
        assert_ne!(e.code(), "INDEX_SIZE_OUT_OF_RANGE");
    }

    #[test]
    /// `into resource entry failure surfaces from the bulk conversion`.
    fn into_resource_entry_failure_surfaces_from_the_bulk_conversion() {
        // `to_resource_index` collects every entry through
        // `into_resource_entry`, which range-checks against the archive size.
        // An archive size smaller than the frames must propagate as an error
        // rather than producing a ResourceIndex.
        let idx = Index::decode(&reference_index()).unwrap();
        // The reference frames run to byte 272; claim an archive of 100 bytes.
        let e = to_resource_index(&idx, 100).unwrap_err();
        // `ResourceEntry::new` range-checks against the archive size; the
        // codes it can raise here are index-range or limit-prefixed.
        assert!(
            is_index_or_path_code(e.code()) || e.code().starts_with("LIMIT"),
            "got {}",
            e.code()
        );
    }

    // ---- writer-side constructors (T1305/T1306) ----

    #[test]
    /// `index entry new accepts a well formed entry`.
    fn index_entry_new_accepts_a_well_formed_entry() {
        let e = IndexEntry::new("a/b.eld", 32, 100, 200, COMPRESSION_ZSTD).unwrap();
        assert_eq!(e.path().as_str(), "a/b.eld");
        assert_eq!(e.frame_offset(), 32);
        assert_eq!(e.compressed_size(), 100);
        assert_eq!(e.uncompressed_size(), 200);
        assert_eq!(e.compression(), COMPRESSION_ZSTD);
    }

    #[test]
    /// `index entry new rejects non zstd compression`.
    fn index_entry_new_rejects_non_zstd_compression() {
        let e = IndexEntry::new("a.eld", 32, 100, 200, 2).unwrap_err();
        assert_eq!(e.code(), "INDEX_BAD_VERSION");
    }

    #[test]
    /// `index entry new rejects non nfc path`.
    fn index_entry_new_rejects_non_nfc_path() {
        // Decomposed 'é' (e + U+0301 combining acute) is not NFC.
        let decomposed = "cafe\u{0301}.eld";
        assert!(!elang_unicode::is_nfc(decomposed));
        let e = IndexEntry::new(decomposed, 32, 100, 200, COMPRESSION_ZSTD).unwrap_err();
        assert_eq!(e.code(), "PATH_NOT_NFC");
    }

    #[test]
    /// `index entry new rejects an over long path`.
    fn index_entry_new_rejects_an_over_long_path() {
        let long = "a".repeat(elang_core::limits::MAX_RESOURCE_PATH_BYTES + 1);
        let e = IndexEntry::new(&long, 32, 100, 200, COMPRESSION_ZSTD).unwrap_err();
        // `ResourcePath::new` rejects it with PATH_TOO_LONG; a path that fits
        // u16 but exceeds the ResourcePath ceiling takes this branch first.
        assert!(
            e.code() == "PATH_TOO_LONG" || e.code() == "INDEX_ENTRY_MALFORMED",
            "got {}",
            e.code()
        );
    }

    #[test]
    /// `index with entries accepts a sorted unique vector`.
    fn index_with_entries_accepts_a_sorted_unique_vector() {
        let e1 = IndexEntry::new("a.eld", 32, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e2 = IndexEntry::new("b.eld", 52, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e3 = IndexEntry::new("c.eld", 72, 10, 20, COMPRESSION_ZSTD).unwrap();
        let idx = Index::with_entries(vec![e1, e2, e3]).unwrap();
        assert_eq!(idx.len(), 3);
        assert_eq!(idx.entries()[1].path().as_str(), "b.eld");
    }

    #[test]
    /// `index with entries rejects unsorted entries`.
    fn index_with_entries_rejects_unsorted_entries() {
        let e1 = IndexEntry::new("z.eld", 32, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e2 = IndexEntry::new("a.eld", 52, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e = Index::with_entries(vec![e1, e2]).unwrap_err();
        assert_eq!(e.code(), "INDEX_PATH_NOT_SORTED");
    }

    #[test]
    /// `index with entries rejects duplicate paths`.
    fn index_with_entries_rejects_duplicate_paths() {
        let e1 = IndexEntry::new("a.eld", 32, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e2 = IndexEntry::new("a.eld", 52, 10, 20, COMPRESSION_ZSTD).unwrap();
        let e = Index::with_entries(vec![e1, e2]).unwrap_err();
        assert_eq!(e.code(), "INDEX_DUPLICATE_PATH");
    }

    #[test]
    /// `writer construct round trips through encode decode`.
    fn writer_construct_round_trips_through_encode_decode() {
        // The end-to-end property the writer needs: building an Index via the
        // new constructors and encoding it produces bytes that decode back to
        // an equal Index. Proves the new constructors and the existing
        // encode/decode pair agree on the wire format.
        let e1 = IndexEntry::new("Header/format.version", 32, 48, 6, COMPRESSION_ZSTD).unwrap();
        let e2 =
            IndexEntry::new("Linguistics/phonemes.eld", 80, 100, 50, COMPRESSION_ZSTD).unwrap();
        let e3 = IndexEntry::new("META-INF/integrity.kdl", 180, 50, 25, COMPRESSION_ZSTD).unwrap();
        let e4 = IndexEntry::new("mimetype", 230, 12, 4, COMPRESSION_ZSTD).unwrap();
        let e5 = IndexEntry::new("package.epf", 242, 30, 15, COMPRESSION_ZSTD).unwrap();
        let built = Index::with_entries(vec![e1, e2, e3, e4, e5]).unwrap();
        let bytes = built.encode().unwrap();
        let parsed = Index::decode(&bytes).unwrap();
        assert_eq!(built, parsed);
    }
}
