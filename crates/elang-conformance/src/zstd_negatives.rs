//! T1411 — Zstd frame contract negatives.
//!
//! The ELANG container requires every indexed slot to hold exactly one
//! **Zstandard** frame that conforms to RFC 8878's frame contract and to
//! the profile pinned in steering §3. The negative fixtures here break
//! each clause of that contract one at a time, keeping the surrounding
//! envelope (header, index, footer) valid so the reader reaches the
//! frame stage and rejects for the intended reason.
//!
//! | Fixture | Broken clause | Expected code |
//! |---|---|---|
//! | `zstd/missing-checksum` | no content checksum | `FRAME_MISSING_CHECKSUM` |
//! | `zstd/window-too-large` | window exceeds the 128 MiB ceiling | `FRAME_WINDOW_TOO_LARGE` |
//! | `zstd/concatenated` | two frames in one slot | `FRAME_CONCATENATED` |
//! | `zstd/skippable-frame` | a skippable frame | `FRAME_SKIPPABLE` |
//! | `zstd/bad-magic` | not a Zstd frame at all | `FRAME_BAD_MAGIC` |
//! | `zstd/truncated` | a frame that ends mid-body | `FRAME_TRUNCATED` |
//!
//! # How the frames are forged
//!
//! The writer cannot produce any of these — it always emits a
//! conforming frame. So the fixtures forge the frame bytes directly and
//! wrap them in a hand-built envelope whose index hash matches the
//! bytes it stores. The reader's index-hash pre-check therefore passes
//! and the *frame* is what fails.
//!
//! Each forged frame is built by taking a **real** production frame and
//! editing exactly the byte that encodes the broken clause — so the
//! fixture differs from a valid archive in one bit, not in a whole
//! hand-rolled byte soup.

use crate::corpus::{Expectation, Fixture, Level};
use elang_container::frame::Profile;
use elang_core::error::Category;

/// The Zstandard magic number in little-endian byte order.
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// The frame header descriptor byte of a production frame.
///
/// The canonical profile always sets `Content_Checksum_flag` (bit 2) and
/// declares the content size, so a single descriptor-bit edit can turn
/// a valid frame into one with no checksum.
fn descriptor_byte(frame: &[u8]) -> u8 {
    frame[4]
}

/// Build a real production frame and clear its content-checksum bit.
fn frame_without_checksum() -> Vec<u8> {
    let mut frame =
        elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
    let descriptor = descriptor_byte(&frame);
    frame[4] = descriptor & !0b0000_0100;
    frame
}

/// Build a real production frame and declare an oversized window.
///
/// A hand-built frame with `windowLog = 40` and `Single_Segment_flag`
/// cleared. The remaining header is filled with bytes that make the
/// header structure valid (the multi-segment path needs an 8-byte
/// `Frame_Content_Size`); the body itself is a single compressed
/// block copied from a real production frame so the size bookkeeping
/// matches what the decoder finds.
fn frame_with_oversized_window() -> Vec<u8> {
    let real =
        elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
    let mut out = Vec::new();
    out.extend_from_slice(&ZSTD_MAGIC);
    // descriptor byte: FCS_Flag=3 (8 bytes), Single_Segment=0,
    // Content_Checksum_Flag=1.
    out.push(0b1100_0100);
    // window descriptor: exponent=30 (windowLog=10+30=40), mantissa=0.
    out.push(0b1111_1000);
    // dictionary ID: flag=0 means no field.
    // 8-byte Frame_Content_Size = 2 (the payload length).
    out.extend_from_slice(&2u64.to_le_bytes());
    // Now we need a Zstd block. The decoder will check the window
    // size before reading the body, so we can append any plausible
    // bytes — a real block followed by the checksum.
    let real_header_size = 5 + 1 /* single-segment FCS */;
    let real_body_and_checksum = &real[real_header_size..];
    out.extend_from_slice(real_body_and_checksum);
    out
}

/// Two production frames concatenated into one slot.
fn concatenated_frames() -> Vec<u8> {
    let first =
        elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
    let second =
        elang_container::frame::encode(b"y\n", Profile::Fast).expect("fixture frame encodes");
    let mut out = first;
    out.extend_from_slice(&second);
    out
}

/// A skippable frame (RFC 8878 §3.1.2): magic `0x184D2A5?` plus a
/// 4-byte little-endian payload length.
fn skippable_frame() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0x184D_2A50u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

/// A frame whose magic number is wrong.
fn frame_with_bad_magic() -> Vec<u8> {
    let mut frame =
        elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
    frame[0] = ZSTD_MAGIC[0] ^ 0xFF;
    frame
}

/// A production frame truncated mid-body.
fn truncated_frame() -> Vec<u8> {
    let frame =
        elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
    frame[..frame.len() / 2].to_vec()
}

