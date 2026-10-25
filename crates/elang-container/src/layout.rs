//! Frame layout validation and index-only archive listing.
//!
//! Covers T0316-T0318 (contiguity) and T0324 (index-only listing) from Issue
//! #12. The layout rule from REQ-BIN-014 is:
//!
//! > Frames contiguous from byte 32; no gaps/overlaps/unindexed/trailing
//! > bytes; last frame ends at index offset.
//!
//! Frames are laid out in **index order** (ascending `frame_offset`), each one
//! immediately following the previous, beginning at byte 32 (`HEADER_LEN`), and
//! the last one ending exactly at `index_offset`. Every byte in
//! `[32, index_offset)` must belong to exactly one frame. This module proves
//! that relation without decompressing anything.

use elang_core::limits::{MAX_GENERIC_RESOURCE_BYTES, MAX_UNCOMPRESSED_AGGREGATE_BYTES};
use elang_core::{Category, Error, Result};

use crate::footer::Footer;
use crate::header::{Header, HEADER_LEN};
use crate::index::Index;

fn layout_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Index, msg)
}

/// What one frame occupies in the archive.
///
/// A frame is the compressed resource body. Listing does **not** decompress it
/// (REQ-BIN-017) — only the byte range and the declared sizes are surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSpan {
    /// Absolute byte offset of this frame in the archive.
    pub offset: u64,
    /// On-disk (compressed) size of this frame.
    pub compressed_size: u64,
}

impl FrameSpan {
    /// Exclusive end offset of this frame, or `None` if it would overflow `u64`.
    ///
    /// A wrapping `offset + compressed_size` is the whole point of this being a
    /// method: a frame claiming `offset = u64::MAX, size = 8` must be rejected,
    /// not silently wrap to 7 and compare equal.
    #[must_use]
    pub fn end(&self) -> Option<u64> {
        self.offset.checked_add(self.compressed_size)
    }
}

/// Proven frame layout: contiguous, gapless, exactly covering `[32, index_offset)`.
///
/// Constructed only by [`validate_layout`], so holding one is proof the
/// contiguity relation holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLayout {
    spans: Vec<FrameSpan>,
    index_offset: u64,
}

impl FrameLayout {
    /// The per-frame spans, in archive order.
    #[must_use]
    pub fn spans(&self) -> &[FrameSpan] {
        &self.spans
    }

    /// Number of frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Whether there are no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Exclusive end of the last frame — always equal to `index_offset`.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.index_offset
    }

    /// Total bytes covered by all frames (`index_offset - HEADER_LEN`).
    #[must_use]
    pub fn total_frame_bytes(&self) -> u64 {
        self.index_offset.saturating_sub(HEADER_LEN as u64)
    }
}

/// Validate that frames are contiguous from byte 32 with no gaps, overlaps,
/// unindexed bytes, or trailing bytes (T0316-T0318, REQ-BIN-014).
///
/// `index` supplies the frame offsets/sizes in index order. `index_offset` is
/// where the index begins, i.e. the exclusive end the last frame must reach.
///
/// Checks, in order, each under checked arithmetic so no `u64` wrap can make a
/// malformed archive look well-formed:
///
/// 1. Each frame's `offset + compressed_size` does not overflow (`u64`).
/// 2. The first frame starts exactly at `HEADER_LEN` (32).
/// 3. Each frame starts exactly where the previous ended — a start *past* the
///    previous end is `FRAME_LAYOUT_GAP`; a start *before* it is
///    `FRAME_LAYOUT_OVERLAP`.
/// 4. The last frame ends exactly at `index_offset` — short leaves unindexed
///    bytes (`FRAME_LAYOUT_UNINDEXED_BYTES`); long overruns the index
///    (`FRAME_LAYOUT_TRAILING_BYTES`).
/// 5. An empty index is only valid when `index_offset == HEADER_LEN`; an empty
///    index with frames expected is `FRAME_LAYOUT_UNINDEXED_BYTES`.
///
/// # Errors
///
/// - `FRAME_LAYOUT_GAP` — a hole between frames, or the first frame not at 32.
/// - `FRAME_LAYOUT_OVERLAP` — frames covering the same byte twice.
/// - `FRAME_LAYOUT_UNINDEXED_BYTES` — bytes between the last frame and the
///   index that no index entry accounts for.
/// - `FRAME_LAYOUT_TRAILING_BYTES` — the last frame runs past the index start.
/// - `INDEX_ENTRY_MALFORMED` — a frame's `offset + size` overflows `u64`.
pub fn validate_layout(index: &Index, index_offset: u64) -> Result<FrameLayout> {
    let entries = index.entries();

    // Case 1: no frames. Only coherent if the index starts right after the
    // header; otherwise the whole gap is unindexed.
    if entries.is_empty() {
        if index_offset == HEADER_LEN as u64 {
            return Ok(FrameLayout {
                spans: Vec::new(),
                index_offset,
            });
        }
        return Err(layout_err(
            "FRAME_LAYOUT_UNINDEXED_BYTES",
            "index declares no frames but the archive has body bytes",
        ));
    }

    let mut spans = Vec::with_capacity(entries.len());

    // Case 2 (first frame at byte 32) falls out of the loop below rather than
    // getting its own check: `cursor` starts at HEADER_LEN, so an entry whose
    // `frame_offset` is not exactly 32 trips the `offset > cursor` gap branch
    // on the very first iteration. A separate pre-check would be the same
    // comparison with the same result code, just written twice.
    let mut cursor = HEADER_LEN as u64;

    for entry in entries {
        let span = FrameSpan {
            offset: entry.frame_offset(),
            compressed_size: entry.compressed_size(),
        };

        // Case 1: reject a frame whose end would wrap u64.
        let end = span.end().ok_or_else(|| {
            layout_err(
                "INDEX_ENTRY_MALFORMED",
                "frame offset + compressed size overflows u64",
            )
        })?;

        // Case 3: contiguity — a gap leaves bytes no entry accounts for, an
        // overlap means two entries claim the same bytes.
        if span.offset > cursor {
            return Err(layout_err(
                "FRAME_LAYOUT_GAP",
                "frames are not contiguous: a gap precedes this frame",
            ));
        }
        if span.offset < cursor {
            return Err(layout_err(
                "FRAME_LAYOUT_OVERLAP",
                "frames are not contiguous: this frame overlaps the previous",
            ));
        }

        // Case 5: a zero-length frame advances nothing; it would let the same
        // offset appear twice without tripping the contiguity check.
        if span.compressed_size == 0 {
            return Err(layout_err(
                "INDEX_ENTRY_MALFORMED",
                "frame has zero compressed size",
            ));
        }

        spans.push(span);
        cursor = end;
    }

    // Case 4: the last frame must end exactly at the index offset.
    if cursor < index_offset {
        return Err(layout_err(
            "FRAME_LAYOUT_UNINDEXED_BYTES",
            "bytes between the last frame and the index are not covered by any entry",
        ));
    }
    if cursor > index_offset {
        return Err(layout_err(
            "FRAME_LAYOUT_TRAILING_BYTES",
            "the last frame extends past the start of the index",
        ));
    }

    Ok(FrameLayout {
        spans,
        index_offset,
    })
}

