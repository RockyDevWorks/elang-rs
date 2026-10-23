//! SHA-256 streaming hasher + per-resource digest verification (T0706, T0707).
//!
//! This module is the **reader-side** half of constraint C-06 from
//! [`specs/grammar/integrity-0.1.0.md`](../../../specs/grammar/integrity-0.1.0.md):
//!
//! > Each `sha256` compared to SHA-256 over the resource's **exact
//! > uncompressed bytes** (reader stage 11). The footer index hash (§3.3) is
//! > verified separately and is not an `integrity` entry.
//!
//! ## API shape
//!
//! - [`Hasher`] — a streaming SHA-256 accumulator the container drives with
//!   [`Hasher::update`] on each chunk of uncompressed bytes as the
//!   decompressor emits it.
//! - [`verify_one`] — constant-time compare of a computed digest against a
//!   manifest entry's declared digest; emits `INTEGRITY_HASH_MISMATCH` at
//!   stage 11 with the offending resource's path attached on mismatch.
//!
//! ## Constant-time discipline (T0707)
//!
//! Digests are public, so the *content* of a digest is not a secret. The only
//! timing channel that matters is the **equality** check itself —
//! short-circuiting on the first differing byte would let an attacker probe
//! one byte at a time by feeding many candidate resources and measuring
//! response time. [`ct_eq_32`] performs a branch-free XOR-fold over all 32
//! bytes so equality takes the same time regardless of where (or whether)
//! the inputs differ.
//!
//! ## What this module does not do
//!
//! - It does **not** decompress resources. The container layer (stage 11)
//!   owns decompression; this crate receives the uncompressed byte stream.
//! - It does **not** verify the footer index hash. ADR 0004 §3 ("Coverage")
//!   keeps the footer hash separate: it lives in the container footer, not
//!   in this manifest. The structural separation is already enforced —
//!   [`crate::parse::parse_document`] rejects any unknown property on a
//!   `resource` node, so a hypothetical `footer_hash` property would fail
//!   C-05 with `INTEGRITY_STRUCTURE_INVALID`.

use std::fmt::Write;

use elang_core::error::{Category, Diagnostic, Error};
use elang_core::ids::ResourcePath;
use sha2::{Digest, Sha256};

use crate::manifest::{IntegrityEntry, SHA256_LEN};

/// Streaming SHA-256 hasher.
///
/// `sha2::Sha256` is already a streaming primitive internally; this wrapper
/// exists so the public surface is owned by `elang-integrity` (auditable,
/// named constant for digest length, no leakage of the `sha2` crate's API
/// into the rest of the codebase).
///
/// The intended caller is `elang-container` stage 11: feed each chunk of
/// uncompressed resource bytes to [`Hasher::update`], then call
/// [`Hasher::finalize`] and compare against the matching [`IntegrityEntry`]
/// via [`verify_one`].
#[derive(Clone)]
pub struct Hasher {
    inner: Sha256,
}

impl Hasher {
    /// Start a new SHA-256 hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Sha256::new(),
        }
    }

    /// Feed `bytes` into the hasher. May be called any number of times; the
    /// caller decides the chunk size (typically whatever the Zstd decoder
    /// yields per `decode_chunk` call).
    pub fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }

    /// Consume the hasher and return the 32-byte SHA-256 digest.
    #[must_use]
    pub fn finalize(self) -> [u8; SHA256_LEN] {
        let out = self.inner.finalize();
        out.into()
    }

    /// Consume the hasher and return the digest as 64 lowercase hex
    /// characters (the canonical form the manifest writer emits).
    #[must_use]
    pub fn finalize_hex(self) -> String {
        let digest = self.finalize();
        hex_lower(&digest)
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Hasher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Hasher").finish_non_exhaustive()
    }
}

