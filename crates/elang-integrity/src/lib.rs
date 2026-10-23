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
//!
//! ### Deferred to the writer-and-docs PR (T0709, T0711)
//!
//! - [`canonical_bytes`](crate::write) — T0709 canonical manifest writer
//!   (lock-step with the grammar's serialization rules, ADR 0004 §"Canonical
//!   serialization").
//! - The "SHA-256 is not a publisher signature" doc page — T0711,
//!   REQ-INTEGRITY-008.
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
//! integrity, not identity (steering §0.8, REQ-INTEGRITY-008).
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

pub use elang_core::ids::ResourcePath;

pub use crate::manifest::{
    verify_coverage, Algorithm, IntegrityEntry, IntegrityManifest, SHA_256_LITERAL,
};
pub use crate::parse::{parse, parse_document, parse_with_budgets};
pub use crate::verify::{verify_one, Hasher};
