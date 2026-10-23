//! Tests for the Zstandard frame reader and canonical encoder.
//!
//! # How to read this file
//!
//! The tests are grouped by surface area: [`pinning`] verifies the lock, then
//! [`inspect`] exhaustively walks every rejection branch in [`crate::frame::inspect`],
//! [`decode`] tests the bounded streaming decoder, and [`encode`] tests the
//! three profiles plus determinism and round-trips. The whole module is run
//! against every branch by `cargo test -p elang-container frame::`.
//!
//! [`pinning`]: self::pinning
//! [`inspect`]: self::inspect
//! [`decode`]: self::decode
//! [`encode`]: self::encode

use super::*;
use elang_core::Category;

// =============================================================================
// Pinned library version
// =============================================================================

#[test]
fn pinned_zstd_constant_is_157() {
    assert_eq!(PINNED_LIBZSTD_VERSION, 10507);
}

#[test]
fn linked_libzstd_is_157() {
    // REQ-FRAME-001 / T0401. If this fails, the build strategy drifted:
    // canonical frame bytes may have moved.
    assert_eq!(
        linked_libzstd_version(),
        PINNED_LIBZSTD_VERSION,
        "linked libzstd is not the pinned 1.5.7",
    );
}

// =============================================================================
// `inspect` — every rejection branch
// =============================================================================

#[test]
fn inspect_rejects_an_empty_buffer_as_bad_magic() {
    let err = inspect(&[]).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
    assert_eq!(err.category(), Category::Frame);
}

#[test]
fn inspect_rejects_three_bytes_as_bad_magic() {
    let err = inspect(&[0x28, 0xB5, 0x2F]).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
}

#[test]
fn inspect_rejects_a_wrong_magic_as_bad_magic() {
    let mut bytes = vec![0u8; 32];
    bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let err = inspect(&bytes).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
}

#[test]
fn inspect_rejects_a_skippable_frame_magic() {
    // Skippable frame magic variant 0: 0x184D2A50.
    let mut bytes = vec![0u8; 16];
    bytes[0..4].copy_from_slice(&[0x50, 0x2A, 0x4D, 0x18]);
    let err = inspect(&bytes).unwrap_err();
    assert_eq!(err.code(), "FRAME_SKIPPABLE");
}

#[test]
fn inspect_rejects_the_highest_skippable_magic_variant() {
    // 0x184D2A5F.
    let mut bytes = vec![0u8; 16];
    bytes[0..4].copy_from_slice(&[0x5F, 0x2A, 0x4D, 0x18]);
    let err = inspect(&bytes).unwrap_err();
    assert_eq!(err.code(), "FRAME_SKIPPABLE");
}

#[test]
fn inspect_rejects_an_unused_bit_set_in_descriptor() {
    let frame = valid_canonical(b"abc");
    let mut mutated = frame;
    mutated[4] |= 0b0001_0000; // Unused_bit
    let err = inspect(&mutated).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
}

#[test]
fn inspect_rejects_a_reserved_bit_set_in_descriptor() {
    let frame = valid_canonical(b"abc");
    let mut mutated = frame;
    mutated[4] |= 0b0000_1000; // Reserved_bit
    let err = inspect(&mutated).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
}

#[test]
fn inspect_rejects_a_frame_with_no_content_size_flag() {
    // Build a frame by hand with FCS_field_size = 0: a single non-skippable
    // frame magic followed by a single descriptor byte.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]); // magic
    bytes.push(0x00); // descriptor: FCS=0, single=0, no dict, no checksum
    bytes.push(0xAF); // window desc -> windowLog=10+(0xAF>>3)=0x15=21 (valid)
    bytes.extend_from_slice(&[0u8; 4]); // body placeholder
    let err = inspect(&bytes).unwrap_err();
    assert_eq!(err.code(), "FRAME_MISSING_CONTENT_SIZE");
}

