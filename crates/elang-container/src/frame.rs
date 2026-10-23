//! Zstandard frame inspection, bounded streaming decode, and the locked
//! canonical encoder (Phase 4, T0401–T0421).
//!
//! # Why the header is parsed by hand
//!
//! libzstd exposes `ZSTD_getFrameHeader`, but it is not reachable through
//! `zstd-safe` without dropping to raw FFI, and it does not surface the two
//! RFC 8878 §3.1.1.1.1 descriptor bits that ELANG treats as fatal:
//! `Unused_bit` (bit 4) and `Reserved_bit` (bit 3). Both MUST be zero, and a
//! frame setting either is not conforming no matter what it decompresses to.
//! Parsing the six header bytes here costs ~40 lines, keeps the FFI surface to
//! exactly two libzstd calls, and makes the mapping from "which bit was
//! wrong" to "which error code" explicit and auditable.
//!
//! libzstd remains the authority for the frame *boundary*
//! ([`zstd_safe::find_frame_compressed_size`]) and for decoding blocks —
//! reimplementing block walking would be a correctness liability for no gain.
//!
//! # Why `zstd-safe` and not `zstd`
//!
//! The canonical profile (REQ-FRAME-010) pins parameters the high-level
//! `zstd` crate does not expose: `contentSizeFlag`, `dictIDFlag`,
//! `checksumFlag`, `windowLog`, LDM and worker count. `zstd-safe` gives
//! direct `CCtx::set_parameter` access while owning the FFI lifetimes.
//!
//! # Validation order
//!
//! [`inspect`] is the single gate every frame passes through. The order below
//! is normative because it decides which error a frame with several defects
//! reports:
//!
//! 1. Magic — is this a Zstd frame at all? A skippable magic is reported
//!    separately from garbage, because a skippable frame is a *well-formed*
//!    frame the format forbids here, not corruption.
//! 2. Descriptor sanity — unused/reserved bits (T0402).
//! 3. Declared content size required (T0403).
//! 4. Dictionary dependency (T0405), before the window check, because a
//!    dictionary-dependent frame is rejected on its face.
//! 5. Window ceiling, >128 MiB (T0404).
//! 6. Frame checksum required (T0403).
//! 7. Exactly one frame, complete, nothing after it (T0406, T0407).

use elang_core::limits::{MAX_GENERIC_RESOURCE_BYTES, MAX_UNCOMPRESSED_AGGREGATE_BYTES};
use elang_core::{Category, Error, Result};
use zstd_safe::{
    CCtx, CParameter, DCtx, InBuffer, OutBuffer, MAGICNUMBER, MAGIC_SKIPPABLE_MASK,
    MAGIC_SKIPPABLE_START,
};

/// Maximum permitted frame window: 128 MiB, i.e. `windowLog` 27 (T0404).
///
/// A larger window is not merely wasteful, it is a denial-of-service vector:
/// libzstd allocates a history buffer proportional to the window before
/// producing any output byte.
pub const MAX_WINDOW_LOG: u32 = 27;

/// Window ceiling in bytes: exactly `1 << MAX_WINDOW_LOG`.
pub const MAX_WINDOW_BYTES: u64 = 1 << MAX_WINDOW_LOG;

/// The locked libzstd version, encoded `MAJOR*10_000 + MINOR*100 + PATCH`.
///
/// T0401/REQ-FRAME-001 pin libzstd 1.5.7. The `zstd-sys` build metadata says
/// `+zstd.1.5.7`, but that records what was *requested*; only
/// [`linked_libzstd_version`] reads what actually linked, which is the check
/// that survives a widened pin or a system library leaking in.
pub const PINNED_LIBZSTD_VERSION: u32 = 10_507;

/// The `elang-canonical-zstd-v1` profile identifier (REQ-CORE-VERSION-001).
pub const CANONICAL_PROFILE: &str = "elang-canonical-zstd-v1";

/// Return the runtime libzstd version as `MAJOR*10_000 + MINOR*100 + PATCH`.
///
/// Reads the *linked* library rather than the crate pin, so it detects a
/// build-strategy substitution that would silently change canonical bytes.
#[must_use]
pub fn linked_libzstd_version() -> u32 {
    zstd_safe::version_number()
}