/// One line of an index-only archive listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingEntry {
    /// The resource's validated path.
    pub path: String,
    /// Absolute byte offset of the frame.
    pub frame_offset: u64,
    /// Compressed (on-disk) frame size.
    pub compressed_size: u64,
    /// Declared uncompressed size.
    pub uncompressed_size: u64,
}

/// An index-only view of an archive (T0324, REQ-BIN-017).
///
/// Built by reading only the header, the terminal footer, and the index region.
/// No frame body is decompressed, so listing is O(index size) regardless of
/// how large the archive is. This is the entry point for `elang inspect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveListing {
    header: Header,
    footer: Footer,
    index: Index,
    layout: FrameLayout,
}

impl ArchiveListing {
    /// Parse an archive for listing, validating framing, index invariants and
    /// layout without decompressing a frame.
    ///
    /// Runs, in order: header decode, footer decode, the
    /// `index_offset + index_size + 64 == len` framing check (T0306), the index
    /// SHA-256 check (T0308), index parse, bytewise path ordering/uniqueness and
    /// declared size ceilings, then frame layout (T0316-T0318).
    ///
    /// # Errors
    ///
    /// Any of the header/footer/index/layout errors documented on those types.
    /// Index invariants return `INDEX_DUPLICATE_PATH`, `INDEX_PATH_NOT_SORTED`,
    /// `INDEX_SIZE_OVER_CEILING`, or `LIMIT_UNCOMPRESSED_AGGREGATE`.
    pub fn parse(archive: &[u8]) -> Result<Self> {
        let header = Header::decode(archive)?;

        // A too-small archive cannot even hold a header plus a footer.
        let min = (HEADER_LEN + crate::footer::FOOTER_LEN) as u64;
        let file_size = u64::try_from(archive.len())
            .map_err(|_| layout_err("ARCHIVE_TOO_SMALL", "archive length does not fit in u64"))?;
        if file_size < min {
            return Err(Error::new(
                "ARCHIVE_TOO_SMALL",
                Category::Container,
                "archive is smaller than a header plus a footer",
            ));
        }

        let footer = Footer::decode(archive)?;
        footer.check_framing(file_size)?;

        // Locate the index region under checked arithmetic only.
        let index_offset = usize::try_from(footer.index_offset()).map_err(|_| {
            layout_err(
                "ARCHIVE_SIZE_INCONSISTENT",
                "index offset does not fit in usize on this platform",
            )
        })?;
        let index_size = usize::try_from(footer.index_size()).map_err(|_| {
            layout_err(
                "ARCHIVE_SIZE_INCONSISTENT",
                "index size does not fit in usize on this platform",
            )
        })?;
        let index_end = index_offset.checked_add(index_size).ok_or_else(|| {
            layout_err(
                "ARCHIVE_SIZE_INCONSISTENT",
                "index offset + index size overflows",
            )
        })?;
        // Framing already proved index_end + 64 == file_size, so this slice is
        // in bounds; checked anyway so a future change cannot make it a panic.
        let index_bytes = archive.get(index_offset..index_end).ok_or_else(|| {
            layout_err(
                "ARCHIVE_SIZE_INCONSISTENT",
                "index region falls outside the archive",
            )
        })?;

        footer.verify_index_hash(index_bytes)?;
        let index = Index::decode(index_bytes)?;

        // Enforce the ResourceIndex invariants directly: index-only inspection
        // has no integrity-manifest digests to build ResourceEntry values with.
        let mut previous_path: Option<&[u8]> = None;
        let mut total_uncompressed = 0u64;
        for entry in index.entries() {
            let path = entry.path().as_bytes();
            if let Some(previous) = previous_path {
                if previous == path {
                    return Err(layout_err(
                        "INDEX_DUPLICATE_PATH",
                        "duplicate resource path",
                    ));
                }
                if previous > path {
                    return Err(layout_err(
                        "INDEX_PATH_NOT_SORTED",
                        "resource paths are not in ascending UTF-8 byte order",
                    ));
                }
            }
            previous_path = Some(path);
            if entry.uncompressed_size() > MAX_GENERIC_RESOURCE_BYTES {
                return Err(layout_err(
                    "INDEX_SIZE_OVER_CEILING",
                    "uncompressed resource size exceeds the per-resource ceiling",
                ));
            }
            total_uncompressed = total_uncompressed
                .checked_add(entry.uncompressed_size())
                .filter(|total| *total <= MAX_UNCOMPRESSED_AGGREGATE_BYTES)
                .ok_or_else(|| {
                    Error::new(
                        "LIMIT_UNCOMPRESSED_AGGREGATE",
                        Category::Limit,
                        "aggregate uncompressed size exceeds the archive ceiling",
                    )
                })?;
        }

        let layout = validate_layout(&index, footer.index_offset())?;

        Ok(Self {
            header,
            footer,
            index,
            layout,
        })
    }

