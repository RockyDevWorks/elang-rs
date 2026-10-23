//! # elang-integrity
//!
//! `META-INF/integrity.kdl` parsing, structural validation, and (in a later
//! round) SHA-256 verification over the exact uncompressed bytes of every
//! indexed resource except the manifest itself.
//!
//! ## Scope of this crate (T0701–T0705, this PR)
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
//!
//! ## Not yet in this crate (T0706–T0711, follow-up PRs)
//!
//! - Streaming SHA-256 over exact uncompressed bytes (T0706, T0707).
//! - Constant-time digest compare + per-resource failure reporting (T0707).
//! - Footer index hash separation (T0708).
//! - Canonical integrity manifest writer (T0709).
//! - Full conformance corpus (T0710).
//! - Documentation that SHA-256 is not a publisher signature (T0711,
//!   REQ-INTEGRITY-008).
//!
//! ## Error model
//!
//! Every diagnostic carries a code from `specs/registry/error-codes.{md,json}`
//! under the `INTEGRITY` category. Stage-9 codes (`INTEGRITY_STRUCTURE_INVALID`,
//! `INTEGRITY_BAD_ALGORITHM`, `INTEGRITY_BAD_HEX`, `INTEGRITY_MISSING_ENTRY`,
//! `INTEGRITY_EXTRA_ENTRY`, `INTEGRITY_DUPLICATE_ENTRY`) are emitted here;
//! `INTEGRITY_HASH_MISMATCH` (stage 11) will be emitted by the verify path in
//! the follow-up PR. KDL parser errors propagate with their own category.
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
//! [`ADR 0004`](../../../docs/decisions/0004-integrity-manifest-grammar.md)
//! for the normative artifacts this crate implements.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod manifest;
pub mod parse;

pub use elang_core::ids::ResourcePath;

pub use crate::manifest::{verify_coverage, Algorithm, IntegrityEntry, IntegrityManifest};
pub use crate::parse::{parse, parse_document, parse_with_budgets};