/// A validated standard Zstd frame header.
///
/// Every field is populated by [`inspect`], which has already rejected every
/// frame that could make a field untrustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Declared decompressed size. Always known: ELANG rejects frames
    /// without a content-size field, so this is never a sentinel.
    pub content_size: u64,
    /// Window size in bytes. At most [`MAX_WINDOW_BYTES`].
    pub window_size: u64,
    /// Total frame size on disk, excluding any bytes beyond the frame.
    pub compressed_size: u64,
    /// Bytes in the frame header itself, from the magic through the last
    /// header field.
    pub header_size: usize,
}

impl FrameHeader {
    /// Check the declared content size against the index entry (T0408).
    ///
    /// # Errors
    ///
    /// `FRAME_CONTENT_SIZE_MISMATCH` when the frame's declared size differs
    /// from the size the resource index claims for this slot.
    pub fn verify_against_index(self, index_uncompressed_size: u64) -> Result<Self> {
        if self.content_size != index_uncompressed_size {
            return Err(frame_err(
                "FRAME_CONTENT_SIZE_MISMATCH",
                format!(
                    "frame declares {} bytes, index declares {index_uncompressed_size}",
                    self.content_size
                ),
            ));
        }
        Ok(self)
    }
}

fn frame_err(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, Category::Frame, msg)
}

/// True when the first four bytes carry a Zstd skippable-frame magic.
///
/// Skippable magics are `0x184D2A50..=0x184D2A5F`, so the low nibble masks off.
fn is_skippable_magic(magic: u32) -> bool {
    magic & MAGIC_SKIPPABLE_MASK == MAGIC_SKIPPABLE_START
}

/// Little-endian integer from up to 8 bytes, or `None` if out of range.
fn le(bytes: &[u8]) -> Option<u64> {
    let mut buf = [0u8; 8];
    let take = bytes.len().min(8);
    for (d, s) in buf.iter_mut().zip(bytes.iter().take(take)) {
        *d = *s;
    }
    Some(u64::from_le_bytes(buf))
}