#[test]
fn inspect_rejects_a_frame_with_no_checksum_flag() {
    // Encode with checksumFlag=false by hand. Easiest: build a real frame
    // via our encoder, then disable the checksum bit.
    let mut frame = valid_canonical(b"hello");
    frame[4] &= !0b0000_0100; // clear Content_Checksum_flag
    let err = inspect(&frame).unwrap_err();
    assert_eq!(err.code(), "FRAME_MISSING_CHECKSUM");
}

#[test]
fn inspect_rejects_a_truncated_frame_header() {
    let mut frame = valid_canonical(b"hello");
    frame.truncate(3); // mid-magic
    let err = inspect(&frame).unwrap_err();
    assert_eq!(err.code(), "FRAME_BAD_MAGIC");
}

#[test]
fn inspect_rejects_a_truncated_frame_body() {
    let frame = valid_canonical(b"hello, this is a longer input");
    let truncated = &frame[..frame.len() / 2];
    let err = inspect(truncated).unwrap_err();
    assert_eq!(err.code(), "FRAME_TRUNCATED");
}

#[test]
fn inspect_rejects_a_concatenated_frame() {
    let frame = valid_canonical(b"first");
    let extra = valid_canonical(b"second");
    let mut both = frame.clone();
    both.extend_from_slice(&extra);
    let err = inspect(&both).unwrap_err();
    assert_eq!(err.code(), "FRAME_CONCATENATED");
}

#[test]
fn inspect_rejects_trailing_bytes_without_a_frame_magic() {
    let frame = valid_canonical(b"hi");
    let mut both = frame.clone();
    both.extend_from_slice(&[0u8; 8]);
    let err = inspect(&both).unwrap_err();
    assert_eq!(err.code(), "FRAME_CONCATENATED");
}

#[test]
fn inspect_rejects_an_oversized_window() {
    // Hand-craft a frame whose window descriptor yields >128 MiB:
    // windowLog=27 (descriptor bits 5..7), mantissa=7 (low bits). The base is
    // exactly 128 MiB; the add is (128 MiB / 8) * 7 = 112 MiB; total 240 MiB.
    //
    // The descriptor must put us past the FCS-presence gate first:
    // fcs_flag = 1 (2-byte FCS), single_segment = 0 (window present), checksum
    // = 1, dict_id_flag = 0. Byte 0b0100_0100 = 0x44.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]); // magic
    bytes.push(0x44); // descriptor: fcs=1, single=0, checksum=1, dict=0
    bytes.push(0x97); // window descriptor: log=27, mantissa=7 -> 240 MiB
    bytes.extend_from_slice(&0u16.to_le_bytes()); // FCS = 0 (FCS, +256 if relevant)
    bytes.extend_from_slice(&[0u8; 8]); // body placeholder
    let err = inspect(&bytes).unwrap_err();
    assert_eq!(err.code(), "FRAME_WINDOW_TOO_LARGE");
}

#[test]
fn inspect_accepts_a_valid_canonical_frame() {
    let frame = valid_canonical(b"this is a typical ELANG payload of modest size");
    let header = inspect(&frame).expect("canonical frame should be accepted");
    assert_eq!(header.content_size, 46);
    assert!(header.window_size <= MAX_WINDOW_BYTES);
    assert!(usize::try_from(header.compressed_size).unwrap_or(0) == frame.len());
    // has_checksum is implicit in inspect()'s accept path.
}

// =============================================================================
// `decode` — bounded streaming
// =============================================================================

#[test]
fn decode_round_trips_a_canonical_frame() {
    let input = b"hello ELANG hello ELANG hello ELANG hello ELANG hello ELANG";
    let frame = encode(input, Profile::Canonical).unwrap();
    let mut budget = DecodeBudget::generic();
    let out = decode(&frame, &mut budget).unwrap();
    assert_eq!(out, input);
}

#[test]
fn decode_charges_the_aggregate() {
    let input = vec![0u8; 4096];
    let frame = encode(&input, Profile::Canonical).unwrap();
    let mut budget = DecodeBudget::generic();
    let _ = decode(&frame, &mut budget).unwrap();
    assert_eq!(budget.aggregate_used, 4096);
}

