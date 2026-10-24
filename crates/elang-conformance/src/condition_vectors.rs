//! T1406 + T1409 — condition sorting-key vectors and truth tables.
//!
//! # T1406 — sorting-key byte vectors
//!
//! The Canonical Condition Sorting Key v1 is defined by steering §9.1
//! and implemented in `elang_eld::canon::key`. The byte vectors
//! published here are **pinned** against that specification: they are
//! the exact bytes the implementation must produce for the published
//! conditions. An independent implementation can be checked byte-for-byte
//! without reading this repository's Rust code.
//!
//! ```text
//! atomic_key    = rank:u8 || value_utf8_length:u16BE || exact_validated_value_utf8
//! composite_key = rank:u8 || child_count:u16BE ||
//!                 repeated(sorted_child_key_length:u32BE || sorted_child_key)
//! ```
//!
//! # T1409 — truth tables
//!
//! The condition evaluator (`elang_eld::when::evaluate`) is tested against
//! a published table of (condition, context) → result rows. Every row
//! lists the candidate index, the underlying root sequence, the syllable
//! role, and the expected boolean. Truth tables cover atomic predicates,
//! nested AND (`group`), nested OR, and word-position edge cases
//! (single-phoneme word, two-phoneme word).
//!
//! ## Edge case: single-phoneme word
//!
//! A 1-phoneme root is both `Initial` *and* `Final`; `Medial` never
//! applies (steering §8). Truth-table rows assert this boundary
//! explicitly so a future evaluator change cannot silently invert it.

use crate::corpus::VectorFixture;
use elang_core::ids::IpaIdentifierText;
use elang_core::model::when::{Condition, Predicate, SyllablePosition, WordPosition};
use elang_eld::canon::key::{
    atomic_key, condition_key, RANK_GROUP, RANK_NEXT_PHONEME, RANK_OR, RANK_PREVIOUS_PHONEME,
    RANK_SYLLABLE_POSITION, RANK_WORD_POSITION,
};
use elang_eld::canon::key::{syllable_position_value, word_position_value};

#[cfg(test)]
use crate::corpus::run_vector_fixture;
#[cfg(test)]
use elang_eld::canon::key::{sorted_children, sorted_operands};
#[cfg(test)]
use elang_eld::when::{evaluate, EvalContext, InventoryRefs};

// =======================================================================
// T1406 — sorting-key byte vectors
// =======================================================================

/// Build a [`VectorFixture`] pinning the atomic key for `(rank, value)`.
fn atomic_vector(rank: u8, value: &str, id: &'static str) -> VectorFixture {
    VectorFixture {
        id: id.to_string(),
        description: "pinned atomic sorting key",
        requirements: &["REQ-CANON-005"],
        actual: atomic_key(rank, value),
        expected: expected_atomic(rank, value),
    }
}

/// The pinned expected bytes for an atomic key, derived by hand from the
/// specification (rank || u16BE length || value UTF-8 bytes).
fn expected_atomic(rank: u8, value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + value.len());
    out.push(rank);
    let len = u16::try_from(value.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value.as_bytes());
    out
}