/// Little-endian 4-byte value. The cast is infallible on a 4-byte input, but
/// clippy can't see that without an explicit `try_from`; wrapping it here
/// keeps the call sites narrow and the safety documented in one place.
fn le_u32(bytes: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

/// Inspect one standard Zstd frame and enforce the ELANG profile rules.
///
/// Performs *no* decompression, so it is safe to call on arbitrary bytes.
///
/// # Errors
///
/// | Condition | Code | Task |
/// |---|---|---|
/// | Fewer than 4 bytes, wrong magic, bad descriptor bits | `FRAME_BAD_MAGIC` | T0402 |
/// | Skippable-frame magic | `FRAME_SKIPPABLE` | T0406 |
/// | Header or body incomplete | `FRAME_TRUNCATED` | T0407 |
/// | Non-zero `dictID` | `FRAME_DICTIONARY_DEPENDENCY` | T0405 |
/// | Window > 128 MiB, or `windowLog` out of range | `FRAME_WINDOW_TOO_LARGE` | T0404 |
/// | No declared content size | `FRAME_MISSING_CONTENT_SIZE` | T0403 |
/// | No frame checksum | `FRAME_MISSING_CHECKSUM` | T0403 |
/// | Bytes remain after the frame | `FRAME_CONCATENATED` | T0406 |
///
/// Bytes after a complete frame report `FRAME_CONCATENATED` rather than a
/// separate trailing-bytes code: both mean this slot holds more than one
/// frame's worth of data, and one code keeps the contract total.
pub fn inspect(bytes: &[u8]) -> Result<FrameHeader> {
    let magic = le_u32(bytes.get(..4).ok_or_else(|| {
        frame_err(
            "FRAME_BAD_MAGIC",
            format!(
                "frame is {} bytes, too short for a magic number",
                bytes.len()
            ),
        )
    })?)
    .unwrap_or_default(); // 4-byte LE always fits u32

    if is_skippable_magic(magic) {
        return Err(frame_err(
            "FRAME_SKIPPABLE",
            "skippable frames are not permitted in an ELANG package",
        ));
    }
    if magic != MAGICNUMBER {
        return Err(frame_err(
            "FRAME_BAD_MAGIC",
            format!("expected magic {MAGICNUMBER:#010x}, found {magic:#010x}"),
        ));
    }

    // RFC 8878 §3.1.1.1.1 Frame_Header_Descriptor.
    let d = *bytes
        .get(4)
        .ok_or_else(|| frame_err("FRAME_TRUNCATED", "frame ends before the header descriptor"))?;
    let fcs_flag = (d >> 6) & 0x03;
    let single_segment = (d >> 5) & 0x01 == 1;
    let unused_bit = (d >> 4) & 0x01 == 1;
    let reserved_bit = (d >> 3) & 0x01 == 1;
    let has_checksum = (d >> 2) & 0x01 == 1;
    let dict_id_flag = d & 0x03;

    if unused_bit {
        return Err(frame_err(
            "FRAME_BAD_MAGIC",
            "frame header descriptor sets the unused bit, which RFC 8878 requires to be zero",
        ));
    }
    if reserved_bit {
        return Err(frame_err(
            "FRAME_BAD_MAGIC",
            "frame header descriptor sets the reserved bit, which RFC 8878 requires to be zero",
        ));
    }

    // T0403. Size is checked before window arithmetic because without a
    // declared size there is nothing to bound.
    let fcs_len = match (fcs_flag, single_segment) {
        (0, false) => 0,
        (0, true) => 1,
        (_, _) => 1usize.checked_shl(u32::from(fcs_flag)).unwrap_or(0),
    };
    if fcs_len == 0 {
        return Err(frame_err(
            "FRAME_MISSING_CONTENT_SIZE",
            "frame does not declare its decompressed size",
        ));
    }

    let mut at = 5usize;

    // Window_Descriptor is present only when Single_Segment_flag is clear.
    let mut window_size = 0u64;
    let mut window_log = 0u32;
    if !single_segment {
        let w = *bytes.get(at).ok_or_else(|| {
            frame_err("FRAME_TRUNCATED", "frame ends before the window descriptor")
        })?;
        at = at.saturating_add(1);
        window_log = 10u32.saturating_add(u32::from(w >> 3));
        let mantissa = u64::from(w & 0x07);
        let base = 1u64.checked_shl(window_log).unwrap_or(u64::MAX);
        window_size =
            base.saturating_add(base.checked_div(8).unwrap_or(0).saturating_mul(mantissa));
    }

    // Dictionary_ID: 0, 1, 2 or 4 bytes, little-endian. The size table is
    // RFC 8878 §3.1.1.1.3: `flag == 0` means *no* field, not a 1-byte field.
    // `dict_id_flag` is masked to 2 bits above, so the index is in range.
    const DICT_FIELD_SIZE: [usize; 4] = [0, 1, 2, 4];
    let dict_len = DICT_FIELD_SIZE
        .get(usize::from(dict_id_flag))
        .copied()
        .unwrap_or(0);
    if dict_len > 0 {
        let raw = bytes.get(at..at.saturating_add(dict_len)).ok_or_else(|| {
            frame_err(
                "FRAME_TRUNCATED",
                "frame ends inside its dictionary ID field",
            )
        })?;
        let id = le(raw).unwrap_or(0);
        if id != 0 {
            return Err(frame_err(
                "FRAME_DICTIONARY_DEPENDENCY",
                format!(
                    "frame requires dictionary {id:#010x}; ELANG resources are dictionary-free"
                ),
            ));
        }
        at = at.saturating_add(dict_len);
    }

    // Dictionary dependency takes precedence over every window error.
    if !single_segment && !(10..=31).contains(&window_log) {
        return Err(frame_err(
            "FRAME_WINDOW_TOO_LARGE",
            format!("frame window log {window_log} is outside the legal range 10..=31"),
        ));
    }

    // Frame_Content_Size.
    let raw_fcs = bytes.get(at..at.saturating_add(fcs_len)).ok_or_else(|| {
        frame_err(
            "FRAME_TRUNCATED",
            "frame ends inside its content size field",
        )
    })?;
    let raw_size = le(raw_fcs).unwrap_or(0);
    // A 2-byte content size is stored biased by 256 (RFC 8878 §3.1.1.1.2).
    let content_size = if fcs_len == 2 {
        raw_size.saturating_add(256)
    } else {
        raw_size
    };
    at = at.saturating_add(fcs_len);

    // Single_Segment_flag means the whole input forms one window.
    if single_segment {
        window_size = content_size;
    }
    if window_size > MAX_WINDOW_BYTES {
        return Err(frame_err(
            "FRAME_WINDOW_TOO_LARGE",
            format!("frame window {window_size} exceeds the {MAX_WINDOW_BYTES}-byte ceiling"),
        ));
    }

    if !has_checksum {
        return Err(frame_err(
            "FRAME_MISSING_CHECKSUM",
            "frame does not carry a content checksum",
        ));
    }

    // libzstd walks the block chain to find the boundary; it is the authority
    // here.
    let frame_len = zstd_safe::find_frame_compressed_size(bytes).map_err(|e| {
        frame_err(
            "FRAME_TRUNCATED",
            format!("frame body is incomplete or corrupt: {e}"),
        )
    })?;
    let frame_len = u64::try_from(frame_len).unwrap_or(u64::MAX);
    let total = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if frame_len > total {
        return Err(frame_err(
            "FRAME_TRUNCATED",
            format!("frame declares {frame_len} bytes but only {total} are present"),
        ));
    }
    if frame_len < total {
        return Err(frame_err(
            "FRAME_CONCATENATED",
            format!("{total} bytes occupy one slot but the first frame is only {frame_len}"),
        ));
    }

    Ok(FrameHeader {
        content_size,
        window_size,
        compressed_size: frame_len,
        header_size: at,
    })
}

/// Compression profiles for the writer (T0412–T0414).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Level 3. Fast, larger output, deterministic but not canonical.
    Fast,
    /// Level 9. Slower, smaller output, deterministic but not canonical.
    Compact,
    /// Level 9 with the locked `elang-canonical-zstd-v1` parameters.
    /// Byte-identical across runs and platforms (REQ-FRAME-011).
    Canonical,
}

