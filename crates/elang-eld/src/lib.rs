//! # elang-eld
//!
//! ELD phoneme-inventory v0.1.0 Phase 9: structural schema (groups, roots,
//! allophones, romanization) validation.
//!
//! This crate implements constraints **C-01..C-16** from
//! `specs/grammar/eld-0.1.0.md`, covering:
//!
//! * Document and group structure (C-01..C-05),
//! * Root phoneme shape and cardinality (C-06..C-08),
//! * Allophone shape and cardinality (C-10..C-12),
//! * Romanization syntax and uniqueness (C-13..C-15),
//! * Romanization inheritance (C-16),
//! * Global IPA uniqueness including tie-bar aliases (C-09).
//!
//! ## What Phase 9 does NOT do
//!
//! * The `when` condition language (C-20..C-27) — **Phase 10**.
//! * Canonical serialization (steering §9) — **Phase 11**.
//!
//! ## Phase 10 handoff
//!
//! Phase 9 carries `when` blocks **verbatim** in
//! [`crate::model::WhenBlock`]. See that type's documentation for the
//! handoff contract.
//!
//! ## IPA validity
//!
//! The Phase 8 IPA validator (`elang_repertoire::ipa::validate`) is the sole
//! authority for IPA validity, tie-bar normalisation and comparison keys.
//!
//! ## KDL input constraint
//!
//! KDL 2.0 permits bare identifiers as string arguments. Without a node
//! terminator, a bare identifier on the same line is an argument of the
//! preceding node, so `roman "ph" when { … }` parses as a single `roman`
//! node carrying the arguments `"ph"` and `"when"`. This is valid KDL 2.0
//! syntax. Every ELD document must place at most one node per line, matching
//! the canonical ELD serialisation profile (steering §9, Phase 11). The ELD
//! validator rejects the resulting argument count with a shape-error
//! diagnostic rather than identifying a misplaced sibling.
//!
//! Implemented in Phase 9 (Issue #19).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod model;
pub mod parse;

pub use crate::model::{
    EldAllophone, EldDocument, EldGroup, EldGroupKind, EldRootPhoneme, WhenBlock,
};
pub use crate::parse::{parse, parse_document};
pub use elang_core::error::{Category, Error};
