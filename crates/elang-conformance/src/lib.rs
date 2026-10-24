//! # elang-conformance
//!
//! Fixture runner and interoperability checks for the
//! `elang-canonical-zstd-v1-conformance` corpus: golden byte comparisons,
//! expected stage/error-code assertions and canonical reproducibility.
//!
//! Most of the runner is implemented in Phase 14. The [`binary_vectors`]
//! module lands early (T0118) because the Phase 3 container reader and the
//! Phase 14 corpus both validate against the published binary header / index /
//! footer golden bytes, and those goldens must be generated — never
//! hand-written — from the exact little-endian layout in steering §3.

// The binary-vector builder is a deterministic developer/fixture tool: it runs
// over hard-coded, trusted inputs (not untrusted archive bytes), so a panic is
// the intended fail-closed mode and the manual byte serialization uses a few
// `as` casts on values already bounds-checked by asserts. Relax the
// untrusted-input lints for this crate accordingly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::panic
)]

pub mod binary_vectors;
pub mod canonical_goldens;
pub mod normative_manifest;