impl Profile {
    /// The profile identifier written into package metadata.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Compact => "compact",
            Self::Canonical => CANONICAL_PROFILE,
        }
    }

    /// Compression level for this profile (T0412, T0413, T0414).
    const fn level(self) -> i32 {
        match self {
            Self::Fast => 3,
            Self::Compact | Self::Canonical => 9,
        }
    }
}

/// A decoding budget spanning one or more resources.
///
/// Every limit is enforced by [`decode`] *before* an allocation grows past
/// it, which is what separates a real defence from a post-hoc check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeBudget {
    /// Ceiling on a single resource's decompressed size (T0411).
    pub max_resource_bytes: u64,
    /// Ceiling on the running total across every resource decoded under this
    /// budget (T0409): 8 GiB per REQ-FRAME-008.
    pub max_aggregate_bytes: u64,
    /// Bytes already charged to the aggregate.
    pub aggregate_used: u64,
}

impl DecodeBudget {
    /// A budget with the given per-resource ceiling and the standard 8 GiB
    /// aggregate.
    #[must_use]
    pub fn new(max_resource_bytes: u64) -> Self {
        Self {
            max_resource_bytes,
            max_aggregate_bytes: MAX_UNCOMPRESSED_AGGREGATE_BYTES,
            aggregate_used: 0,
        }
    }

    /// A budget bounded only by the generic per-resource ceiling (T0411).
    ///
    /// Phase 4 has no EPF, ELD or integrity manifest to read yet, so no
    /// resource class has a tighter ceiling to apply. Callers that later do
    /// use [`Self::new`] with the class-specific limit.
    #[must_use]
    pub const fn generic() -> Self {
        Self {
            max_resource_bytes: MAX_GENERIC_RESOURCE_BYTES,
            max_aggregate_bytes: MAX_UNCOMPRESSED_AGGREGATE_BYTES,
            aggregate_used: 0,
        }
    }

