//! Immutable validated EPF and ELD model structures (REQ-CORE-MODEL-003).
//!
//! These are the AST/model types the format revolves around. They hold
//! **already-validated** data: the KDL parsers and schema/semantic validators
//! in `elang-epf` and `elang-eld` (Phases 6, 9–11) construct them after
//! enforcing the full grammar (steering §5, §6, §8). This crate owns the shape
//! and the intrinsic per-node invariants a value can enforce on its own —
//! cardinality bounds and internally-checkable cross-field rules — so that an
//! instance is unrepresentable-if-structurally-invalid.
//!
//! What stays out of this crate: KDL parsing, schema rejection of unknown
//! nodes, the IPA allowlist check, global IPA/romanization uniqueness and
//! `when` reference resolution. Those need external data (the repertoire) or a
//! whole-inventory view and belong to the owning crates per design §5.
//!
//! All types are immutable: fields are private, construction is the only way to
//! obtain a value, and there are no setters.

pub mod eld;
pub mod epf;
pub mod when;

pub use eld::{Allophone, Group, GroupKind, PhonemeInventory, RootPhoneme};
pub use epf::{EpfPackage, LanguageIdentifier, Manifest, Metadata};
pub use when::{Condition, Predicate, SyllablePosition, WhenCondition, WordPosition};