#[test]
fn decode_releases_the_aggregate_on_failure() {
    let input = vec![0u8; 4096];
    let mut frame = encode(&input, Profile::Canonical).unwrap();
    let last = frame.len() - 1;
    frame[last] ^= 0xFF; // corrupt the checksum
    let mut budget = DecodeBudget::generic();
    let _ = decode(&frame, &mut budget).unwrap_err();
    assert_eq!(
        budget.aggregate_used, 0,
        "a failed decode must not leak budget"
    );
}

#[test]
fn decode_rejects_a_frame_above_the_per_resource_ceiling() {
    // Build a 32-byte payload, then a budget with a 16-byte ceiling.
    let input = vec![0u8; 32];
    let frame = encode(&input, Profile::Canonical).unwrap();
    let mut budget = DecodeBudget::new(16);
    let err = decode(&frame, &mut budget).unwrap_err();
    assert_eq!(err.code(), "DECOMPRESSION_BUDGET_EXCEEDED");
}

#[test]
fn decode_rejects_a_frame_above_the_aggregate_ceiling() {
    let input = vec![0u8; 4096];
    let frame = encode(&input, Profile::Canonical).unwrap();
    let mut budget = DecodeBudget {
        max_resource_bytes: u64::MAX,
        max_aggregate_bytes: 2048, // under the resource size
        aggregate_used: 0,
    };
    let err = decode(&frame, &mut budget).unwrap_err();
    assert_eq!(err.code(), "DECOMPRESSION_BUDGET_EXCEEDED");
}

#[test]
fn decode_rejects_a_corrupted_body() {
    let input = vec![0xABu8; 4096];
    let mut frame = encode(&input, Profile::Canonical).unwrap();
    // Flip a bit well past the header so it lies in a block body, not metadata.
    let target = (frame.len() / 2).min(frame.len().saturating_sub(2));
    frame[target] ^= 0xFF;
    let mut budget = DecodeBudget::generic();
    let err = decode(&frame, &mut budget).unwrap_err();
    assert!(
        matches!(
            err.code(),
            "FRAME_DECOMPRESS_FAILED"
                | "FRAME_DECOMPRESSED_SIZE_MISMATCH"
                | "FRAME_CHECKSUM_MISMATCH"
                | "FRAME_TRUNCATED"
        ),
        "unexpected code {}",
        err.code()
    );
}

#[test]
fn decode_rejects_a_corrupted_checksum() {
    let input = vec![0u8; 1024];
    let mut frame = encode(&input, Profile::Canonical).unwrap();
    // The trailing 4 bytes are the frame checksum.
    let last = frame.len() - 1;
    frame[last] ^= 0xFF;
    let mut budget = DecodeBudget::generic();
    let err = decode(&frame, &mut budget).unwrap_err();
    assert!(
        matches!(
            err.code(),
            "FRAME_CHECKSUM_MISMATCH" | "FRAME_DECOMPRESS_FAILED"
        ),
        "expected checksum mismatch, got {}",
        err.code()
    );
}

// =============================================================================
// `encode` — three profiles + determinism + round-trips
// =============================================================================

#[test]
fn profile_ids_are_stable() {
    assert_eq!(Profile::Fast.id(), "fast");
    assert_eq!(Profile::Compact.id(), "compact");
    assert_eq!(Profile::Canonical.id(), "elang-canonical-zstd-v1");
}

#[test]
fn encode_then_inspect_round_trips_each_profile() {
    let input = b"the quick brown fox jumps over the lazy dog, again and again";
    for profile in [Profile::Fast, Profile::Compact, Profile::Canonical] {
        let frame = encode(input, profile).unwrap();
        let header = inspect(&frame).unwrap();
        assert_eq!(header.content_size, input.len() as u64, "{profile:?}");
        let mut budget = DecodeBudget::generic();
        let out = decode(&frame, &mut budget).unwrap();
        assert_eq!(out, input, "{profile:?}");
    }
}

