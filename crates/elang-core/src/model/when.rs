//! Immutable validated `when` condition AST (REQ-CORE-MODEL-003, REQ-WHEN-*).
//!
//! The AST models the condition language from steering §8: four atomic
//! predicates, two AND containers (the top-level `when` and nested `group`) and
//! a binary `or`. Instances hold **already-validated** structure: the parser
//! (Phase 10, `elang-eld`) builds them after enforcing value domains, arity,
//! and the depth/predicate budgets. The constructors here enforce the intrinsic
//! shape invariants a node can check on its own — `group` needs ≥ 2 children and
//! `or` needs exactly 2 — so a malformed container is unrepresentable.
//!
//! The tree is preserved as authored (no Boolean simplification, no flattening,
//! duplicate and contradictory predicates retained — REQ-WHEN-011).

use crate::error::{Category, Error};
use crate::ids::IpaIdentifierText;

/// Word position relative to the underlying root-phoneme sequence (steering §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordPosition {
    /// First underlying root phoneme in the word.
    Initial,
    /// Neither first nor last.
    Medial,
    /// Last underlying root phoneme in the word.
    Final,
}

/// Generator-assigned syllable component (steering §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyllablePosition {
    /// Syllable onset.
    Onset,
    /// Syllable nucleus.
    Nucleus,
    /// Syllable coda.
    Coda,
}

/// An atomic leaf predicate (steering §8 / REQ-WHEN-005..007).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Predicate {
    /// `word-position "<value>"`.
    WordPosition(WordPosition),
    /// `syllable-position "<value>"`.
    SyllablePosition(SyllablePosition),
    /// `previous-phoneme "<ipa>"` — references an IPA identifier.
    PreviousPhoneme(IpaIdentifierText),
    /// `next-phoneme "<ipa>"` — references an IPA identifier.
    NextPhoneme(IpaIdentifierText),
}

/// A condition expression node: an atomic predicate, an AND group, or a binary
/// OR (steering §8 / REQ-WHEN-001).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// An atomic leaf predicate.
    Predicate(Predicate),
    /// `group { .. }`: AND of ≥ 2 direct children.
    Group(Vec<Condition>),
    /// `or { a b }`: OR of exactly 2 children.
    Or(Box<Condition>, Box<Condition>),
}

impl Condition {
    /// A leaf predicate condition.
    #[must_use]
    pub fn predicate(p: Predicate) -> Self {
        Condition::Predicate(p)
    }

    /// Build an AND `group` from its children, enforcing ≥ 2 direct children
    /// (REQ-WHEN-003).
    pub fn group(children: Vec<Condition>) -> Result<Self, Error> {
        if children.len() < 2 {
            return Err(err(
                "WHEN_GROUP_ARITY",
                "group requires at least 2 children",
            ));
        }
        Ok(Condition::Group(children))
    }

    /// Build a binary `or` (REQ-WHEN-003: exactly 2 operands).
    #[must_use]
    pub fn or(left: Condition, right: Condition) -> Self {
        Condition::Or(Box::new(left), Box::new(right))
    }
}

/// The top-level `when` block: an implicit AND of ≥ 1 direct child expression
/// (steering §8 / REQ-WHEN-002). An explicit empty `when {}` is invalid, which
/// the non-empty constructor enforces here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhenCondition {
    children: Vec<Condition>,
}

impl WhenCondition {
    /// Build a `when` block, enforcing ≥ 1 direct child (REQ-WHEN-002).
    pub fn new(children: Vec<Condition>) -> Result<Self, Error> {
        if children.is_empty() {
            return Err(err("WHEN_EMPTY", "when block must have at least one child"));
        }
        Ok(Self { children })
    }

    /// The direct child expressions (implicitly ANDed).
    #[must_use]
    pub fn children(&self) -> &[Condition] {
        &self.children
    }
}

fn err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::When, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn wp(p: WordPosition) -> Condition {
        Condition::predicate(Predicate::WordPosition(p))
    }

    fn prev(ipa: &str) -> Condition {
        Condition::predicate(Predicate::PreviousPhoneme(
            IpaIdentifierText::from_validated(ipa.to_string()),
        ))
    }

    #[test]
    fn when_requires_at_least_one_child() {
        assert_eq!(WhenCondition::new(vec![]).unwrap_err().code(), "WHEN_EMPTY");
        let w = WhenCondition::new(vec![wp(WordPosition::Initial)]).unwrap();
        assert_eq!(w.children().len(), 1);
    }

    #[test]
    fn group_requires_two_children() {
        assert_eq!(
            Condition::group(vec![wp(WordPosition::Final)])
                .unwrap_err()
                .code(),
            "WHEN_GROUP_ARITY"
        );
        let g = Condition::group(vec![
            wp(WordPosition::Final),
            Condition::predicate(Predicate::SyllablePosition(SyllablePosition::Coda)),
        ])
        .unwrap();
        // Exercise both match arms: `g` is a Group, the predicate falls to
        // the non-Group arm (asserted as empty below).
        let mut lens = Vec::new();
        for cond in [
            g,
            Condition::predicate(Predicate::SyllablePosition(SyllablePosition::Onset)),
        ] {
            match cond {
                Condition::Group(children) => lens.push(children.len()),
                _ => lens.push(0),
            }
        }
        assert_eq!(lens, vec![2, 0]);
    }

    #[test]
    fn or_is_binary_and_nests() {
        // Mirrors the steering §8 example: or { group { .. } previous-phoneme "s" }.
        let inner = Condition::group(vec![
            wp(WordPosition::Final),
            Condition::predicate(Predicate::SyllablePosition(SyllablePosition::Coda)),
        ])
        .unwrap();
        let or = Condition::or(inner, prev("s"));
        let when = WhenCondition::new(vec![or]).unwrap();
        // Exercise both match arms: the fixture is an Or, the predicate is not.
        let mut fallback_seen = false;
        for cond in [
            when.children().first().unwrap(),
            &Condition::predicate(Predicate::SyllablePosition(SyllablePosition::Onset)),
        ] {
            match cond {
                Condition::Or(l, r) => {
                    assert!(matches!(**l, Condition::Group(_)));
                    assert!(matches!(
                        **r,
                        Condition::Predicate(Predicate::PreviousPhoneme(_))
                    ));
                }
                _ => fallback_seen = true,
            }
        }
        assert!(fallback_seen, "non-Or condition must hit the fallback arm");
    }

    #[test]
    fn duplicate_and_contradictory_predicates_are_retained() {
        // REQ-WHEN-011: no simplification; both children kept verbatim.
        let when =
            WhenCondition::new(vec![wp(WordPosition::Initial), wp(WordPosition::Final)]).unwrap();
        assert_eq!(when.children().len(), 2);
    }
}