/// Pin a composite key (rank || u16BE count || repeated(u32BE len || key)).
fn expected_composite(rank: u8, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(rank);
    let count = u16::try_from(children.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&count.to_be_bytes());
    for key in children {
        let len = u32::try_from(key.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(key);
    }
    out
}

/// All T1406 vector fixtures, in rank-table order.
#[must_use]
pub fn sorting_key_fixtures() -> Vec<VectorFixture> {
    let mut fixtures = Vec::with_capacity(16);
    fixtures.push(atomic_vector(
        RANK_WORD_POSITION,
        word_position_value(WordPosition::Initial),
        "condition/atomic-key/word-position-initial",
    ));
    fixtures.push(atomic_vector(
        RANK_SYLLABLE_POSITION,
        syllable_position_value(SyllablePosition::Onset),
        "condition/atomic-key/syllable-position-onset",
    ));
    fixtures.push(atomic_vector(
        RANK_PREVIOUS_PHONEME,
        "a",
        "condition/atomic-key/previous-phoneme-a",
    ));
    fixtures.push(atomic_vector(
        RANK_NEXT_PHONEME,
        "a",
        "condition/atomic-key/next-phoneme-a",
    ));

    // Author rank 2 before rank 0 so this pins recursive child sorting too.
    let group_condition = Condition::Group(vec![prev("s"), wp(WordPosition::Initial)]);
    fixtures.push(VectorFixture {
        id: "condition/composite-key/group-of-two".to_string(),
        description: "pinned composite sorting key for a `group` of two children",
        requirements: &["REQ-CANON-005"],
        actual: condition_key(&group_condition),
        expected: vec![
            4, 0, 2, // group rank and u16BE child count
            0, 0, 0, 10, // u32BE length of word-position key
            0, 0, 7, b'i', b'n', b'i', b't', b'i', b'a', b'l', 0, 0, 0,
            4, // u32BE length of previous-phoneme key
            2, 0, 1, b's',
        ],
    });

    // An `or` with two operands, also sorted. The operands are authored in
    // the *wrong* order (rank 3 before rank 0) so that the vector pins the
    // canonicalization itself: `condition_key` sorts the operands before it
    // builds the parent's key, so the emitted bytes must be the left-sorted
    // pair regardless of the authored order.
    let or_condition = Condition::or(next("a"), wp(WordPosition::Initial));
    let or_actual = condition_key(&or_condition);
    let or_expected = expected_composite(
        RANK_OR,
        &[
            atomic_key(RANK_WORD_POSITION, "initial"),
            atomic_key(RANK_NEXT_PHONEME, "a"),
        ],
    );
    fixtures.push(VectorFixture {
        id: "condition/composite-key/or-of-two".to_string(),
        description: "pinned composite sorting key for an `or` of two operands",
        requirements: &["REQ-CANON-005"],
        actual: or_actual,
        expected: or_expected,
    });

    // Rank-table pin: the six ranks are 0..=5 in the documented order.
    fixtures.push(VectorFixture {
        id: "condition/rank-table".to_string(),
        description: "The six ranks are 0..=5 in the documented order",
        requirements: &["REQ-CANON-005"],
        actual: vec![
            RANK_WORD_POSITION,
            RANK_SYLLABLE_POSITION,
            RANK_PREVIOUS_PHONEME,
            RANK_NEXT_PHONEME,
            RANK_GROUP,
            RANK_OR,
        ],
        expected: vec![0, 1, 2, 3, 4, 5],
    });

    fixtures
}

// =======================================================================
// T1409 — condition semantics truth tables
// =======================================================================

/// Build an [`EvalContext`] for a single-phoneme root sequence.
#[cfg(test)]
fn make_ctx<'a>(
    roots: &'a [String],
    index: usize,
    syllable_role: Option<SyllablePosition>,
    refs: &'a InventoryRefs,
) -> EvalContext<'a> {
    EvalContext {
        roots,
        index,
        syllable_role,
        refs,
    }
}

/// Vowels in the IPA allowlist (subset sufficient for the truth table).
#[cfg(test)]
const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "ə", "ɛ"];

/// Build an [`InventoryRefs`] from a minimal ELD document declaring the
/// given underlying roots. Each root is placed in `consonants` or
/// `vowels` per its IPA category so the schema's mandatory-group check
/// passes.
#[cfg(test)]
fn build_refs(roots: &[&str]) -> InventoryRefs {
    let mut vowels_block = String::new();
    let mut consonants_block = String::new();
    for r in roots {
        let entry = format!("        phoneme \"{r}\" {{\n            roman \"{r}\"\n        }}\n");
        if VOWELS.contains(r) {
            vowels_block.push_str(&entry);
        } else {
            consonants_block.push_str(&entry);
        }
    }
    let source = format!(
        "phonemes {{\n    consonants {{\n{consonants_block}    }}\n    vowels {{\n{vowels_block}    }}\n}}\n"
    );
    let rep = elang_repertoire::repertoire::load().expect("repertoire loads");
    match elang_eld::parse(source.as_bytes(), &rep) {
        Ok(doc) => InventoryRefs::from_document(&doc),
        Err(err) => {
            panic!(
                "truth-table reference document must parse cleanly: {:?}\n--- source ---\n{}",
                err, source
            );
        }
    }
}

