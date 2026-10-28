//! Binary container header: 32-byte decode and encode (T0301–T0303).
//!
//! The header is the first thing a reader touches and the first thing an
//! attacker controls, so every field is validated before any other container
//! structure is parsed. Layout (steering §3.1):
//!
//! ```text
//! offset  width  encoding  field
//!      0      8  raw       magic          ELANG\r\n\x1A
//!      8      2  u16 LE    major          must be 0
//!     10      2  u16 LE    minor          must be 1
//!     12      4  u32 LE    flags          must be 0
//!     16     16  raw       reserved       all zero
//! ```
//!
//! [`Header::decode`] takes any slice and never panics, indexes out of bounds,
//! or panics on short input: a short slice is `HEADER_BAD_MAGIC`, not a crash.
//! [`Header::encode`] writes the exact little-endian bytes and is the inverse
//! of `decode` for the version this build supports.
//!
//! Only binary `0.1` is accepted (REQ-CORE-VERSION-004). A different `minor`
//! is rejected rather than "best-effort parsed": the reserved bytes and flag
//! semantics are version-specific, so accepting an unknown version would let
//! an attacker hand us bytes we misinterpret as valid.
//
// `indexing_slicing` is denied at the workspace level for untrusted-input
// safety. Every slice index inside this module sits behind an explicit length
// check — `Header::decode` rejects input shorter than [`HEADER_LEN`] before
// any byte read, and `Header::encode` bounds-checks the destination buffer
// before any byte write. The indexing is therefore provably in-bounds even
// though clippy cannot prove it statically.
#![allow(clippy::indexing_slicing)]

use elang_core::{Category, Error, Result};

/// 8-byte header magic: `ELANG\r\n\x1A` (steering §3.1).
pub const HEADER_MAGIC: [u8; 8] = [0x45, 0x4C, 0x41, 0x4E, 0x47, 0x0D, 0x0A, 0x1A];

/// Fixed header length in bytes (steering §3.1).
pub const HEADER_LEN: usize = 32;

/// Byte offset of the `major` field.
const MAJOR_OFFSET: usize = 8;
/// Byte offset of the `minor` field.
const MINOR_OFFSET: usize = 10;
/// Byte offset of the `flags` field.
const FLAGS_OFFSET: usize = 12;
/// Byte offset of the 16 reserved bytes.
const RESERVED_OFFSET: usize = 16;
/// Length of the reserved region.
const RESERVED_LEN: usize = 16;

/// The only supported binary major version (REQ-CORE-VERSION-004).
pub const SUPPORTED_MAJOR: u16 = 0;
/// The only supported binary minor version (REQ-CORE-VERSION-004).
pub const SUPPORTED_MINOR: u16 = 1;

/// A validated 32-byte container header.
///
/// Constructing one guarantees the magic matched, `major`/`minor` are the
/// supported binary version, `flags == 0`, and all 16 reserved bytes are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    major: u16,
    minor: u16,
    flags: u32,
}

impl Header {
    /// The header for the single supported binary version (`0.1`, no flags).
    #[must_use]
    pub fn current() -> Self {
        Self {
            major: SUPPORTED_MAJOR,
            minor: SUPPORTED_MINOR,
            flags: 0,
        }
    }

    /// Decode and fully validate a header from the start of `bytes`.
    ///
    /// Requires at least [`HEADER_LEN`] bytes. Errors:
    /// - `HEADER_BAD_MAGIC` — fewer than 32 bytes, or the 8-byte magic differs.
    /// - `HEADER_BAD_VERSION` — `major`/`minor` is not binary `0.1`.
    /// - `HEADER_NONZERO_FLAGS` — any flag bit is set (none are defined in 0.1).
    /// - `HEADER_RESERVED_NONZERO` — any of the 16 reserved bytes is non-zero.
    ///
    /// Validation order is deliberate: magic first (cheap reject of
    /// non-containers), then version (so flag/reserved meanings are known),
    /// then flags, then reserved. A caller therefore never interprets
    /// version-specific fields it does not understand.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(err(
                "HEADER_BAD_MAGIC",
                "archive is shorter than the 32-byte header",
            ));
        }
        if !bytes.starts_with(&HEADER_MAGIC) {
            return Err(err("HEADER_BAD_MAGIC", "header magic mismatch"));
        }

        let major = read_u16(bytes, MAJOR_OFFSET);
        let minor = read_u16(bytes, MINOR_OFFSET);
        if major != SUPPORTED_MAJOR || minor != SUPPORTED_MINOR {
            return Err(err(
                "HEADER_BAD_VERSION",
                "unsupported binary container version",
            ));
        }

        let flags = read_u32(bytes, FLAGS_OFFSET);
        if flags != 0 {
            return Err(err(
                "HEADER_NONZERO_FLAGS",
                "header flags must be zero in binary 0.1",
            ));
        }

        if !reserved_is_zero(bytes) {
            return Err(err(
                "HEADER_RESERVED_NONZERO",
                "header reserved bytes must be zero",
            ));
        }

        Ok(Self {
            major,
            minor,
            flags,
        })
    }

    /// Encode the header as exactly [`HEADER_LEN`] little-endian bytes.
    ///
    /// Writes into `out` at `offset`. Returns the number of bytes written
    /// (always [`HEADER_LEN`]) or an error if `out` is too short — no panic,
    /// no partial silent write.
    pub fn encode(&self, out: &mut [u8], offset: usize) -> Result<usize> {
        let end = offset
            .checked_add(HEADER_LEN)
            .ok_or_else(|| err("HEADER_BAD_MAGIC", "header write offset overflows"))?;
        if end > out.len() {
            return Err(err(
                "HEADER_BAD_MAGIC",
                "output buffer is too small for the 32-byte header",
            ));
        }

        let slot = &mut out[offset..end];
        slot.fill(0);
        slot[..8].copy_from_slice(&HEADER_MAGIC);
        slot[8..10].copy_from_slice(&self.major.to_le_bytes());
        slot[10..12].copy_from_slice(&self.minor.to_le_bytes());
        slot[12..16].copy_from_slice(&self.flags.to_le_bytes());
        // reserved region (16..32) already zeroed by the fill above.

        Ok(HEADER_LEN)
    }

    /// Binary major version (always [`SUPPORTED_MAJOR`] for a decoded header).
    #[must_use]
    pub fn major(&self) -> u16 {
        self.major
    }

    /// Binary minor version (always [`SUPPORTED_MINOR`] for a decoded header).
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.minor
    }

    /// Header flags (always `0` for a decoded header).
    #[must_use]
    pub fn flags(&self) -> u32 {
        self.flags
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::current()
    }
}