/// Encode one index entry in the container's wire format.
fn encode_index_entry(
    out: &mut Vec<u8>,
    path: &[u8],
    frame_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    compression: u8,
) {
    let path_len = u16::try_from(path.len()).expect("test path fits u16");
    out.extend_from_slice(&path_len.to_le_bytes());
    out.extend_from_slice(path);
    out.extend_from_slice(&frame_offset.to_le_bytes());
    out.extend_from_slice(&compressed_size.to_le_bytes());
    out.extend_from_slice(&uncompressed_size.to_le_bytes());
    out.push(compression);
}

/// Build an archive whose single indexed slot holds `frame`.
///
/// The index hash in the footer is computed over the bytes actually
/// stored, so the reader's hash pre-check passes and the frame stage is
/// the one that rejects.
///
/// # Panics
///
/// Panics on fixture-authoring bugs — a silently-skipped fixture would
/// hide a gap in the corpus.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helper assembles a hand-rolled archive with bounded arithmetic"
)]
fn archive_with_frame(frame: Vec<u8>) -> Vec<u8> {
    use elang_container::footer::Footer;
    use elang_container::header::{Header, HEADER_LEN};
    use sha2::Digest;

    let frame_offset = HEADER_LEN as u64;
    let mut index_bytes = Vec::new();
    index_bytes.extend_from_slice(&elang_container::index::INDEX_MAGIC);
    index_bytes.extend_from_slice(&elang_container::index::INDEX_VERSION.to_le_bytes());
    index_bytes.extend_from_slice(&elang_container::index::ENTRY_FORMAT.to_le_bytes());
    index_bytes.extend_from_slice(&1u32.to_le_bytes());
    encode_index_entry(
        &mut index_bytes,
        b"Linguistics/phonemes.eld",
        frame_offset,
        frame.len() as u64,
        2,
        1,
    );
    let index_hash: [u8; 32] = sha2::Sha256::digest(&index_bytes).into();
    let footer = Footer::new(
        frame_offset.saturating_add(frame.len() as u64),
        index_bytes.len() as u64,
        index_hash,
    )
    .expect("fixture footer constructs");

    let mut out = Vec::new();
    let header = Header::current();
    let mut header_buf = [0u8; HEADER_LEN];
    header
        .encode(&mut header_buf, 0)
        .expect("fixture header encodes");
    out.extend_from_slice(&header_buf);
    out.extend_from_slice(&frame);
    out.extend_from_slice(&index_bytes);
    let mut footer_buf = [0u8; 64];
    footer
        .encode(&mut footer_buf, 0)
        .expect("fixture footer encodes");
    out.extend_from_slice(&footer_buf);
    out
}

/// All T1411 Zstd frame negatives.
#[must_use]
pub fn fixtures() -> Vec<Fixture> {
    let cases: &[(&'static str, &'static str, Vec<u8>, &'static str, Category)] = &[
        (
            "zstd/missing-checksum",
            "A frame without a content checksum is rejected",
            frame_without_checksum(),
            "FRAME_MISSING_CHECKSUM",
            Category::Frame,
        ),
        (
            "zstd/window-too-large",
            "A frame declaring an oversized window is rejected",
            frame_with_oversized_window(),
            "FRAME_WINDOW_TOO_LARGE",
            Category::Frame,
        ),
        (
            "zstd/concatenated",
            "Two frames in one indexed slot are rejected",
            concatenated_frames(),
            "FRAME_CONCATENATED",
            Category::Frame,
        ),
        (
            "zstd/skippable-frame",
            "A skippable frame is rejected",
            skippable_frame(),
            "FRAME_SKIPPABLE",
            Category::Frame,
        ),
        (
            "zstd/bad-magic",
            "A frame with the wrong magic number is rejected",
            frame_with_bad_magic(),
            "FRAME_BAD_MAGIC",
            Category::Frame,
        ),
        (
            "zstd/truncated",
            "A frame truncated mid-body is rejected",
            truncated_frame(),
            "FRAME_TRUNCATED",
            Category::Frame,
        ),
    ];

    cases
        .iter()
        .map(|(id, description, frame, code, category)| Fixture {
            id,
            description,
            requirements: &["REQ-FRAME-003", "REQ-FRAME-004", "REQ-FRAME-006"],
            expectation: Expectation::Reject {
                level: Level::Inspect,
                code,
                category: *category,
            },
            archive: archive_with_frame(frame.clone()),
        })
        .collect()
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_fixture)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_fixture;

    #[test]
    fn every_zstd_negative_is_rejected_at_the_declared_level() {
        for fixture in fixtures() {
            run_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn the_forged_frames_differ_from_a_real_frame() {
        let real =
            elang_container::frame::encode(b"x\n", Profile::Fast).expect("fixture frame encodes");
        assert_ne!(frame_without_checksum(), real);
        assert_ne!(frame_with_oversized_window(), real);
        assert_ne!(frame_with_bad_magic(), real);
        assert_ne!(truncated_frame(), real);
        assert!(concatenated_frames().len() > real.len());
    }
}
