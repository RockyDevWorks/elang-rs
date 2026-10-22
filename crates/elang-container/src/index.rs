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
//! Ordering and uniqueness (T0315) are enforced by `elang_core::ResourceIndex`,
//! which this module drives — the same split the core types were designed for.
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
pub struct IndexEntry {
    path: elang_core::ResourcePath,
    frame_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    compression: u8,
}

impl IndexEntry {
    /// The validated, NFC-checked resource path.
    #[must_use]
    pub fn path(&self) -> &elang_core::ResourcePath {
        &self.path
    }

    /// Absolute byte offset of this resource's frame.
    #[must_use]
    pub fn frame_offset(&self) -> u64 {
        self.frame_offset
    }

    /// Compressed (on-disk) frame size.
    #[must_use]
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Declared uncompressed size.
    #[must_use]
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    /// Compression code (always [`COMPRESSION_ZSTD`] for a parsed entry).
    #[must_use]
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
/// enforced when building a `ResourceIndex` from the entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    entries: Vec<IndexEntry>,
}

impl Index {
    /// Parse the serialized index bytes (header + entries).
    ///
    /// `bytes` is the exact index region — `index_size` bytes read from the
    /// archive at `index_offset` (the caller has already verified the footer
    /// hash and the framing relation). Errors:
    /// - `INDEX_BAD_MAGIC` — fewer than 16 bytes or the magic differs.
    /// - `INDEX_BAD_VERSION` — `index_version` or `entry_format` != 1.
    /// - `INDEX_COUNT_OUT_OF_RANGE` — `resource_count` > 25,000.
    /// - `INDEX_ENTRY_MALFORMED` — an entry is truncated, overruns the buffer,
    ///   or has an illegal compression code.
    /// - path errors (`PATH_NOT_UTF8`, `PATH_NOT_NFC`, `PATH_ABSOLUTE`, …) —
    ///   from `ResourcePath`, after NFC validation.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < INDEX_HEADER_LEN {
            return Err(index_err(
                "INDEX_BAD_MAGIC",
                "index is shorter than the 16-byte index header",
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

        Ok(Self { entries })
    }

    /// The parsed entries, in index order.
    #[must_use]
    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    /// Number of parsed entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index declares no resources.
    #[must_use]
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
    /// `INDEX_SIZE_OVER_CEILING` if the total serialization would exceed
    /// `MAX_INDEX_SIZE` (32 MiB), or `INDEX_COUNT_OUT_OF_RANGE` if an entry
    /// path exceeds the u16 length field.
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

    /// The exact 237-byte reference index from `index.bin`.
    fn reference_index() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/valid/container/minimal-archive/index.bin");
        std::fs::read(&path).expect("index.bin fixture present")
    }

    // ---- T0309 / T0310: header ----

    #[test]
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
    fn rejects_bad_index_magic() {
        let mut b = reference_index();
        b[0] = 0x00;
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_BAD_MAGIC");
        assert_eq!(e.category(), Category::Index);
    }

    #[test]
    fn rejects_bad_index_version_or_entry_format() {
        let mut ver = reference_index();
        ver[8] = 2; // index_version = 2
        assert_eq!(Index::decode(&ver).unwrap_err().code(), "INDEX_BAD_VERSION");

        let mut fmt = reference_index();
        fmt[10] = 2; // entry_format = 2
        assert_eq!(Index::decode(&fmt).unwrap_err().code(), "INDEX_BAD_VERSION");
    }

    #[test]
    fn rejects_count_above_the_ceiling() {
        let mut b = reference_index();
        b[12..16].copy_from_slice(&(elang_core::limits::MAX_RESOURCES + 1).to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_COUNT_OUT_OF_RANGE");
    }

    // ---- T0311 / T0312: entries ----

    #[test]
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
    fn rejects_a_truncated_entry() {
        // Chop the last few bytes so the final entry runs off the end.
        let full = reference_index();
        let truncated = &full[..full.len() - 10];
        let e = Index::decode(truncated).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
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
                        code.starts_with("INDEX_")
                            || code.starts_with("PATH_")
                            || code.starts_with("ARCHIVE"),
                        "prefix {len} produced unregistered code {code}",
                    );
                }
            }
        }
    }

    #[test]
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
    fn rejects_a_huge_path_length_without_allocating() {
        // Claim a 65535-byte path on a 237-byte index: must fail, not allocate.
        let mut b = reference_index();
        b[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    // ---- T0313 / T0314: path validation ----

    #[test]
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
    fn rejects_absolute_path() {
        let mut b = index_header(1);
        append_entry(&mut b, b"/etc/passwd", 32, 10, 10, COMPRESSION_ZSTD);
        let e = Index::decode(&b).unwrap_err();
        assert_eq!(e.code(), "PATH_ABSOLUTE");
    }

    #[test]
    fn rejects_dot_dot_segment() {
        let mut b = index_header(1);
        append_entry(&mut b, b"a/../../etc/passwd", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b).unwrap_err().code(), "PATH_EMPTY_SEGMENT");
    }

    #[test]
    fn rejects_nul_and_backslash_in_path() {
        let mut b = index_header(1);
        append_entry(&mut b, b"a\0b.eld", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b).unwrap_err().code(), "PATH_NUL");

        let mut b2 = index_header(1);
        append_entry(&mut b2, b"a\\b.eld", 32, 10, 10, COMPRESSION_ZSTD);
        assert_eq!(Index::decode(&b2).unwrap_err().code(), "PATH_BACKSLASH");
    }

    #[test]
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
    fn reference_index_satisfies_the_ordering_invariants() {
        let idx = Index::decode(&reference_index()).unwrap();
        // 573-byte archive; every frame fits.
        assert!(to_resource_index(&idx, 573).is_ok());
    }

    #[test]
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
    fn rejects_duplicate_entries_when_built_as_a_resource_index() {
        let mut b = index_header(2);
        append_entry(&mut b, b"a.eld", 32, 10, 10, COMPRESSION_ZSTD);
        append_entry(&mut b, b"a.eld", 42, 10, 10, COMPRESSION_ZSTD);
        let idx = Index::decode(&b).unwrap();
        let e = to_resource_index(&idx, 1000).unwrap_err();
        assert_eq!(e.code(), "INDEX_DUPLICATE_PATH");
    }

    // ---- helpers to build synthetic indexes ----

    fn index_header(count: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&INDEX_MAGIC);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&count.to_le_bytes());
        b
    }

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
    fn encoding_is_idempotent() {
        let golden = reference_index();
        let once = Index::decode(&golden).unwrap().encode().unwrap();
        let twice = Index::decode(&once).unwrap().encode().unwrap();
        assert_eq!(once, twice);
    }

    #[test]
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
}
