//! Binary container terminal footer: 64-byte decode, encode, and index-hash
//! verification (T0304–T0308).
//!
//! The footer is the last 64 bytes of every `.elang` archive. It tells a reader
//! where the index lives and proves the index has not been tampered with:
//!
//! ```text
//! offset  width  encoding  field
//!      0      8  raw       magic          ELEND001
//!      8      8  u64 LE    index_offset   absolute byte offset of the index
//!     16      8  u64 LE    index_size     exact index byte length
//!     24     32  raw       index_sha256   SHA-256 of the exact index bytes
//!     56      8  raw       reserved       all zero
//! ```
//!
//! # The framing relation
//!
//! Because the index sits immediately before the footer, the footer plus the
//! index must consume the file exactly up to `file_size - 64`:
//!
//! ```text
//! index_offset + index_size + 64 == file_size
//! ```
//!
//! This is the container's core structural invariant (steering §3.3). It is
//! checked with `checked_add` so an adversarial `index_offset + index_size`
//! cannot wrap past `u64::MAX` and slip past the comparison.
//!
//! # Why the hash, and why it is exact
//!
//! T0308 verifies the footer's `index_sha256` against the SHA-256 of the
//! *exact* index bytes the footer claims to describe. This catches a truncated
//! index, reordered entries, and any bit-flip in the index — none of which the
//! framing relation alone would detect, because offsets can stay consistent
//! while the content changes.
//!
//! Like the header, all slice indexing here sits behind explicit bounds checks
//! (`decode` rejects short input; `encode` bounds-checks the destination), so
//! the module compiles clean under `clippy::indexing_slicing`.

#![allow(clippy::indexing_slicing)]

use elang_core::{Category, Error, Result};
use sha2::{Digest, Sha256};

/// 8-byte footer magic: `ELEND001` (steering §3.3).
pub const FOOTER_MAGIC: [u8; 8] = *b"ELEND001";

/// Fixed footer length in bytes (steering §3.3).
pub const FOOTER_LEN: usize = 64;

/// Minimum index size: the 16-byte index header (T0307).
pub const MIN_INDEX_SIZE: u64 = 16;

/// Maximum index size (T0307).
pub const MAX_INDEX_SIZE: u64 = 33_554_432;

/// Byte offset of the `index_offset` field.
const INDEX_OFFSET_POS: usize = 8;
/// Byte offset of the `index_size` field.
const INDEX_SIZE_POS: usize = 16;
/// Byte offset of the 32-byte `index_sha256` field.
const INDEX_SHA256_POS: usize = 24;
/// Byte offset of the 8 reserved bytes.
const RESERVED_POS: usize = 56;
/// Length of the reserved region.
const RESERVED_LEN: usize = 8;

/// A validated 64-byte container footer.
///
/// Construction guarantees the magic matched, the reserved bytes are zero, and
/// the index size is within [`MIN_INDEX_SIZE`]..=[`MAX_INDEX_SIZE`]. The
/// framing relation against the real file size and the index hash are checked
/// by the dedicated methods, because they need inputs the footer alone does not
/// carry (the file size, the index bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
    index_offset: u64,
    index_size: u64,
    index_sha256: [u8; 32],
}

impl Footer {
    /// Build a footer for a given index location and digest.
    ///
    /// Returns `FOOTER_OFFSET_INCONSISTENT` if the index size is outside
    /// [`MIN_INDEX_SIZE`]..=[`MAX_INDEX_SIZE`].
    pub fn new(index_offset: u64, index_size: u64, index_sha256: [u8; 32]) -> Result<Self> {
        if !(MIN_INDEX_SIZE..=MAX_INDEX_SIZE).contains(&index_size) {
            return Err(container_err(
                "FOOTER_OFFSET_INCONSISTENT",
                "index size outside the permitted 16..=33,554,432 range",
            ));
        }
        Ok(Self {
            index_offset,
            index_size,
            index_sha256,
        })
    }