/// Constant-time equality over two 32-byte SHA-256 digests.
///
/// Branch-free XOR-fold: every byte is read and XORed, the result is OR-folded
/// into a single `u8` accumulator, and the accumulator is compared to zero
/// at the end. No early exit on mismatch — every input pair takes the same
/// number of operations regardless of content.
///
/// This is the only place timing information about digest equality is
/// produced, so it is the only place that needs the discipline. The digests
/// themselves are not secret.
///
/// The index `i` is bounded by `0..SHA256_LEN` (a `pub(crate) const` whose
/// value matches both array lengths), so `a[i]` and `b[i]` are provably in
/// range. The workspace's untrusted-input lint targets accidental
/// indexing; this is deliberate constant-time indexing on a
/// length-known array.
#[must_use]
#[allow(
    clippy::indexing_slicing,
    reason = "i is bounded by 0..SHA256_LEN, which equals both array lengths; the loop is the constant-time body"
)]
pub fn ct_eq_32(a: &[u8; SHA256_LEN], b: &[u8; SHA256_LEN]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..SHA256_LEN {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Verify a single computed SHA-256 digest against a manifest entry's
/// declared digest.
///
/// On match: returns `Ok(())`. On mismatch: returns `Err(INTEGRITY_HASH_MISMATCH)`
/// with the offending resource's path attached, emitted at stage 11 (the
/// decompression-and-verify stage).
///
/// This is the per-resource primitive; the caller (typically
/// `elang-container` stage 11) drives the streaming decompressor, feeds
/// each chunk to a [`Hasher`], finalizes, and calls this function. The
/// caller is responsible for aggregating mismatches across resources — this
/// function reports exactly one mismatch at a time.
///
/// # Errors
///
/// - `INTEGRITY_HASH_MISMATCH` (stage 11) when `computed != expected`,
///   with `resource_path` set to `entry.path`.
pub fn verify_one(entry: &IntegrityEntry, computed: &[u8; SHA256_LEN]) -> Result<(), Error> {
    if ct_eq_32(computed, &entry.digest) {
        return Ok(());
    }
    let computed_hex = hex_lower(computed);
    let expected_hex = hex_lower(&entry.digest);
    Err(integrity_err_for_path(
        "INTEGRITY_HASH_MISMATCH",
        format!(
            "resource '{}': computed digest {computed_hex} != declared digest {expected_hex}",
            entry.path.as_str(),
        ),
        &entry.path,
    ))
}

/// Build an `INTEGRITY` diagnostic at stage 11 with a resource path attached.
///
/// Use for verify-path errors where there is no KDL span to point at
/// (post-parse, streaming context) — the resource path travels on the
/// [`Diagnostic::resource_path`] field so a consumer filtering by stage-11
/// errors can group them by path.
pub(crate) fn integrity_err_for_path(
    code: &'static str,
    message: impl Into<String>,
    path: &ResourcePath,
) -> Error {
    let d = Diagnostic::new(code, Category::Integrity, message)
        .with_stage(11)
        .with_resource(path.as_str());
    Error { diagnostic: d }
}

/// Format `digest` as 64 lowercase hex characters (the canonical form).
///
/// `sha2` does not expose a lowercase-hex formatter directly; `format!("{x:02x}")`
/// on each byte is the standard recipe. Allocation-free apart from the
/// returned `String`.
fn hex_lower(digest: &[u8; SHA256_LEN]) -> String {
    let mut out = String::with_capacity(SHA256_LEN * 2);
    for b in digest {
        // Lowercase hex per ADR 0004 §3 / grammar §4 ("`sha256` value is
        // lowercase hex, never uppercase"). `write!` to a String panics only
        // on allocator failure, which the workspace lints cover elsewhere.
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact digests / error codes; a panic signals a regression"
)]
mod tests {
    use super::*;
    use elang_core::ids::ResourcePath;

    /// SHA-256("abc") per FIPS 180-2 Appendix A.1 — the canonical known
    /// answer for an SHA-256 implementation.
    const SHA256_ABC_HEX: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const SHA256_ABC: [u8; SHA256_LEN] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    /// SHA-256("") per FIPS 180-2 — the canonical empty-input answer.
    const SHA256_EMPTY_HEX: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SHA256_EMPTY: [u8; SHA256_LEN] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];

    fn entry_with_digest(path: &str, digest: [u8; SHA256_LEN]) -> IntegrityEntry {
        IntegrityEntry {
            path: ResourcePath::new(path.to_string()).expect("valid path"),
            digest,
        }
    }

    #[test]
    fn hasher_known_vector_for_abc() {
        let mut h = Hasher::new();
        h.update(b"abc");
        assert_eq!(h.finalize(), SHA256_ABC);
    }

    #[test]
    fn hasher_known_vector_for_empty() {
        let h = Hasher::new();
        assert_eq!(h.finalize(), SHA256_EMPTY);
    }

    #[test]
    fn hasher_streaming_is_byte_equivalent_to_single_update() {
        // "abc" fed one byte at a time must hash identically to one update.
        let mut h = Hasher::new();
        for byte in b"abc" {
            h.update(std::slice::from_ref(byte));
        }
        assert_eq!(h.finalize(), SHA256_ABC);
    }

    #[test]
    fn hasher_zero_length_update_is_a_no_op() {
        // update(&[]) must not affect the hash.
        let mut h = Hasher::new();
        h.update(b"");
        h.update(b"abc");
        assert_eq!(h.finalize(), SHA256_ABC);
    }

    #[test]
    fn finalize_hex_emits_lowercase_64_chars() {
        let h = Hasher::new();
        let hex = h.finalize_hex();
        assert_eq!(hex, SHA256_EMPTY_HEX);
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn ct_eq_32_matches_self() {
        let a = SHA256_ABC;
        assert!(ct_eq_32(&a, &a));
        let b = SHA256_EMPTY;
        assert!(ct_eq_32(&b, &b));
    }

    #[test]
    fn ct_eq_32_distinguishes_distinct_digests() {
        assert!(!ct_eq_32(&SHA256_ABC, &SHA256_EMPTY));
    }

    #[test]
    fn ct_eq_32_detects_single_bit_flip() {
        let mut flipped = SHA256_ABC;
        flipped[0] ^= 0x01;
        assert!(!ct_eq_32(&SHA256_ABC, &flipped));
    }

    #[test]
    fn ct_eq_32_detects_single_bit_flip_in_last_byte() {
        // Specifically guards against an early-exit implementation that
        // might stop at byte 0 and miss a far-flip.
        let mut flipped = SHA256_ABC;
        flipped[SHA256_LEN - 1] ^= 0x80;
        assert!(!ct_eq_32(&SHA256_ABC, &flipped));
    }

    #[test]
    fn verify_one_passes_when_digests_match() {
        let entry = entry_with_digest("Header/format.version", SHA256_ABC);
        verify_one(&entry, &SHA256_ABC).expect("matching digests must pass");
    }

    #[test]
    fn verify_one_rejects_mismatch_with_path_and_stage() {
        let entry = entry_with_digest("Header/format.version", SHA256_ABC);
        let err = verify_one(&entry, &SHA256_EMPTY).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_HASH_MISMATCH");
        assert_eq!(err.diagnostic.category, Category::Integrity);
        assert_eq!(err.diagnostic.stage, Some(11));
        assert_eq!(
            err.diagnostic.resource_path.as_deref(),
            Some("Header/format.version")
        );
        // The message carries both the computed and declared hex so an
        // operator can diagnose without re-reading the manifest.
        assert_eq!(
            err.diagnostic.message,
            format!(
                "resource 'Header/format.version': computed digest {SHA256_EMPTY_HEX} != declared digest {SHA256_ABC_HEX}"
            )
        );
    }

    #[test]
    fn hex_lower_is_lowercase_and_64_chars() {
        let hex = hex_lower(&SHA256_ABC);
        assert_eq!(hex, SHA256_ABC_HEX);
        assert_eq!(hex.len(), 64);
    }
}