    /// Charge `bytes` to a copy of this budget.
    ///
    /// The caller commits the copy's total only after the bytes exist, so a
    /// decode that fails mid-stream cannot leak budget into the next resource.
    fn charged(&self, bytes: u64) -> Result<Self> {
        if bytes > self.max_resource_bytes {
            return Err(frame_err(
                "DECOMPRESSION_BUDGET_EXCEEDED",
                format!(
                    "resource declares {bytes} bytes, per-resource ceiling is {}",
                    self.max_resource_bytes
                ),
            ));
        }
        let total = self.aggregate_used.checked_add(bytes).ok_or_else(|| {
            frame_err(
                "DECOMPRESSION_BUDGET_EXCEEDED",
                "aggregate size overflowed u64",
            )
        })?;
        if total > self.max_aggregate_bytes {
            return Err(frame_err(
                "DECOMPRESSION_BUDGET_EXCEEDED",
                format!(
                    "aggregate would reach {total} bytes, ceiling is {}",
                    self.max_aggregate_bytes
                ),
            ));
        }
        Ok(Self {
            aggregate_used: total,
            ..*self
        })
    }
}

/// Scratch size for streaming decode. Small enough to stay in cache, large
/// enough that per-call overhead is irrelevant next to libzstd's work.
const DECODE_SCRATCH: usize = 128 * 1024;

/// Decode exactly one frame under a budget.
///
/// The header is inspected before any decode begins, so a malformed or
/// oversized frame costs nothing. libzstd is then driven in streaming mode
/// against a bounded scratch buffer, which is what actually stops a
/// decompression bomb: a frame declaring 1 KiB that expands to 100 GiB is
/// killed at the cap rather than after allocating its expansion.
///
/// # Errors
///
/// Every [`inspect`] error, plus `DECOMPRESSION_BUDGET_EXCEEDED` when the resource
/// or the running total exceeds its ceiling, `FRAME_DECOMPRESS_FAILED` when
/// libzstd rejects the body, `FRAME_CHECKSUM_MISMATCH` when its checksum fails,
/// and `FRAME_DECOMPRESSED_SIZE_MISMATCH` when a
/// frame contradicts its own declared size.
pub fn decode(bytes: &[u8], budget: &mut DecodeBudget) -> Result<Vec<u8>> {
    let header = inspect(bytes)?;
    let charged = budget.charged(header.content_size)?;

    let mut dctx = DCtx::create();
    let mut out: Vec<u8> = Vec::with_capacity(
        usize::try_from(header.content_size.min(DECODE_SCRATCH as u64)).unwrap_or(DECODE_SCRATCH),
    );
    let mut scratch = vec![0u8; DECODE_SCRATCH];
    let mut input = InBuffer::around(bytes);

    loop {
        let produced = {
            let mut dst = OutBuffer::around(&mut scratch[..]);
            let hint = dctx.decompress_stream(&mut dst, &mut input).map_err(|e| {
                // SAFETY: ZSTD_getErrorCode only classifies an integer result;
                // it takes no pointers and has no memory or lifetime requirements.
                #[allow(
                    unsafe_code,
                    reason = "zstd-safe 8.2.0 has no safe error-code accessor"
                )]
                let code = match unsafe { zstd_sys::ZSTD_getErrorCode(e) } {
                    zstd_sys::ZSTD_ErrorCode::ZSTD_error_checksum_wrong => {
                        "FRAME_CHECKSUM_MISMATCH"
                    }
                    _ => "FRAME_DECOMPRESS_FAILED",
                };
                frame_err(code, format!("libzstd rejected the frame body: {e}"))
            })?;
            let pos = dst.pos();
            (hint, pos)
        };
        let (hint, written) = produced;
        out.extend_from_slice(scratch.get(..written).unwrap_or(&[]));

        let total = u64::try_from(out.len()).unwrap_or(u64::MAX);
        if total > header.content_size {
            return Err(frame_err(
                "FRAME_DECOMPRESSED_SIZE_MISMATCH",
                format!(
                    "frame produced at least {total} bytes but declared {}",
                    header.content_size
                ),
            ));
        }
        if total > charged.max_resource_bytes {
            return Err(frame_err(
                "DECOMPRESSION_BUDGET_EXCEEDED",
                format!(
                    "decompressed output reached {total} bytes, ceiling is {}",
                    charged.max_resource_bytes
                ),
            ));
        }

        // `hint == 0` is libzstd's frame-complete signal.
        if hint == 0 {
            break;
        }
        // No input left and nothing produced: the body ended early.
        if input.pos() >= bytes.len() && written == 0 {
            return Err(frame_err(
                "FRAME_TRUNCATED",
                "frame body ended before the declared content size",
            ));
        }
    }

    let total = u64::try_from(out.len()).unwrap_or(u64::MAX);
    if total != header.content_size {
        return Err(frame_err(
            "FRAME_DECOMPRESSED_SIZE_MISMATCH",
            format!(
                "decoded {total} bytes, frame declared {}",
                header.content_size
            ),
        ));
    }

    // Commit the aggregate charge only once the bytes actually exist.
    budget.aggregate_used = charged.aggregate_used;
    Ok(out)
}