/// Read a `u16` little-endian at `offset`.
///
/// Uses `saturating_add` and `try_from` rather than unchecked `offset + 2` so
/// this compiles clean under `arithmetic_side_effects`. A short slice yields
/// `0`; callers have already length-checked, so that path is unreachable in
/// practice and can never panic either way.
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|pair| <[u8; 2]>::try_from(pair).ok())
        .map(u16::from_le_bytes)
        .unwrap_or_default()
}

/// Read a `u32` little-endian at `offset`. See [`read_u16`] for the rationale.
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|quad| <[u8; 4]>::try_from(quad).ok())
        .map(u32::from_le_bytes)
        .unwrap_or_default()
}

/// Whether all 16 reserved bytes are zero.
fn reserved_is_zero(bytes: &[u8]) -> bool {
    bytes
        .get(RESERVED_OFFSET..RESERVED_OFFSET.saturating_add(RESERVED_LEN))
        .is_some_and(|reserved| reserved.iter().all(|&b| b == 0))
}

/// Build a `CONTAINER`-category error.
fn err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::Container, msg)
}

#[cfg(test)]
#[cfg(not(miri))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::testdata::reference_header;

    /// The exact 32 reference header bytes from
    /// `specs/registry/binary-reference-vectors.json` / `header.bin`.
    const GOLDEN: [u8; 32] = [
        0x45, 0x4C, 0x41, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];

    // ---- T0301: decode ----

    #[test]
    fn decodes_the_reference_header_byte_for_byte() {
        let h = Header::decode(&GOLDEN).unwrap();
        assert_eq!(h.major(), 0);
        assert_eq!(h.minor(), 1);
        assert_eq!(h.flags(), 0);
        assert_eq!(h, Header::current());
    }

    #[test]
    fn decode_ignores_bytes_after_the_header() {
        // A whole archive starts with the header; decode must not care about
        // the remainder.
        let mut archive = GOLDEN.to_vec();
        archive.extend_from_slice(&[0xAB; 128]);
        assert_eq!(Header::decode(&archive).unwrap(), Header::current());
    }

    #[test]
    fn rejects_short_input_without_panicking() {
        for len in 0..HEADER_LEN {
            let short = &GOLDEN[..len];
            let e = Header::decode(short).unwrap_err();
            assert_eq!(e.code(), "HEADER_BAD_MAGIC", "len {len}");
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let mut b = GOLDEN;
        b[0] = 0x00; // corrupt first magic byte
        assert_eq!(Header::decode(&b).unwrap_err().code(), "HEADER_BAD_MAGIC");

        let mut b2 = GOLDEN;
        b2[7] = 0x00; // corrupt the \x1A terminator byte
        assert_eq!(Header::decode(&b2).unwrap_err().code(), "HEADER_BAD_MAGIC");
    }

    // ---- T0302: enforce version / flags / reserved ----

    #[test]
    fn rejects_unsupported_major() {
        let mut b = GOLDEN;
        b[8] = 1; // major = 1
        let e = Header::decode(&b).unwrap_err();
        assert_eq!(e.code(), "HEADER_BAD_VERSION");
        assert_eq!(e.category(), Category::Container);
    }

    #[test]
    fn rejects_unsupported_minor() {
        // minor is `u16 LE` at offset 10, so the high byte lives at byte 10
        // and the low byte at byte 11. `b[11] = 2` would set the low byte
        // and leave the high byte (0x01) intact, giving 0x0102 — which is
        // also rejected, but for the wrong reason.
        let mut hi = GOLDEN;
        hi[10] = 0; // high byte → major still 0, minor high = 0
        hi[11] = 2; // low byte  → minor = 2
        let e = Header::decode(&hi).unwrap_err();
        assert_eq!(e.code(), "HEADER_BAD_VERSION");

        let mut zero = GOLDEN;
        zero[10] = 0; // high byte = 0
        zero[11] = 0; // low byte  = 0 → minor = 0
        let e0 = Header::decode(&zero).unwrap_err();
        assert_eq!(e0.code(), "HEADER_BAD_VERSION");
    }

    #[test]
    fn rejects_nonzero_flags() {
        let mut b = GOLDEN;
        b[12] = 1; // lowest flag bit
        let e = Header::decode(&b).unwrap_err();
        assert_eq!(e.code(), "HEADER_NONZERO_FLAGS");
        assert_eq!(e.category(), Category::Container);
    }

    #[test]
    fn rejects_every_nonzero_reserved_byte() {
        // Any single non-zero reserved byte must fail, at every position.
        for i in RESERVED_OFFSET..(RESERVED_OFFSET + RESERVED_LEN) {
            let mut b = GOLDEN;
            b[i] = 1;
            let e = Header::decode(&b).unwrap_err();
            assert_eq!(e.code(), "HEADER_RESERVED_NONZERO", "reserved byte {i}");
        }
    }

    #[test]
    fn version_is_checked_before_flags_and_reserved() {
        // An archive that is wrong in several ways at once must report the
        // version problem first, so we never interpret fields whose meaning
        // depends on an unknown version.
        let mut b = GOLDEN;
        b[8] = 9; // major = 9
        b[12] = 0xFF; // flags set
        b[20] = 0xFF; // reserved dirty
        assert_eq!(Header::decode(&b).unwrap_err().code(), "HEADER_BAD_VERSION");
    }

    // ---- T0303: encode ----

    #[test]
    fn encode_produces_the_reference_bytes() {
        let h = Header::current();
        let mut out = [0u8; 32];
        let n = h.encode(&mut out, 0).unwrap();
        assert_eq!(n, 32);
        assert_eq!(out, GOLDEN, "encoder must match the golden header exactly");
    }

    #[test]
    fn encode_decode_round_trip() {
        let h = Header::current();
        let mut buf = [0u8; 32];
        h.encode(&mut buf, 0).unwrap();
        assert_eq!(Header::decode(&buf).unwrap(), h);
    }

    #[test]
    fn encode_writes_at_an_offset_and_zero_fills_the_tail() {
        let mut out = [0xFFu8; 64];
        let n = Header::current().encode(&mut out, 32).unwrap();
        assert_eq!(n, 32);
        assert_eq!(&out[32..64], &GOLDEN);
        // Everything before the header is untouched.
        assert_eq!(&out[..32], &[0xFF; 32]);
    }

    #[test]
    fn encode_rejects_a_short_buffer_without_panicking() {
        let mut small = [0u8; 31];
        let e = Header::current().encode(&mut small, 0).unwrap_err();
        assert_eq!(e.code(), "HEADER_BAD_MAGIC");

        // Offset that would run past the end.
        let mut buf = [0u8; 32];
        assert!(Header::current().encode(&mut buf, 1).is_err());
    }

    #[test]
    fn encode_rejects_an_overflowing_offset() {
        let mut buf = [0u8; 32];
        assert!(Header::current().encode(&mut buf, usize::MAX).is_err());
    }

    // ---- cross-check against the committed fixture ----

    #[test]
    fn matches_the_committed_header_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/valid/container/minimal-archive/header.bin");
        let bytes = std::fs::read(&path).expect("header.bin fixture present");
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(bytes, GOLDEN.to_vec(), "fixture drifted from the spec");
        assert_eq!(Header::decode(&bytes).unwrap(), Header::current());
    }

    #[test]
    fn round_trips_the_real_archive_header() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/valid/container/minimal-archive/archive.elang");
        let bytes = std::fs::read(&path).expect("archive fixture present");
        assert!(bytes.len() > HEADER_LEN);
        let h = Header::decode(&bytes).unwrap();
        assert_eq!((h.major(), h.minor(), h.flags()), (0, 1, 0));
    }

    #[test]
    fn default_header_equals_current() {
        // `Default::default()` is part of the API surface and must produce
        // the same header as `Header::current()` so callers that pass through
        // `..Default::default()` do not see a different binary than callers
        // that pass through `Header::current()`.
        let d: Header = Header::default();
        let c = Header::current();
        assert_eq!(d.major(), c.major());
        assert_eq!(d.minor(), c.minor());
        assert_eq!(d.flags(), c.flags());
    }

    #[test]
    fn default_header_encodes_to_the_golden_header_bin() {
        // Encoding must produce exactly the committed golden header bytes
        // regardless of whether `Default::default()` or `Header::current()`
        // produced the value.
        let h = Header::default();
        let mut out = vec![0u8; HEADER_LEN];
        let written = h.encode(&mut out, 0).unwrap();
        assert_eq!(written, HEADER_LEN);
        assert_eq!(&out[..], &reference_header()[..]);
    }
}
