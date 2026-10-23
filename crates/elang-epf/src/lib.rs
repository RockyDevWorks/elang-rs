//! # elang-epf
//!
//! `package.epf` v0.1.0 schema parsing, semantic validation, manifest
//! resolution, and canonical KDL serialization.
//!
//! `package.epf` is the package-definition layer of ELANG. It declares the
//! language identity, schema versions, metadata, and a reference to the
//! phoneme `.eld` resource contained within an `.elang` package. This crate
//! parses the KDL 2.0 document, enforces the full frozen grammar in
//! [`specs/grammar/epf-0.1.0.md`](../../specs/grammar/epf-0.1.0.md), resolves
//! the phoneme path against the archive's resource index, and emits a
//! byte-deterministic canonical form ratified by [ADR 0005].
//!
//! ## Entry points
//!
//! - [`parse`] / [`parse_with_budgets`] — parse + validate the EPF document
//!   (constraints C-01..C-11).
//! - [`manifest::resolve_phonemes`] — perform the cross-field C-12 check that
//!   the declared phoneme resource exists in the archive index.
//! - [`canon::to_string`] / [`canon::to_bytes`] — emit the canonical form
//!   (T0616, T0617).
//!
//! ## Error model
//!
//! Every diagnostic is one of the registry-codes published in
//! `specs/registry/error-codes.md` under the `EPF_SCHEMA` category (stage
//! 10), except for KDL parser errors (which retain their own `KDL_*` codes)
//! and Unicode NFC / category violations (which surface as `UNICODE_*`
//! codes — grammar §1 mandates NFC on all string values, enforced here).
//!
//! ## What this crate does not do
//!
//! It does not implement ELD phoneme definitions, allophone semantics, IPA
//! repertoire validation or `when` predicate evaluation — those belong to
//! `elang-eld`. It does not verify SHA-256 resource integrity; that belongs
//! to `elang-integrity` (Phase 7). It does not prove URN namespace ownership
//! (steering §0.8).
//!
//! Implemented in Phase 6 (Issue #16). See `specs/grammar/epf-0.1.0.md` and
//! `docs/decisions/0005-canonical-epf-serialization.md` for the normative
//! artifacts this crate implements.
//!
//! [ADR 0005]: ../../docs/decisions/0005-canonical-epf-serialization.md

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod canon;
pub mod error;
pub mod manifest;
pub mod parse;

pub use elang_core::model::epf as model;
pub use elang_core::ResourcePath;

pub use crate::canon::{to_bytes as canonical_bytes, to_string as canonical};
pub use crate::manifest::{has_eld_extension, IndexLookup};
pub use crate::parse::{parse, parse_document, parse_error, parse_with_budgets};