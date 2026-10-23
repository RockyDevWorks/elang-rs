//! # elang-integrity
//!
//! `META-INF/integrity.kdl` parsing, structural validation, and SHA-256
//! verification over the exact uncompressed bytes of every indexed resource
//! except the manifest itself.
//!
//! ## Scope (Phase 7, Issue #17 — T0701..T0711)
//!
//! ### Done in this crate
//!
//! - [`parse`] / [`parse_with_budgets`] — parse the manifest under the 16 MiB
//!   cap (`MAX_INTEGRITY_BYTES`) using KDL 2.0 with parser budgets.
//! - [`parse_document`] — enforce constraints C-01..C-05 from
//!   [`specs/grammar/integrity-0.1.0.md`](../../../specs/grammar/integrity-0.1.0.md):
//!   exactly one `integrity` node, exactly one `algorithm` node with the
//!   `"sha-256"` arg, one `resource` per entry with a single `sha256`
//!   64-lowercase-hex property, no duplicates, no unknown constructs.
//! - [`IntegrityManifest`] / [`IntegrityEntry`] / [`Algorithm`] — the validated
//!   semantic model.
//! - [`verify_coverage`] — enforce constraint C-04: every indexed resource
//!   except `META-INF/integrity.kdl` has exactly one entry, and there are no
//!   entries for non-indexed paths.
//! - [`Hasher`] — streaming SHA-256 accumulator fed by the container's
//!   decompressor (stage 11).
//! - [`verify_one`] — per-resource constant-time digest comparison; emits
//!   [`INTEGRITY_HASH_MISMATCH`] at stage 11 with the resource path attached
//!   on mismatch.
//! - [`canonical_bytes`] — emit the canonical byte sequence per ADR 0004
//!   §"Canonical serialization". [`CANONICAL_GOLDEN`] is the byte-for-byte
//!   fixture; [`parse`] + [`canonical_bytes`] form a stable roundtrip.
//!
//! ### Deferred
//!
//! None — all of Phase 7 (T0701..T0711, REQ-INTEGRITY-001..008) lands in
//! this crate. T1618 (release-notes language about integrity vs publisher
//! authenticity) is Phase 16 and is owned by the release-management work.
//!
//! ## Error model
//!
//! Every diagnostic carries a code from `specs/registry/error-codes.{md,json}`.
//! Stage-9 codes (`INTEGRITY_STRUCTURE_INVALID`, `INTEGRITY_BAD_ALGORITHM`,
//! `INTEGRITY_BAD_HEX`, `INTEGRITY_MISSING_ENTRY`, `INTEGRITY_EXTRA_ENTRY`,
//! `INTEGRITY_DUPLICATE_ENTRY`) are emitted from [`parse`] /
//! [`parse_document`]. Stage-11 codes (`INTEGRITY_HASH_MISMATCH`) are emitted
//! from [`verify_one`]. KDL parser errors propagate with their own category.
//! NFC / forbidden-category violations on `resource` path args surface with
//! the `UNICODE` category.
//!
//! ## What this crate does not do
//!
//! It does not parse `package.epf` (that is `elang-epf`); it does not parse
//! `.eld` files (that is `elang-eld`); it does not own the container header,
//! footer, frame I/O or Zstd reader (that is `elang-container`); and it does
//! **not** verify publisher authenticity — SHA-256 resource hashes prove
//! integrity, not identity.
//!
//! **SHA-256 integrity is not a publisher signature.** A matching digest
//! proves the package's contents are intact and unaltered; it does **not**
//! identify who produced the package. URN-like language identifiers do not
//! prove ownership; ELANG v0.1.0 defines no publisher signatures; a verified
//! hash is not a trust decision about the publisher. This is the explicit
//! non-goal of REQ-INTEGRITY-008 / T0711; see
//! [`docs/public/format/integrity.md`](../../../docs/public/format/integrity.md#integrity-is-not-authorship)
//! for the user-facing treatment.
//!
//! Implemented in Phase 7 (Issue #17). See `specs/grammar/integrity-0.1.0.md`,
//! `specs/grammar/integrity-0.1.0.abnf` and
//! [ADR 0004](../../../docs/decisions/0004-integrity-manifest-grammar.md)
//! for the normative artifacts this crate implements.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod manifest;
pub mod parse;
pub mod verify;
pub mod write;

pub use elang_core::ids::ResourcePath;

pub use crate::manifest::{
    verify_coverage, Algorithm, IntegrityEntry, IntegrityManifest, SHA_256_LITERAL,
};
pub use crate::parse::{parse, parse_document, parse_with_budgets};
pub use crate::verify::{verify_one, Hasher};
pub use crate::write::{canonical_bytes, CANONICAL_GOLDEN};