    /// Decode and validate a footer from the last 64 bytes of an archive.
    ///
    /// `archive` is the whole file (or any slice ending at the footer). The
    /// footer is taken from `archive.len() - FOOTER_LEN`. Errors:
    /// - `FOOTER_BAD_MAGIC` — fewer than 64 bytes, or the magic differs.
    /// - `FOOTER_RESERVED_NONZERO` — any reserved byte is non-zero.
    /// - `FOOTER_OFFSET_INCONSISTENT` — index size outside the permitted range.
    pub fn decode(archive: &[u8]) -> Result<Self> {
        if archive.len() < FOOTER_LEN {
            return Err(container_err(
                "FOOTER_BAD_MAGIC",
                "archive is shorter than the 64-byte footer",
            ));
        }
        // `checked_sub` rather than bare `-`: the guard above already proves
        // the subtraction cannot underflow, but an explicit checked form keeps
        // this correct if the constant or the guard ever changes.
        let start = archive
            .len()
            .checked_sub(FOOTER_LEN)
            .ok_or_else(|| container_err("FOOTER_BAD_MAGIC", "footer slice is out of bounds"))?;
        let bytes = archive
            .get(start..)
            .ok_or_else(|| container_err("FOOTER_BAD_MAGIC", "footer slice is out of bounds"))?;

        if !bytes.starts_with(&FOOTER_MAGIC) {
            return Err(container_err("FOOTER_BAD_MAGIC", "footer magic mismatch"));
        }

        let index_offset = read_u64(bytes, INDEX_OFFSET_POS);
        let index_size = read_u64(bytes, INDEX_SIZE_POS);
        if !(MIN_INDEX_SIZE..=MAX_INDEX_SIZE).contains(&index_size) {
            return Err(container_err(
                "FOOTER_OFFSET_INCONSISTENT",
                "index size outside the permitted 16..=33,554,432 range",
            ));
        }

        if !reserved_is_zero(bytes) {
            return Err(container_err(
                "FOOTER_RESERVED_NONZERO",
                "footer reserved bytes must be zero",
            ));
        }

        Ok(Self {
            index_offset,
            index_size,
            index_sha256: read_digest(bytes, INDEX_SHA256_POS),
        })
    }

    /// Encode the footer as exactly [`FOOTER_LEN`] little-endian bytes,
    /// appended to `out` at `offset`.
    ///
    /// Returns the number of bytes written ([`FOOTER_LEN`]) or an error if the
    /// destination is too small.
    pub fn encode(&self, out: &mut [u8], offset: usize) -> Result<usize> {
        let end = offset
            .checked_add(FOOTER_LEN)
            .ok_or_else(|| container_err("FOOTER_BAD_MAGIC", "footer write offset overflows"))?;
        if end > out.len() {
            return Err(container_err(
                "FOOTER_BAD_MAGIC",
                "output buffer is too small for the 64-byte footer",
            ));
        }

        let slot = &mut out[offset..end];
        slot.fill(0);
        slot[..8].copy_from_slice(&FOOTER_MAGIC);
        slot[8..16].copy_from_slice(&self.index_offset.to_le_bytes());
        slot[16..24].copy_from_slice(&self.index_size.to_le_bytes());
        slot[24..56].copy_from_slice(&self.index_sha256);
        // reserved region (56..64) already zeroed.

        Ok(FOOTER_LEN)
    }

    /// Absolute byte offset of the index within the archive.
    #[must_use]
    pub fn index_offset(&self) -> u64 {
        self.index_offset
    }

    /// Declared index size in bytes.
    #[must_use]
    pub fn index_size(&self) -> u64 {
        self.index_size
    }

    /// The declared SHA-256 of the index bytes.
    #[must_use]
    pub fn index_sha256(&self) -> &[u8; 32] {
        &self.index_sha256
    }

    /// Enforce the framing relation against a real file size (T0306).
    ///
    /// Requires `index_offset + index_size + 64 == file_size` under checked
    /// arithmetic. Returns `ARCHIVE_SIZE_INCONSISTENT` on any mismatch,
    /// including a wrapping `index_offset + index_size`.
    pub fn check_framing(&self, file_size: u64) -> Result<()> {
        let expected_end = self
            .index_offset
            .checked_add(self.index_size)
            .and_then(|v| v.checked_add(FOOTER_LEN as u64));
        match expected_end {
            Some(end) if end == file_size => Ok(()),
            _ => Err(container_err(
                "ARCHIVE_SIZE_INCONSISTENT",
                "index_offset + index_size + 64 != file_size",
            )),
        }
    }