/// Compress `input` into exactly one frame under `profile`.
///
/// # Canonical determinism
///
/// The canonical path builds a **fresh** [`CCtx`] per call (T0414). Reusing a
/// context would let libzstd carry state — a loaded dictionary, an internal
/// workspace sized to a previous input, the pledged source size from an
/// earlier frame — into the next frame and change its bytes. A fresh context,
/// an explicit parameter set (T0415–T0416), and a single `ZSTD_compress2`
/// over the whole input (T0417) are what make T0419's byte-for-byte
/// comparison hold.
///
/// Every parameter that could vary between runs is set explicitly. Those left
/// at the pinned library default are asserted by the default-freeze test, so a
/// libzstd bump cannot quietly move canonical bytes (T0418).
///
/// # Errors
///
/// `FRAME_ENCODE_FAILED` if libzstd cannot produce a frame, or if it rejects a
/// parameter — which would mean the pin drifted and canonical output is no
/// longer reproducible.
pub fn encode(input: &[u8], profile: Profile) -> Result<Vec<u8>> {
    let mut cctx = CCtx::create();

    // T0415. These apply to every profile, not just canonical: this crate's
    // reader rejects frames omitting any of them, so an encoder emitting an
    // unreadable frame would be a bug rather than a profile choice.
    apply(&mut cctx, CParameter::CompressionLevel(profile.level()))?;
    apply(&mut cctx, CParameter::NbWorkers(0))?;
    apply(&mut cctx, CParameter::ChecksumFlag(true))?;
    apply(&mut cctx, CParameter::ContentSizeFlag(true))?;
    apply(&mut cctx, CParameter::DictIdFlag(false))?;

    if profile == Profile::Canonical {
        // T0416. The only parameters differing from the library defaults;
        // everything else stays pinned and is verified by T0418.
        apply(&mut cctx, CParameter::WindowLog(MAX_WINDOW_LOG))?;
        apply(&mut cctx, CParameter::EnableLongDistanceMatching(false))?;
    }

    // T0417. A pledged size lets libzstd write the content-size field without
    // a second pass; it does not influence match selection, so it cannot move
    // output bytes.
    cctx.set_pledged_src_size(Some(u64::try_from(input.len()).unwrap_or(u64::MAX)))
        .map_err(|e| frame_err("FRAME_ENCODE_FAILED", format!("set_pledged_src_size: {e}")))?;

    let bound = zstd_safe::compress_bound(input.len());
    let mut buffer = vec![0u8; bound];
    // SAFETY: `compress2` writes only into `buffer[..]` and reads only
    // `input`; both outlive the call.
    let written = cctx
        .compress2(&mut buffer[..], input)
        .map_err(|e| frame_err("FRAME_ENCODE_FAILED", format!("compress2: {e}")))?;

    buffer.truncate(written);

    // Self-check: a frame this crate cannot re-read is worse than an error at
    // write time, so every emitted frame goes through our own reader.
    inspect(&buffer)?;
    Ok(buffer)
}

fn apply(cctx: &mut CCtx, param: CParameter) -> Result<()> {
    cctx.set_parameter(param).map(|_| ()).map_err(|e| {
        frame_err(
            "FRAME_ENCODE_FAILED",
            format!("libzstd rejected {param:?}: {e}"),
        )
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "frame tests build hand-crafted frames with fixed-length slices and assert Err variants; the indices are compile-time constants and the panics are test-failure signals"
)]
mod tests;
