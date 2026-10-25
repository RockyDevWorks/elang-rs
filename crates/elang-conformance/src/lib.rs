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

// Lint posture for this crate.
//
// This crate has two distinct kinds of code, and they need different
// guarantees:
//
// 1. The **fixture authoring** helpers (`negatives`, `zstd_negatives`,
//    `golden_packages`, the vector modules) build deliberately malformed
//    archives from known-good inputs. When the authoring itself goes
//    wrong — a byte offset that does not line up, a fixture whose
//    mutation failed to apply — there is no "correct" recovery: the
//    corpus would silently publish a fixture that does not test what it
//    claims. Panicking is the intended fail-closed mode, so
//    `expect_used` / `panic` are allowed for these paths.
//
// 2. The **runner** (`corpus::run_fixture`, `corpus::run_document_fixture`,
//    `corpus::run_vector_fixture`) consumes untrusted-shaped bytes — the
//    malformed archives this crate exists to test. It must never panic on
//    malformed input; it returns `Result` and reports a conformance
//    failure instead. `corpus.rs` therefore contains no `expect` /
//    `panic!` / unchecked indexing at all, and the runner is the
//    boundary between the two kinds.
//
// The indexing and arithmetic lints are relaxed for the authoring
// helpers, whose manual byte serialization operates on values that are
// bounds-checked by the surrounding asserts.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::panic
)]

pub mod benchmarks;
pub mod binary_vectors;
pub mod bytewise_ordering;
pub mod canonical_goldens;
pub mod canonical_idempotence;
pub mod catalog;
pub mod condition_vectors;
pub mod corpus;
pub mod epf_canonical_goldens;
pub mod epf_eld_negatives;
pub mod golden_packages;
pub mod inspect_matrix;
pub mod meta_tests;
pub mod negatives;
pub mod normative_manifest;
pub mod parser_budgets;
pub mod peak_memory;
pub mod requirement_coverage_data;
pub mod roundtrip;
pub mod roundtrip_proptest;
pub mod unicode_ipa;
pub mod when_attribution;
pub mod zstd_negatives;
pub mod zstd_vectors;