#[test]
fn canonical_is_byte_identical_across_repeated_runs() {
    let input = b"deterministic-by-construction canonical payload";
    let a = encode(input, Profile::Canonical).unwrap();
    let b = encode(input, Profile::Canonical).unwrap();
    let c = encode(input, Profile::Canonical).unwrap();
    assert_eq!(a, b, "canonical not byte-identical between run 1 and run 2");
    assert_eq!(b, c, "canonical not byte-identical between run 2 and run 3");
}

#[test]
fn canonical_uses_a_fresh_context_per_call() {
    // This is a structural assertion: encode from the *same* context reused
    // for two distinct inputs could leak state. Our encoder creates a new
    // CCtx per call, so identical inputs are stable. Smoke by encoding the
    // same input 10 times and confirming equality across all 10.
    let input = vec![0xCAu8; 1024];
    let first = encode(&input, Profile::Canonical).unwrap();
    for i in 1..10 {
        let again = encode(&input, Profile::Canonical).unwrap();
        assert_eq!(again, first, "canonical frame drifted on iteration {i}");
    }
}

#[test]
fn canonical_window_never_exceeds_the_ceiling() {
    // T0416 / REQ-FRAME-010. `WindowLog` is a *maximum*: libzstd may choose a
    // smaller window for small inputs. What we can pin down without
    // pinning down libzstd's heuristics is that the produced frame's window
    // never crosses the 128 MiB ceiling. Inspect already enforces this, so a
    // successful round-trip on a payload large enough to be interesting is
    // the assertion.
    let payload = vec![0u8; 1024 * 1024]; // 1 MiB of zeros
    let frame = encode(&payload, Profile::Canonical).unwrap();
    let header = inspect(&frame).unwrap();
    assert!(header.window_size <= MAX_WINDOW_BYTES);
}

#[test]
fn canonical_includes_a_declared_content_size() {
    let payload = b"with a content size"; // 19 bytes
    let frame = encode(payload, Profile::Canonical).unwrap();
    let header = inspect(&frame).unwrap();
    assert_eq!(
        usize::try_from(header.content_size).unwrap_or(0),
        payload.len()
    );
}

#[test]
fn canonical_includes_a_frame_checksum() {
    // The accept path requires has_checksum=true; inspect would have returned
    // MISSING_CHECKSUM otherwise. So a successful inspect is the assertion.
    let frame = encode(b"with a checksum", Profile::Canonical).unwrap();
    inspect(&frame).unwrap();
}

#[test]
fn encode_empty_input_produces_a_valid_frame() {
    let frame = encode(&[], Profile::Canonical).unwrap();
    let header = inspect(&frame).unwrap();
    assert_eq!(header.content_size, 0);
    let mut budget = DecodeBudget::generic();
    let out = decode(&frame, &mut budget).unwrap();
    assert!(out.is_empty());
}

#[test]
fn encode_then_verify_against_index_matches_content_size() {
    let input = b"x".repeat(1234);
    let frame = encode(&input, Profile::Canonical).unwrap();
    let header = inspect(&frame).unwrap();
    let header = header.verify_against_index(1234).unwrap();
    assert_eq!(header.content_size, 1234);
    let err = inspect(&frame)
        .unwrap()
        .verify_against_index(9999)
        .unwrap_err();
    assert_eq!(err.code(), "FRAME_CONTENT_SIZE_MISMATCH");
}

// =============================================================================
// Helpers
// =============================================================================

/// Produce a known-good frame the inspect tests can then mutate.
///
/// Each test asserts on a one-byte change, so the helper pins the bytes our
/// encoder produces. If a future profile change moves any byte, the failing
/// mutation test will tell us which assumption shifted.
fn valid_canonical(input: &[u8]) -> Vec<u8> {
    encode(input, Profile::Canonical).expect("canonical encode cannot fail on in-memory input")
}