fn ipa(text: &str) -> IpaIdentifierText {
    IpaIdentifierText::from_validated(text.to_string())
}

fn wp(value: WordPosition) -> Condition {
    Condition::predicate(Predicate::WordPosition(value))
}
fn sp(value: SyllablePosition) -> Condition {
    Condition::predicate(Predicate::SyllablePosition(value))
}
fn prev(text: &str) -> Condition {
    Condition::predicate(Predicate::PreviousPhoneme(ipa(text)))
}
fn next(text: &str) -> Condition {
    Condition::predicate(Predicate::NextPhoneme(ipa(text)))
}

/// A single truth-table row.
///
/// The fields are consumed by the test-only truth-table runner.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "fields are read by the test-only truth-table runner"
)]
pub struct TruthRow {
    /// A stable id for the row.
    id: &'static str,
    /// The candidate index.
    index: usize,
    /// The underlying root-phoneme sequence.
    roots: Vec<String>,
    /// The generator-assigned syllable role.
    syllable_role: Option<SyllablePosition>,
    /// The condition to evaluate.
    condition: WhenCondition,
    /// The expected boolean.
    expected: bool,
}

/// Evaluate every row in `rows` against its context and return the per-row
/// pass/fail record.
#[cfg(test)]
fn run_truth_table(rows: &[TruthRow]) -> Vec<(&'static str, bool, bool, bool)> {
    // The reference map is built from a document declaring the roots used
    // by the table ("a", "b", "c"), so neighbour predicates resolve to
    // their owning root exactly as they do in a real document.
    let refs = build_refs(&["a", "b", "c"]);
    let mut out = Vec::new();
    for row in rows {
        let ctx = make_ctx(&row.roots, row.index, row.syllable_role, &refs);
        let observed = evaluate(&row.condition, &ctx).unwrap_or_else(|err| {
            panic!(
                "truth-table row `{}` must evaluate without error: {:?}",
                row.id, err
            )
        });
        out.push((row.id, row.expected, observed, row.expected == observed));
    }
    out
}

use elang_core::model::when::WhenCondition;

