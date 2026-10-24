//! # elang-eld
//!
//! ELD phoneme-inventory v0.1.0: structural schema (Phase 9), phonological
//! condition language (Phase 10) and canonical serialization (Phase 11).
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
//! It also implements the `when` condition language (C-20..C-27) and the
//! canonical ELD serialization profile (steering §9 / §9.1).
//!
//! ## Module map
//!
//! | Module | Phase | Role |
//! |---|---|---|
//! | [`model`] | 9 | Structural model; `when` blocks carried verbatim |
//! | [`mod@parse`] | 9 | Structural validation of a KDL document |
//! | [`when`] | 10 | Condition parser, reference resolver, pure evaluator |
//! | [`inventory`] | 11 | Builds the core validated [`PhonemeInventory`](elang_core::model::eld::PhonemeInventory) |
//! | [`canon`] | 11 | Canonical ELD writer ([`canon::to_string`]) |
//!
//! ## Canonical serialization (Phase 11)
//!
//! [`canon::to_string`] turns a validated
//! [`PhonemeInventory`](elang_core::model::eld::PhonemeInventory) into
//! byte-deterministic canonical KDL 2.0. The profile is locked in steering
//! §9/§9.1 and [ADR 0009]; the residual edge cases (authored-byte ordering,
//! authored-byte condition keys, the `"`/`\` escape set, bare-vs-block
//! emission and the layout invariants) are pinned by that ADR.
//!
//! Use [`inventory::from_document`] to turn a parsed document into the model
//! the writer consumes:
//!
//! ```
//! let rep = elang_repertoire::repertoire::load().expect("pinned repertoire loads");
//! let source = concat!(
//!     "phonemes {\n",
//!     "  consonants {\n",
//!     "    phoneme \"t\" {\n",
//!     "      roman \"t\"\n",
//!     "    }\n",
//!     "  }\n",
//!     "  vowels {\n",
//!     "    phoneme \"a\" {\n",
//!     "      roman \"a\"\n",
//!     "    }\n",
//!     "  }\n",
//!     "}\n",
//! );
//! let doc = elang_eld::parse(source.as_bytes(), &rep).expect("valid ELD");
//! let inventory = elang_eld::inventory::from_document(&doc, &rep).expect("model builds");
//! let canonical = elang_eld::canon::to_string(&inventory);
//! assert!(canonical.ends_with("}\n"));
//! ```
//!
//! ## What this crate does NOT do
//!
//! * The IPA repertoire validation (Phase 8) — owned by `elang-repertoire`.
//! * The KDL 2.0 parsing (Phase 5) — owned by `elang-kdl`.
//! * The package reader / public SDK (Phase 12) and the package writer /
//!   CLI (Phase 13).
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
//! Implemented in Phase 9 (Issue #19), Phase 10 (Issue #20) and Phase 11
//! (Issue #21).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod canon;
pub mod error;
pub mod inventory;
pub mod model;
pub mod parse;
pub mod when;

pub use crate::canon::{to_bytes as canonical_bytes, to_string as canonical_string};
pub use crate::model::{
    EldAllophone, EldDocument, EldGroup, EldGroupKind, EldRootPhoneme, WhenBlock,
};
pub use crate::parse::{parse, parse_document};
pub use crate::when::{
    eligibility_for_root, evaluate, max_nesting_depth, parse_all, parse_block, Eligibility,
    EvalContext, InventoryRefs, Realization, MAX_NESTING_DEPTH, MAX_PREDICATES_PER_WHEN,
};
pub use elang_core::error::{Category, Error};
pub use elang_core::model::when::{Condition, Predicate, WhenCondition};