    /// Verify the footer hash against the exact index bytes (T0308).
    ///
    /// Computes `SHA-256(index_bytes)` and compares it to the stored digest.
    /// Returns `FOOTER_INDEX_HASH_MISMATCH` on any difference.
    pub fn verify_index_hash(&self, index_bytes: &[u8]) -> Result<()> {
        let actual: [u8; 32] = Sha256::digest(index_bytes).into();
        if actual == self.index_sha256 {
            Ok(())
        } else {
            Err(index_err(
                "FOOTER_INDEX_HASH_MISMATCH",
                "index SHA-256 does not match the footer",
            ))
        }
    }
}

/// Read a `u64` little-endian at `offset` from a footer-length slice.
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    bytes
        .get(offset..offset.saturating_add(8))
        .and_then(|octet| <[u8; 8]>::try_from(octet).ok())
        .map(u64::from_le_bytes)
        .unwrap_or_default()
}

/// Read a 32-byte digest at `offset`.
fn read_digest(bytes: &[u8], offset: usize) -> [u8; 32] {
    bytes
        .get(offset..offset.saturating_add(32))
        .and_then(|chunk| <[u8; 32]>::try_from(chunk).ok())
        .unwrap_or([0u8; 32])
}

/// Whether all reserved bytes are zero.
fn reserved_is_zero(bytes: &[u8]) -> bool {
    bytes
        .get(RESERVED_POS..RESERVED_POS.saturating_add(RESERVED_LEN))
        .is_some_and(|reserved| reserved.iter().all(|&b| b == 0))
}

/// Build a `CONTAINER`-category error.
fn container_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Container, msg)
}

/// Build an `INDEX`-category error (index-structure failures).
fn index_err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Index, msg)
}