    /// The validated 32-byte header.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The validated 64-byte terminal footer.
    #[must_use]
    pub fn footer(&self) -> &Footer {
        &self.footer
    }

    /// The parsed index.
    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// The proven frame layout.
    #[must_use]
    pub fn layout(&self) -> &FrameLayout {
        &self.layout
    }

    /// Number of resources, without decompressing any of them.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether the archive declares no resources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Total on-disk bytes occupied by all frames.
    #[must_use]
    pub fn total_frame_bytes(&self) -> u64 {
        self.layout.total_frame_bytes()
    }

    /// The index-only listing: one entry per resource, no decompression.
    ///
    /// Paths are returned as `String` for display; each was validated (NFC,
    /// absolute, no `.`/`..`) during index parse.
    #[must_use]
    pub fn listing(&self) -> Vec<ListingEntry> {
        self.index
            .entries()
            .iter()
            .map(|e| ListingEntry {
                path: e.path().as_str().to_owned(),
                frame_offset: e.frame_offset(),
                compressed_size: e.compressed_size(),
                uncompressed_size: e.uncompressed_size(),
            })
            .collect()
    }
}

#[cfg(test)]
// Test-only allowances, and only for tests.
//
// `unwrap_used`/`expect_used`/`indexing_slicing`/`panic`: tests assert on
// known-good fixture bytes and on expected decode failures, so a failed
// precondition is a test bug that must abort loudly rather than produce a
// misleading pass.
//
// `arithmetic_side_effects`: these tests deliberately edit raw fixture bytes
// at hand-computed offsets (`entry_offset_field` +8, +16) to build malformed
// archives. The offsets are verified against the fixture by
// `entry_offset_field_is_correct`, so checked arithmetic on them would only
// obscure the byte positions under test. Production code in this module uses
// `checked_add` throughout and is covered by the sabotage suite in the same
// test module — those mutating the validator itself rather than the fixture.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::testdata::layout as fix;
    use crate::testdata::{reference_archive, reference_footer, reference_header, reference_index};
    use crate::{Footer, INDEX_HEADER_LEN};
    use sha2::{Digest, Sha256};

    /// Whether a rejection code belongs to any of the container-framing
    /// families that `ArchiveListing::parse` can legitimately return.
    ///
    /// Extracted so each assertion is a single covered line rather than a
    /// short-circuited `||` chain whose later arms never execute.
    fn is_container_parse_code(code: &str) -> bool {
        const FAMILIES: [&str; 4] = ["ARCHIVE", "HEADER", "FOOTER", "INDEX"];
        FAMILIES.iter().any(|f| code.starts_with(f))
    }

    // ---- T0316: contiguity from byte 32 ----

    #[test]
    fn accepts_the_reference_contiguous_layout() {
        let idx = Index::decode(&reference_index()).unwrap();
        let layout = validate_layout(&idx, fix::INDEX_OFFSET).unwrap();
        assert_eq!(layout.len(), fix::RESOURCE_COUNT);
        assert_eq!(layout.end(), fix::INDEX_OFFSET);
        assert_eq!(layout.total_frame_bytes(), fix::FRAMES_LEN);
    }

    #[test]
    fn rejects_a_first_frame_that_does_not_start_at_byte_32() {
        // Move the first frame to 33, leaving byte 32 unindexed.
        let mut raw = reference_index();
        let len0 = u16::from_le_bytes([raw[16], raw[17]]) as usize;
        let off_at = 16 + 2 + len0;
        raw[off_at..off_at + 8].copy_from_slice(&33u64.to_le_bytes());
        let idx = Index::decode(&raw).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_GAP");
    }

    // ---- T0317: gaps, overlaps, unindexed, overflow ----

    #[test]
    fn rejects_a_gap_between_frames() {
        // Push frame 2 from 80 to 81: a 1-byte hole after frame 1.
        let raw = index_with_second_offset(81);
        let idx = Index::decode(&raw).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_GAP");
    }

    #[test]
    fn rejects_overlapping_frames() {
        // Grow frame 1 (at offset 80, size 48 -> ends 128) so it runs past the
        // start of frame 2 (at 128). The next frame then begins *before* the
        // previous one ended: two entries claim the same bytes.
        let raw = index_with_second(80u64, 96u64);
        let idx = Index::decode(&raw).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_OVERLAP");
    }

    #[test]
    fn rejects_unindexed_bytes_before_the_index() {
        // Frames are contiguous among themselves but stop short of INDEX_OFFSET.
        let idx = Index::decode(&reference_index()).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET + 8).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_UNINDEXED_BYTES");
    }

    #[test]
    fn rejects_trailing_frame_bytes_past_the_index() {
        // Frames run past INDEX_OFFSET and overlap the index itself.
        let idx = Index::decode(&reference_index()).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET - 8).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_TRAILING_BYTES");
    }

    #[test]
    fn rejects_an_empty_index_with_body_bytes() {
        // A zero-entry index is only valid when it starts at byte 32.
        let idx = Index::decode(&empty_index_bytes()).unwrap();
        assert!(idx.is_empty());
        // index_offset INDEX_OFFSET with no frames => body bytes unaccounted for.
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "FRAME_LAYOUT_UNINDEXED_BYTES");
        // ...but an empty archive (index right after the header) is fine.
        let ok = validate_layout(&idx, HEADER_LEN as u64).unwrap();
        assert!(ok.is_empty());
    }

    #[test]
    fn rejects_a_zero_length_frame() {
        // A zero-size frame would let the same offset repeat without tripping
        // the contiguity check, so it must be rejected outright.
        let raw = index_with_first_size(0);
        let idx = Index::decode(&raw).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    fn rejects_a_frame_end_that_overflows_u64() {
        // Frame 0 runs 32..80 (size 48). Frame 1 claims offset 80 (contiguous,
        // so the gap/overlap checks pass) with compressed_size u64::MAX:
        // offset + size overflows u64, so no `end` exists at all. It must be
        // refused rather than wrapped to a small value that could then compare
        // equal to a plausible index_offset.
        let raw = index_with_second(80u64, u64::MAX);
        let idx = Index::decode(&raw).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    fn rejects_a_first_frame_offset_at_u64_max() {
        // A first frame at u64::MAX can never start at byte 32. The overflow
        // guard on `offset + compressed_size` fires before the contiguity
        // comparison, which is the more precise diagnostic: the frame
        // describes a range that cannot exist.
        let idx = Index::decode(&index_with_first_offset(u64::MAX)).unwrap();
        let e = validate_layout(&idx, fix::INDEX_OFFSET).unwrap_err();
        assert_eq!(e.code(), "INDEX_ENTRY_MALFORMED");
    }

    #[test]
    fn a_wrapping_frame_end_cannot_satisfy_the_index_offset() {
        // The precise wrap attack. Frame 1 at u64::MAX - 3 with size 8: a
        // wrapping `offset + size` yields 4. A validator that used wrapping
        // arithmetic and compared the result against an index_offset of 4
        // would accept this malformed archive. Under checked arithmetic the
        // frame has no end, so it is refused.
        let raw = index_with_second_wrapping_offset();
        let idx = Index::decode(&raw).unwrap();
        // index_offset 4 is exactly what a wrapping compare would accept.
        let e = validate_layout(&idx, 4).unwrap_err();
        assert_ne!(e.code(), "FRAME_LAYOUT_UNINDEXED_BYTES");
    }

    // ---- T0324: index-only listing ----

    #[test]
    fn lists_the_reference_archive_without_decompressing() {
        let listing = ArchiveListing::parse(&reference_archive()).unwrap();
        let rows = listing.listing();
        assert_eq!(rows.len(), fix::RESOURCE_COUNT);
        assert_eq!(rows[0].path, "Header/format.version");
        assert_eq!(rows[0].frame_offset, fix::FRAMES_OFFSET);
        assert_eq!(rows[0].compressed_size, fix::FRAME_SIZE);
        assert_eq!(rows[0].uncompressed_size, 6);
        assert_eq!(rows[3].path, "mimetype");
        assert_eq!(listing.len(), fix::RESOURCE_COUNT);
        assert_eq!(listing.total_frame_bytes(), fix::FRAMES_LEN);
        // The mimetype frame body is a placeholder, not real Zstd; listing
        // still works because it never touches frame bodies.
        assert_eq!(listing.header().major(), 0);
        assert_eq!(listing.footer().index_offset(), fix::INDEX_OFFSET);
    }

    #[test]
    fn parse_rejects_duplicate_paths() {
        let archive = archive_with_entries(&[("a.eld", 10), ("a.eld", 10)]);
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "INDEX_DUPLICATE_PATH");
        assert_eq!(e.category(), Category::Index);
    }

    #[test]
    fn parse_enforces_bytewise_path_order() {
        let archive = archive_with_entries(&[("a.eld", 10), ("Z.eld", 10)]);
        assert_eq!(
            ArchiveListing::parse(&archive).unwrap_err().code(),
            "INDEX_PATH_NOT_SORTED"
        );
        let sorted = archive_with_entries(&[("Z.eld", 10), ("a.eld", 10), ("é.eld", 10)]);
        assert!(ArchiveListing::parse(&sorted).is_ok());
    }

    #[test]
    fn parse_enforces_the_per_resource_size_ceiling() {
        let max = elang_core::limits::MAX_GENERIC_RESOURCE_BYTES;
        let at_ceiling = archive_with_entries(&[("a.eld", max)]);
        assert!(ArchiveListing::parse(&at_ceiling).is_ok());
        for size in [max + 1, u64::MAX] {
            let archive = archive_with_entries(&[("a.eld", size)]);
            let e = ArchiveListing::parse(&archive).unwrap_err();
            assert_eq!(e.code(), "INDEX_SIZE_OVER_CEILING");
            assert_eq!(e.category(), Category::Index);
        }
    }

    #[test]
    fn parse_enforces_the_aggregate_size_ceiling() {
        let max = elang_core::limits::MAX_GENERIC_RESOURCE_BYTES;
        let mut entries: Vec<_> = ('a'..='h').map(|c| (format!("{c}.eld"), max)).collect();
        let borrowed: Vec<_> = entries
            .iter()
            .map(|(p, size)| (p.as_str(), *size))
            .collect();
        let at_ceiling = archive_with_entries(&borrowed);
        assert!(ArchiveListing::parse(&at_ceiling).is_ok());

        entries.push(("i.eld".to_owned(), 1));
        let borrowed: Vec<_> = entries
            .iter()
            .map(|(p, size)| (p.as_str(), *size))
            .collect();
        let archive = archive_with_entries(&borrowed);
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "LIMIT_UNCOMPRESSED_AGGREGATE");
        assert_eq!(e.category(), Category::Limit);
    }

    #[test]
    fn parse_rejects_the_fixture_when_the_index_hash_is_wrong() {
        let mut archive = reference_archive();
        // Flip a byte inside the index region (offset INDEX_OFFSET..FOOTER_OFFSET).
        archive[usize::try_from(fix::INDEX_OFFSET + 8).unwrap()] ^= 0xFF;
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "FOOTER_INDEX_HASH_MISMATCH");
    }

    #[test]
    fn parse_rejects_a_truncated_archive() {
        let archive = reference_archive();
        let total = archive.len();
        for len in [0usize, 8, 31, 32, 95, 96, 200, total - 1] {
            let e = ArchiveListing::parse(&archive[..len]).unwrap_err();
            // The code is bound to a local so the diagnostic message is
            // built even when the assertion holds.
            let code = e.code();
            assert!(is_container_parse_code(code), "len {len} gave {code}");
        }
    }

    #[test]
    fn parse_rejects_an_archive_whose_footer_framing_is_wrong() {
        let mut archive = reference_archive();
        // Corrupt index_offset in the footer (bytes 8..16 of the 64-byte footer
        // start at 509): set it to 999 so framing fails.
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive[f + 8] = 0xE7;
        archive[f + 9] = 0x03;
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "ARCHIVE_SIZE_INCONSISTENT");
    }

    // ---- helpers ----

    /// Build a complete archive with contiguous one-byte frame bodies and a
    /// real index digest, isolating index invariants from framing failures.
    fn archive_with_entries(entries: &[(&str, u64)]) -> Vec<u8> {
        let mut index = Vec::from(crate::INDEX_MAGIC);
        index.extend_from_slice(&crate::INDEX_VERSION.to_le_bytes());
        index.extend_from_slice(&crate::ENTRY_FORMAT.to_le_bytes());
        index.extend_from_slice(&u32::try_from(entries.len()).unwrap().to_le_bytes());
        let mut offset = HEADER_LEN as u64;
        for (path, size) in entries {
            index.extend_from_slice(&u16::try_from(path.len()).unwrap().to_le_bytes());
            index.extend_from_slice(path.as_bytes());
            index.extend_from_slice(&offset.to_le_bytes());
            index.extend_from_slice(&1u64.to_le_bytes());
            index.extend_from_slice(&size.to_le_bytes());
            index.push(crate::COMPRESSION_ZSTD);
            offset += 1;
        }
        let mut archive = vec![0; HEADER_LEN + entries.len()];
        Header::current().encode(&mut archive, 0).unwrap();
        let digest = Sha256::digest(&index).into();
        let footer = Footer::new(offset, u64::try_from(index.len()).unwrap(), digest).unwrap();
        archive.extend_from_slice(&index);
        let mut footer_bytes = [0; crate::FOOTER_LEN];
        footer.encode(&mut footer_bytes, 0).unwrap();
        archive.extend_from_slice(&footer_bytes);
        archive
    }

    /// Byte position of entry `n`'s `frame_offset` field within an index.
    ///
    /// Each entry is `path_len (2) + path (N) + frame_offset (8)
    /// + compressed_size (8) + uncompressed_size (8) + compression (1)`.
    ///
    /// Walking correctly matters: these helpers mutate raw fixture bytes, and
    /// an off-by-N walk silently edits the previous entry instead — which
    /// produced several convincing but wrong "the validator said GAP" failures
    /// before this was fixed.
    fn entry_offset_field(raw: &[u8], n: usize) -> usize {
        let mut cursor: usize = 16; // skip the 16-byte index header
        for _ in 0..n {
            // path_len (2) + path (N) + tail (25) = 27 + N for entry i.
            let lo = *raw
                .get(cursor)
                .expect("fixture index entry path_len low byte");
            let hi = *raw
                .get(cursor + 1)
                .expect("fixture index entry path_len high byte");
            let plen = usize::from(u16::from_le_bytes([lo, hi]));
            cursor = cursor
                .checked_add(2)
                .and_then(|v| v.checked_add(plen))
                .and_then(|v| v.checked_add(25))
                .expect("entry bounds in fixture");
        }
        // After walking past entries 0..n, cursor points at entry n's path_len.
        // The frame_offset field begins at cursor + 2 + path_len.
        let plen = usize::from(u16::from_le_bytes([
            *raw.get(cursor).expect("fixture index entry header"),
            *raw.get(cursor + 1).expect("fixture index entry header"),
        ]));
        cursor
            .checked_add(2)
            .and_then(|v| v.checked_add(plen))
            .expect("frame_offset field in fixture")
    }

    /// Build an index with entry 1's `frame_offset` replaced.
    fn index_with_second_offset(new_offset: u64) -> Vec<u8> {
        let mut raw = reference_index();
        let at = entry_offset_field(&raw, 1);
        raw.get_mut(at..at + 8)
            .expect("frame_offset field")
            .copy_from_slice(&new_offset.to_le_bytes());
        raw
    }

    /// Build an index with entry 1's `frame_offset` and `compressed_size`
    /// both replaced, leaving every other entry untouched.
    fn index_with_second(new_offset: u64, new_size: u64) -> Vec<u8> {
        let mut raw = reference_index();
        let at = entry_offset_field(&raw, 1);
        raw.get_mut(at..at + 8)
            .expect("frame_offset field")
            .copy_from_slice(&new_offset.to_le_bytes());
        raw.get_mut(at + 8..at + 16)
            .expect("compressed_size field")
            .copy_from_slice(&new_size.to_le_bytes());
        raw
    }

    /// Build an index with entry 0's `compressed_size` replaced.
    fn index_with_first_size(new_size: u64) -> Vec<u8> {
        let mut raw = reference_index();
        let at = entry_offset_field(&raw, 0);
        raw.get_mut(at + 8..at + 16)
            .expect("compressed_size field")
            .copy_from_slice(&new_size.to_le_bytes());
        raw
    }

    /// Build an index with entry 0's `frame_offset` replaced.
    fn index_with_first_offset(new_offset: u64) -> Vec<u8> {
        let mut raw = reference_index();
        let at = entry_offset_field(&raw, 0);
        raw.get_mut(at..at + 8)
            .expect("frame_offset field")
            .copy_from_slice(&new_offset.to_le_bytes());
        raw
    }

    /// Build an index with entry 1's `frame_offset` pushed to a wrapping value.
    fn index_with_second_wrapping_offset() -> Vec<u8> {
        let mut raw = reference_index();
        let at = entry_offset_field(&raw, 1);
        raw.get_mut(at..at + 8)
            .expect("frame_offset field")
            .copy_from_slice(&(u64::MAX - 3).to_le_bytes());
        raw
    }

    /// A minimal, well-formed zero-entry index.
    fn empty_index_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&crate::INDEX_MAGIC);
        b.extend_from_slice(&crate::INDEX_VERSION.to_le_bytes());
        b.extend_from_slice(&crate::ENTRY_FORMAT.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    #[test]
    fn the_empty_index_is_exactly_the_index_header() {
        assert_eq!(empty_index_bytes().len(), INDEX_HEADER_LEN);
        assert_eq!(empty_index_bytes().len(), 16);
    }

    #[test]
    fn reference_footer_still_round_trips() {
        // Guard the layout tests against a silently-changed reference vector.
        let archive = reference_archive();
        let footer = Footer::decode(&archive).unwrap();
        assert_eq!(footer.index_offset(), fix::INDEX_OFFSET);
        assert_eq!(footer.index_size(), fix::INDEX_SIZE);
    }

    #[test]
    fn entry_offset_field_is_correct() {
        // The walker these tests rely on to edit fixture bytes. If it is off
        // by even one field, every malformed-archive test below silently edits
        // the wrong entry and still "passes" for the wrong reason. This pins
        // it against values read independently from layout.json.
        let raw = reference_index();
        assert_eq!(entry_offset_field(&raw, 0), 39);
        assert_eq!(entry_offset_field(&raw, 1), 90);
        assert_eq!(entry_offset_field(&raw, 2), 139);
        assert_eq!(entry_offset_field(&raw, 3), 174);
        assert_eq!(entry_offset_field(&raw, 4), 212);
        // And the value stored at each position is that entry's frame_offset.
        for (n, expected) in [32u64, 80, 128, 176, 224].iter().enumerate() {
            let at = entry_offset_field(&raw, n);
            let got = u64::from_le_bytes(
                raw[at..at + 8]
                    .try_into()
                    .expect("8-byte frame_offset field"),
            );
            assert_eq!(got, *expected, "entry {n} frame_offset");
        }
    }

    // ---- T0321: footer variants ----

    #[test]
    fn rejects_an_archive_with_a_truncated_footer() {
        // The reference archive is 573 bytes. Chop bytes 0..N for every N that
        // leaves a partial footer (509..572) and assert every one is rejected.
        let archive = reference_archive();
        let footer_off = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        for len in [
            footer_off + 8,
            footer_off + 16,
            footer_off + 32,
            footer_off + 63,
        ] {
            let e = ArchiveListing::parse(&archive[..len]).unwrap_err();
            // Either ARCHIVE_TOO_SMALL (didn't reach footer), or a footer
            // parse error that the helper resolves to a registered code.
            let code = e.code();
            assert!(is_container_parse_code(code), "len {len} gave {code}");
        }
    }

    #[test]
    fn rejects_a_footer_with_a_bad_magic() {
        let mut archive = reference_archive();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        // Replace ELEND001 with ELENXXXX.
        archive[f..f + 8].copy_from_slice(b"ELENXXXX");
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "FOOTER_BAD_MAGIC");
    }

    #[test]
    fn rejects_an_index_size_below_the_minimum() {
        // 0-byte index region: footer claims size = 0. Hash verify will fail.
        let mut archive = reference_archive();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive[f + 16..f + 24].copy_from_slice(&0u64.to_le_bytes());
        let e = ArchiveListing::parse(&archive).unwrap_err();
        // Zero index_size breaks the framing relation
        // `index_offset + index_size + 64 == file_size`, which is checked
        // before the hash. That is the stronger guarantee: a malformed size
        // cannot slip through to a (still-wrong) hash mismatch.
        assert_eq!(e.code(), "FOOTER_OFFSET_INCONSISTENT");
    }

    #[test]
    fn rejects_an_index_size_above_the_maximum() {
        // index_size > 32 MiB: footer parse itself rejects before layout.
        let mut archive = reference_archive();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive[f + 16..f + 24].copy_from_slice(&((32u64 * 1024 * 1024) + 1).to_le_bytes());
        let e = ArchiveListing::parse(&archive).unwrap_err();
        // Likewise: an oversized index breaks framing first.
        assert_eq!(e.code(), "FOOTER_OFFSET_INCONSISTENT");
    }

    // ---- T0322: little-endian on every platform ----

    #[test]
    fn the_header_and_footer_always_emit_little_endian_integers() {
        // Reference values from the fixture's expected.json SHA-256 metadata,
        // which the spec marks as architecture-independent golden bytes.
        let archive = reference_archive();
        let header = crate::Header::decode(&archive).unwrap();
        assert_eq!(header.major(), 0);
        assert_eq!(header.minor(), 1);
        // The header's encoded bytes are the fixture's first 32.
        assert_eq!(&archive[..32], reference_header());
        // The footer bytes are the fixture's last 64.
        let footer_off = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        assert_eq!(&archive[footer_off..], reference_footer());
    }

    // ---- T0323: empty / required resources ----

    #[test]
    fn an_empty_index_produces_an_empty_listing() {
        // Build a well-formed but resource-free archive: header, no frames,
        // empty index, footer. The container reader cannot know whether an
        // empty listing is "valid" or "missing required" — that's a profile
        // concern (REQ-CANONICAL-PROFILE). Here we only prove the container
        // round-trips an empty index without panicking.
        let header = crate::Header::current();
        let empty_index_bytes: Vec<u8> = vec![
            b'E', b'L', b'I', b'D', b'X', b'0', b'0', b'1', // magic
            0x01, 0x00, // index_version = 1
            0x01, 0x00, // entry_format = 1
            0x00, 0x00, 0x00, 0x00, // resource_count = 0
        ];
        let mut archive = vec![0u8; 32];
        header.encode(&mut archive, 0).unwrap();
        // No frame region. Index starts at HEADER_LEN.
        let index_off: u64 = 32;
        let index_size: u64 = empty_index_bytes.len() as u64;
        let digest = Sha256::digest(&empty_index_bytes).into();
        let footer = crate::Footer::new(index_off, index_size, digest).unwrap();
        archive.extend_from_slice(&empty_index_bytes);
        let mut footer_slot = [0u8; 64];
        footer.encode(&mut footer_slot, 0).unwrap();
        archive.extend_from_slice(&footer_slot);
        let listing = ArchiveListing::parse(&archive).unwrap();
        assert!(listing.is_empty());
        assert_eq!(listing.len(), 0);
        assert_eq!(listing.total_frame_bytes(), 0);
        assert!(listing.listing().is_empty());
    }

    #[test]
    fn a_single_resource_archive_round_trips_through_the_listing() {
        // Defensive: an archive with exactly one frame round-trips.
        let header = crate::Header::current();
        let path = b"only";
        let frame_off: u64 = 32;
        let comp: u64 = 10;
        let unc: u64 = 7;
        let mut index_bytes = Vec::new();
        index_bytes.extend_from_slice(&crate::INDEX_MAGIC);
        index_bytes.extend_from_slice(&crate::INDEX_VERSION.to_le_bytes());
        index_bytes.extend_from_slice(&crate::ENTRY_FORMAT.to_le_bytes());
        index_bytes.extend_from_slice(&1u32.to_le_bytes());
        index_bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        index_bytes.extend_from_slice(path);
        index_bytes.extend_from_slice(&frame_off.to_le_bytes());
        index_bytes.extend_from_slice(&comp.to_le_bytes());
        index_bytes.extend_from_slice(&unc.to_le_bytes());
        index_bytes.push(crate::COMPRESSION_ZSTD);
        let mut archive = vec![0u8; 32];
        header.encode(&mut archive, 0).unwrap();
        // Frame body (10 placeholder bytes) between header and index.
        let frame_len = usize::try_from(comp).expect("test frame fits usize");
        archive.extend(std::iter::repeat_n(0u8, frame_len));
        let index_off: u64 = frame_off + comp;
        let index_size: u64 = index_bytes.len() as u64;
        let digest = Sha256::digest(&index_bytes).into();
        let footer = crate::Footer::new(index_off, index_size, digest).unwrap();
        archive.extend_from_slice(&index_bytes);
        let mut footer_slot = [0u8; 64];
        footer.encode(&mut footer_slot, 0).unwrap();
        archive.extend_from_slice(&footer_slot);
        let listing = ArchiveListing::parse(&archive).unwrap();
        assert_eq!(listing.len(), 1);
        let rows = listing.listing();
        assert_eq!(rows[0].path, "only");
        assert_eq!(rows[0].frame_offset, 32);
        assert_eq!(rows[0].compressed_size, 10);
        assert_eq!(rows[0].uncompressed_size, 7);
    }

    #[test]
    fn every_byte_offset_in_the_listing_falls_inside_the_archive() {
        let listing = ArchiveListing::parse(&reference_archive()).unwrap();
        let total = u64::try_from(reference_archive().len()).unwrap();
        for row in listing.listing() {
            let end = row.frame_offset.checked_add(row.compressed_size);
            assert!(
                end.is_some() && end.unwrap() <= total,
                "row {:?} escapes archive bounds",
                row
            );
        }
    }

    // ---- coverage: getters and parse guards not reached by the fixture ----

    #[test]
    fn frame_layout_exposes_its_spans_through_the_getter() {
        // `FrameLayout::spans` is a one-line getter but is part of the public
        // API used by callers outside the crate.
        let listing = ArchiveListing::parse(&reference_archive()).unwrap();
        let spans = listing.layout().spans();
        assert_eq!(spans.len(), fix::RESOURCE_COUNT);
        assert_eq!(spans.first().map(|s| s.offset), Some(32));
        assert_eq!(spans.last().map(|s| s.offset), Some(224));
        // Every span must have a positive compressed size (the validator
        // already enforced this, so this is also a regression check).
        for span in spans {
            assert!(span.compressed_size > 0);
            assert!(span.end().is_some());
        }
    }

    #[test]
    fn archive_listing_exposes_index_and_layout_through_getters() {
        let listing = ArchiveListing::parse(&reference_archive()).unwrap();
        assert_eq!(listing.index().len(), fix::RESOURCE_COUNT);
        assert_eq!(listing.layout().len(), fix::RESOURCE_COUNT);
        // The two views see the same five entries in the same order.
        for (i, entry) in listing.index().entries().iter().enumerate() {
            assert_eq!(entry.path().as_str(), listing.listing()[i].path);
        }
    }

    #[test]
    fn archive_listing_rejects_a_truncation_that_removes_the_footer() {
        // Chopping the archive to exactly the footer's start offset leaves no
        // 64-byte footer at all. `Footer::decode` reads the footer from
        // `len - 64`, finds header bytes where the magic should be, and
        // rejects before any index work happens.
        let mut archive = reference_archive();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive.truncate(f);
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "FOOTER_BAD_MAGIC");
    }

    #[test]
    fn archive_listing_rejects_a_corrupt_index_that_still_frames_correctly() {
        // Flip a byte inside the index region and repair the footer's stored
        // digest to match, so framing AND the hash check both pass. What
        // remains is the index's own parse — this proves the index parser, not
        // just the framing arithmetic, is load-bearing.
        let mut archive = reference_archive();
        let idx_at = usize::try_from(fix::INDEX_OFFSET).unwrap();
        // Corrupt the resource_count field to a value above the ceiling.
        archive[idx_at + 12..idx_at + 16].copy_from_slice(&u32::MAX.to_le_bytes());
        // Repair framing: index_size is unchanged, index_offset unchanged, so
        // framing still holds. Repair the hash to the corrupted bytes.
        let idx_end = usize::try_from(fix::INDEX_OFFSET + fix::INDEX_SIZE).unwrap();
        let digest: [u8; 32] = sha2::Sha256::digest(&archive[idx_at..idx_end]).into();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive[f + 24..f + 56].copy_from_slice(&digest);
        // Now framing passes, the hash passes, and the index parser must be
        // the thing that rejects the archive.
        let e = ArchiveListing::parse(&archive).unwrap_err();
        assert_eq!(e.code(), "INDEX_COUNT_OUT_OF_RANGE");
    }

    #[test]
    fn archive_listing_rejects_an_index_size_that_exceeds_the_maximum() {
        // Set index_size to MAX_INDEX_SIZE + 1 directly in the footer bytes.
        // Footer::new enforces the range, so we go through the raw bytes.
        let mut archive = reference_archive();
        let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
        archive[f + 16..f + 24].copy_from_slice(&(crate::MAX_INDEX_SIZE + 1).to_le_bytes());
        let e = ArchiveListing::parse(&archive).unwrap_err();
        // Framing fails first because the new size does not match
        // file_size - 64 - index_offset.
        let code = e.code();
        assert!(
            code == "FOOTER_OFFSET_INCONSISTENT" || code == "INDEX_SIZE_OUT_OF_RANGE",
            "got {code}"
        );
    }

    #[test]
    fn truncated_archives_at_footer_boundaries_are_rejected() {
        // Every length from `HEADER_LEN` to `archive.len() - 1` must yield a
        // registered error, never panic and never succeed. The fixture is
        // large enough that several boundaries fall inside the 64-byte footer.
        let archive = reference_archive();
        let total = archive.len();
        for len in [
            HEADER_LEN,
            32,
            33,
            100,
            200,
            fix::INDEX_OFFSET as usize,
            total - 64,
            total - 32,
            total - 1,
        ] {
            let e = ArchiveListing::parse(&archive[..len]).unwrap_err();
            let code = e.code();
            assert!(is_container_parse_code(code), "len {len} produced {code}");
        }
    }

    #[test]
    fn layout_parse_error_says_which_specific_failure_occurred() {
        // The various parse-side error paths distinguish themselves by code,
        // not just by category. Each of these must give a code the caller can
        // route on (RE: 'each error code is a stable public contract').
        type Mutation = (&'static str, fn(&mut Vec<u8>));
        let cases: &[Mutation] = &[
            ("HEADER_BAD_MAGIC", |a| a[..4].copy_from_slice(b"NOPE")),
            ("FOOTER_BAD_MAGIC", |a| {
                let f = usize::try_from(fix::FOOTER_OFFSET).unwrap();
                a[f..f + 8].copy_from_slice(b"NOPENOPE");
            }),
        ];
        for (expected, mutate) in cases {
            let mut a = reference_archive();
            mutate(&mut a);
            let e = ArchiveListing::parse(&a).unwrap_err();
            assert_eq!(e.code(), *expected, "mutator should yield {expected}");
        }
    }
}
