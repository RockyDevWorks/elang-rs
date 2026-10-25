//! Coverage-closure tests for `elang-eld`.
//!
//! The canonical condition-key table maps every enum variant of a
//! predicate to its frozen literal. The crate's own unit tests only walk
//! the `onset` spelling, so the remaining literals are pinned here.
//!
//! Test code path: `.expect()` / panics signal fixture regressions, not
//! untrusted-input failure. The workspace lint policy denies these on
//! production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use elang_core::model::when::SyllablePosition;
use elang_core::model::when::WordPosition;
use elang_eld::canon::key::{syllable_position_value, word_position_value};

/// Every syllable-position variant maps to its frozen ASCII literal.
#[test]
fn syllable_position_literals_cover_every_variant() {
    assert_eq!(syllable_position_value(SyllablePosition::Onset), "onset");
    assert_eq!(
        syllable_position_value(SyllablePosition::Nucleus),
        "nucleus"
    );
    assert_eq!(syllable_position_value(SyllablePosition::Coda), "coda");
}

/// The word-position literals are pinned alongside them so the two tables
/// stay in step.
#[test]
fn word_position_literals_cover_every_variant() {
    assert_eq!(word_position_value(WordPosition::Initial), "initial");
    assert_eq!(word_position_value(WordPosition::Medial), "medial");
    assert_eq!(word_position_value(WordPosition::Final), "final");
}