/// All T1409 truth-table rows, plus a quick summary test that exercises them.
#[must_use]
pub fn truth_tables() -> Vec<TruthRow> {
    use Condition::*;
    let mut rows = Vec::new();

    // Atomic: word-position on a 3-phoneme word.
    let roots_3 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    rows.push(TruthRow {
        id: "condition/word-position-initial",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Initial)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/word-position-medial",
        index: 1,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Medial)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/word-position-final",
        index: 2,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Final)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });

    // Edge case: single-phoneme word is both Initial AND Final, never Medial.
    let roots_1 = vec!["a".to_string()];
    rows.push(TruthRow {
        id: "condition/single-phoneme-initial",
        index: 0,
        roots: roots_1.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Initial)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/single-phoneme-final",
        index: 0,
        roots: roots_1.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Final)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/single-phoneme-medial",
        index: 0,
        roots: roots_1.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![wp(WordPosition::Medial)])
            .expect("truth-table condition is non-empty"),
        expected: false,
    });

    // Syllable-role predicate.
    rows.push(TruthRow {
        id: "condition/syllable-onset",
        index: 1,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Onset),
        condition: WhenCondition::new(vec![sp(SyllablePosition::Onset)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/syllable-nucleus",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Nucleus),
        condition: WhenCondition::new(vec![sp(SyllablePosition::Nucleus)])
            .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/syllable-coda-mismatch",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Onset),
        condition: WhenCondition::new(vec![sp(SyllablePosition::Coda)])
            .expect("truth-table condition is non-empty"),
        expected: false,
    });

    // Neighbour predicates.
    rows.push(TruthRow {
        id: "condition/previous-phoneme-match",
        index: 1,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![prev("a")]).expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/previous-phoneme-mismatch",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![prev("a")]).expect("truth-table condition is non-empty"),
        expected: false,
    });
    rows.push(TruthRow {
        id: "condition/next-phoneme-match",
        index: 1,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![next("c")]).expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/next-phoneme-mismatch",
        index: 2,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![next("a")]).expect("truth-table condition is non-empty"),
        expected: false,
    });

    // AND (group).
    rows.push(TruthRow {
        id: "condition/and-both-true",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Onset),
        condition: WhenCondition::new(vec![Group(vec![
            wp(WordPosition::Initial),
            sp(SyllablePosition::Onset),
        ])])
        .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/and-one-false",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Coda),
        condition: WhenCondition::new(vec![Group(vec![
            wp(WordPosition::Initial),
            sp(SyllablePosition::Onset),
        ])])
        .expect("truth-table condition is non-empty"),
        expected: false,
    });

    // OR.
    rows.push(TruthRow {
        id: "condition/or-left-true",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: None,
        condition: WhenCondition::new(vec![Or(
            Box::new(wp(WordPosition::Initial)),
            Box::new(sp(SyllablePosition::Coda)),
        )])
        .expect("truth-table condition is non-empty"),
        expected: true,
    });
    rows.push(TruthRow {
        id: "condition/or-both-false",
        index: 0,
        roots: roots_3.clone(),
        syllable_role: Some(SyllablePosition::Coda),
        condition: WhenCondition::new(vec![Or(
            Box::new(wp(WordPosition::Final)),
            Box::new(sp(SyllablePosition::Onset)),
        )])
        .expect("truth-table condition is non-empty"),
        expected: false,
    });

    rows
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
///
/// The truth table is *not* a [`VectorFixture`] — it is a table of
/// `(condition, context) → result` rows consumed by the test-only
/// evaluator, not pinned bytes — so its catalog entry is declared by
/// hand rather than derived from a fixture list.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    let mut entries: Vec<crate::catalog::CatalogEntry> = sorting_key_fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_vector)
        .collect();
    entries.push(crate::catalog::CatalogEntry {
        id: "condition/truth-tables".to_string(),
        description: "Every published condition truth-table row evaluates to its pinned result",
        requirements: &[
            "REQ-WHEN-001",
            "REQ-WHEN-010",
            "REQ-WHEN-012",
            "REQ-WHEN-013",
            "REQ-WHEN-014",
        ],
        expectation: crate::catalog::FixtureExpectation::Accept,
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorting_key_fixtures_pin_the_specification() {
        let mut checked = 0;
        for fixture in sorting_key_fixtures() {
            run_vector_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
            checked += 1;
        }
        assert_eq!(
            checked, 7,
            "every published sorting-key fixture must be pinned"
        );
    }

    #[test]
    fn truth_tables_match_published_expectations() {
        let rows = truth_tables();
        let mut failed = 0;
        for (id, expected, observed, ok) in run_truth_table(&rows) {
            if !ok {
                failed += 1;
                eprintln!("truth-table row `{id}` failed: expected={expected} observed={observed}");
            }
        }
        assert_eq!(failed, 0, "{failed} truth-table row(s) failed");
        assert_eq!(
            rows.len(),
            17,
            "every published truth-table row must be present"
        );
    }

    #[test]
    fn sorted_children_orders_by_rank() {
        let children = vec![
            Condition::predicate(Predicate::NextPhoneme(ipa("a"))),
            wp(WordPosition::Initial),
            prev("s"),
            sp(SyllablePosition::Onset),
        ];
        let sorted = sorted_children(&children);
        let first_key = condition_key(sorted[0]);
        let last_key = condition_key(sorted[sorted.len() - 1]);
        assert!(
            first_key < last_key,
            "sorted_children must order ascending by key"
        );
    }

    #[test]
    fn sorted_operands_orders_by_key() {
        let or = Condition::or(
            Condition::predicate(Predicate::NextPhoneme(ipa("a"))),
            wp(WordPosition::Initial),
        );
        let Condition::Or(left, right) = &or else {
            panic!("Condition::or must build an Or");
        };
        let [first, second] = sorted_operands(left.as_ref(), right.as_ref());
        let first_key = condition_key(first);
        let second_key = condition_key(second);
        assert!(
            first_key <= second_key,
            "sorted_operands must order ascending by key"
        );
    }
}