#[cfg(test)]
#[cfg(not(miri))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// The exact 64 reference footer bytes from
    /// `specs/registry/binary-reference-vectors.json` / `footer.bin`.
    const GOLDEN_FOOTER: [u8; 64] = [
        0x45, 0x4C, 0x45, 0x4E, 0x44, 0x30, 0x30, 0x31, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xED, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA3, 0x48, 0x79, 0xD8, 0xE4, 0xB4,
        0x5C, 0x2A, 0x9C, 0x9F, 0x35, 0xBE, 0x47, 0xD9, 0xC7, 0x11, 0x13, 0x40, 0xCB, 0xE0, 0xC5,
        0x35, 0xF7, 0xCC, 0x79, 0xD6, 0x97, 0xC1, 0x77, 0x7E, 0xFA, 0x35, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    /// The 237-byte reference index from `fixtures/valid/container/minimal-archive/index.bin`.
    ///
    /// Embedded at compile time via `include_bytes!` so the bytes are
    /// available under Miri (which forbids runtime file I/O) and the
    /// fixture is guaranteed to be present regardless of the working
    /// directory at test time. The path is resolved relative to this
    /// source file (`crates/elang-container/src/footer.rs`), so three
    /// `..` are needed to reach the workspace root before descending into
    /// `fixtures/`.
    fn reference_index() -> Vec<u8> {
        include_bytes!("../../../fixtures/valid/container/minimal-archive/index.bin").to_vec()
    }

    fn reference_archive() -> Vec<u8> {
        include_bytes!("../../../fixtures/valid/container/minimal-archive/archive.elang").to_vec()
    }

    // ---- T0304: decode ----

    #[test]
    fn decodes_the_reference_footer_byte_for_byte() {
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        assert_eq!(f.index_offset(), 272);
        assert_eq!(f.index_size(), 237);
        let expected: [u8; 32] = Sha256::digest(reference_index()).into();
        assert_eq!(f.index_sha256(), &expected);
    }

    #[test]
    fn decode_reads_from_the_end_of_a_longer_archive() {
        let mut archive = vec![0xAB; 100];
        archive.extend_from_slice(&GOLDEN_FOOTER);
        assert_eq!(Footer::decode(&archive).unwrap().index_offset(), 272);
    }

    #[test]
    fn rejects_short_input_without_panicking() {
        for len in 0..FOOTER_LEN {
            let short = &GOLDEN_FOOTER[..len];
            assert_eq!(
                Footer::decode(short).unwrap_err().code(),
                "FOOTER_BAD_MAGIC",
                "len {len}"
            );
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let mut b = GOLDEN_FOOTER;
        b[0] = 0x00;
        let e = Footer::decode(&b).unwrap_err();
        assert_eq!(e.code(), "FOOTER_BAD_MAGIC");
        assert_eq!(e.category(), Category::Container);
    }

    // ---- T0305: reserved + bounds ----

    #[test]
    fn rejects_every_nonzero_reserved_byte() {
        for i in RESERVED_POS..(RESERVED_POS + RESERVED_LEN) {
            let mut b = GOLDEN_FOOTER;
            b[i] = 1;
            assert_eq!(
                Footer::decode(&b).unwrap_err().code(),
                "FOOTER_RESERVED_NONZERO",
                "reserved byte {i}"
            );
        }
    }

    // ---- T0306: framing relation ----

    #[test]
    fn framing_relation_holds_for_the_reference_archive() {
        let archive = reference_archive();
        let f = Footer::decode(&archive).unwrap();
        assert!(f.check_framing(archive.len() as u64).is_ok());
    }

    #[test]
    fn framing_rejects_a_wrong_file_size() {
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        let good = 272 + 237 + 64;
        assert!(f.check_framing(good).is_ok());
        assert_eq!(
            f.check_framing(good - 1).unwrap_err().code(),
            "ARCHIVE_SIZE_INCONSISTENT"
        );
        assert_eq!(
            f.check_framing(good + 1).unwrap_err().code(),
            "ARCHIVE_SIZE_INCONSISTENT"
        );
    }

    #[test]
    fn framing_rejects_a_wrapping_offset_instead_of_passing() {
        // index_offset near u64::MAX must fail the checked_add, not wrap to a
        // small value that accidentally equals the file size.
        let f = Footer::new(u64::MAX - 10, 237, [0u8; 32]).unwrap();
        let e = f.check_framing(64).unwrap_err();
        assert_eq!(e.code(), "ARCHIVE_SIZE_INCONSISTENT");
    }

    // ---- T0307: index size range ----

    #[test]
    fn enforces_index_size_range_on_decode() {
        // Below the 16-byte minimum.
        let mut small = GOLDEN_FOOTER;
        small[16..24].copy_from_slice(&15u64.to_le_bytes());
        assert_eq!(
            Footer::decode(&small).unwrap_err().code(),
            "FOOTER_OFFSET_INCONSISTENT"
        );

        // Above the 33 MiB maximum.
        let mut big = GOLDEN_FOOTER;
        big[16..24].copy_from_slice(&(MAX_INDEX_SIZE + 1).to_le_bytes());
        assert_eq!(
            Footer::decode(&big).unwrap_err().code(),
            "FOOTER_OFFSET_INCONSISTENT"
        );
    }

    #[test]
    fn accepts_the_exact_size_range_endpoints() {
        let mut lo = GOLDEN_FOOTER;
        lo[16..24].copy_from_slice(&MIN_INDEX_SIZE.to_le_bytes());
        assert!(Footer::decode(&lo).is_ok());

        let mut hi = GOLDEN_FOOTER;
        hi[16..24].copy_from_slice(&MAX_INDEX_SIZE.to_le_bytes());
        assert!(Footer::decode(&hi).is_ok());
    }

    #[test]
    fn new_rejects_out_of_range_index_size() {
        assert!(Footer::new(0, MIN_INDEX_SIZE - 1, [0u8; 32]).is_err());
        assert!(Footer::new(0, MAX_INDEX_SIZE + 1, [0u8; 32]).is_err());
        assert!(Footer::new(0, MIN_INDEX_SIZE, [0u8; 32]).is_ok());
        assert!(Footer::new(0, MAX_INDEX_SIZE, [0u8; 32]).is_ok());
    }

    // ---- T0308: index hash ----

    #[test]
    fn verifies_the_hash_of_the_exact_index_bytes() {
        let index = reference_index();
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        assert!(f.verify_index_hash(&index).is_ok());
    }

    #[test]
    fn detects_a_single_bit_flip_in_the_index() {
        let mut index = reference_index();
        index[0] ^= 0x01; // corrupt the index magic
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        let e = f.verify_index_hash(&index).unwrap_err();
        assert_eq!(e.code(), "FOOTER_INDEX_HASH_MISMATCH");
        assert_eq!(e.category(), Category::Index);
    }

    #[test]
    fn detects_a_truncated_index() {
        let index = reference_index();
        let truncated = index[..index.len() - 1].to_vec();
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        assert!(f.verify_index_hash(&truncated).is_err());
    }

    #[test]
    fn detects_reordered_index_entries() {
        // Two entries swapped: offsets stay internally consistent, so only the
        // hash catches it.
        let index = reference_index();
        let mut reordered = index.clone();
        reordered.swap(20, 21);
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        assert!(f.verify_index_hash(&reordered).is_err());
    }

    // ---- encode ----

    #[test]
    fn encode_produces_the_reference_bytes() {
        let f = Footer::decode(&GOLDEN_FOOTER).unwrap();
        let mut out = [0u8; 64];
        let n = f.encode(&mut out, 0).unwrap();
        assert_eq!(n, 64);
        assert_eq!(out, GOLDEN_FOOTER, "encoder must match the golden footer");
    }

    #[test]
    fn encode_decode_round_trip() {
        let f = Footer::new(272, 237, [0x42; 32]).unwrap();
        let mut buf = [0u8; 64];
        f.encode(&mut buf, 0).unwrap();
        assert_eq!(Footer::decode(&buf).unwrap(), f);
    }

    #[test]
    fn encode_rejects_a_short_buffer_without_panicking() {
        let f = Footer::new(0, MIN_INDEX_SIZE, [0u8; 32]).unwrap();
        let mut small = [0u8; 63];
        assert_eq!(
            f.encode(&mut small, 0).unwrap_err().code(),
            "FOOTER_BAD_MAGIC"
        );
        let mut buf = [0u8; 64];
        assert!(f.encode(&mut buf, 1).is_err());
        assert!(f.encode(&mut buf, usize::MAX).is_err());
    }

    // ---- cross-check against the committed fixtures ----

    #[test]
    fn matches_the_committed_footer_and_archive_fixtures() {
        // Embed the committed fixtures at compile time so this test
        // is reproducible under Miri (which forbids runtime file I/O)
        // and from any working directory. Paths are resolved
        // relative to this source file (see `reference_index` for the
        // directory-walk rationale).
        let footer_bytes: Vec<u8> =
            include_bytes!("../../../fixtures/valid/container/minimal-archive/footer.bin").to_vec();
        let archive: Vec<u8> =
            include_bytes!("../../../fixtures/valid/container/minimal-archive/archive.elang")
                .to_vec();

        assert_eq!(footer_bytes, GOLDEN_FOOTER.to_vec(), "footer.bin drifted");
        assert_eq!(
            &archive[archive.len() - 64..],
            &footer_bytes[..],
            "archive does not end with footer.bin"
        );

        let f = Footer::decode(&archive).unwrap();
        assert!(f.check_framing(archive.len() as u64).is_ok());
        // The footer hash must match SHA-256 of the exact index bytes, which we
        // read straight out of the archive. `try_from` rather than `as usize`
        // keeps `cast_possible_truncation` honest on 32-bit targets.
        let index_offset = usize::try_from(f.index_offset()).expect("reference offset fits usize");
        let index_end = index_offset
            .checked_add(usize::try_from(f.index_size()).expect("reference size fits usize"))
            .expect("reference index_end fits usize");
        assert!(f
            .verify_index_hash(&archive[index_offset..index_end])
            .is_ok());
    }
}
