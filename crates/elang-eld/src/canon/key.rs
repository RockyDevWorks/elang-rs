//! Canonical Condition Sorting Key v1 (steering §9.1, REQ-CANON-005).
//!
//! The key is the frozen binary discipline that decides the order of the
//! direct children of a `when`, a `group` and an `or`. It is defined by
//! steering §9.1 and is **locked** — changing a rank or a field layout
//! requires a superseding ADR (see
//! [`docs/locked-rules.md`](../../../../docs/locked-rules.md)).
//!
//! ```text
//! atomic_key    = rank:u8 || value_utf8_length:u16BE || exact_validated_value_utf8
//! composite_key = rank:u8 || child_count:u16BE ||
//!                 repeated(sorted_child_key_length:u32BE || sorted_child_key)
//! ```
//!
//! * The rank is the node's position in the frozen rank table: `word-position`
//!   = 0, `syllable-position` = 1, `previous-phoneme` = 2, `next-phoneme` = 3,
//!   `group` = 4, `or` = 5. `when` has **no** rank — it is a root expression
//!   container, not a sortable node.
//! * All lengths are **big-endian**.
//! * Keys are compared as **unsigned lexicographic byte strings**.
//! * Children are sorted recursively **before** their parent's key is built,
//!   so a composite key always describes the *sorted* subtree.
//!
//! The key is a faithful, injective encoding of a condition tree: two
//! conditions have equal keys exactly when they are the same tree with the
//! same leaf values. That is what makes the sort deterministic even when two
//! children carry the same key (duplicate predicates) — their relative order
//! cannot change the emitted bytes.

use elang_core::model::when::{Condition, Predicate, SyllablePosition, WordPosition};

/// Rank of a `word-position` atomic predicate.
pub const RANK_WORD_POSITION: u8 = 0;
/// Rank of a `syllable-position` atomic predicate.
pub const RANK_SYLLABLE_POSITION: u8 = 1;
/// Rank of a `previous-phoneme` atomic predicate.
pub const RANK_PREVIOUS_PHONEME: u8 = 2;
/// Rank of a `next-phoneme` atomic predicate.
pub const RANK_NEXT_PHONEME: u8 = 3;
/// Rank of a `group` composite node.
pub const RANK_GROUP: u8 = 4;
/// Rank of an `or` composite node.
pub const RANK_OR: u8 = 5;

/// The canonical value literal of a `word-position` predicate.
///
/// The three literals are the fixed ASCII enum values of the condition
/// language (steering §8, C-22); they are used verbatim in both the emitted
/// document and the atomic sorting key.
#[must_use]
pub fn word_position_value(value: WordPosition) -> &'static str {
    match value {
        WordPosition::Initial => "initial",
        WordPosition::Medial => "medial",
        WordPosition::Final => "final",
    }
}

/// The canonical value literal of a `syllable-position` predicate.
///
/// The three literals are the fixed ASCII enum values of the condition
/// language (steering §8, C-22); they are used verbatim in both the emitted
/// document and the atomic sorting key.
#[must_use]
pub fn syllable_position_value(value: SyllablePosition) -> &'static str {
    match value {
        SyllablePosition::Onset => "onset",
        SyllablePosition::Nucleus => "nucleus",
        SyllablePosition::Coda => "coda",
    }
}

/// Build an atomic key: `rank:u8 || value_utf8_length:u16BE || value_utf8`.
///
/// `value` is the **exact validated value** in its authored NFC UTF-8 bytes
/// (steering §9.1). For a `previous-phoneme` / `next-phoneme` predicate that
/// is the authored reference exactly as written in the source — not the owning
/// root's identifier and not the tie-bar-folded comparison key
/// ([ADR 0009 E-2](../../../../docs/decisions/0009-residual-eld-canonicalization-edge-cases.md)).
#[must_use]
pub fn atomic_key(rank: u8, value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    // The value is an IPA identifier (≤ 8 scalars, so ≤ 32 UTF-8 bytes) or a
    // fixed ASCII enum literal, so the `u16` length is always lossless. The
    // saturating fallback keeps the function total for hypothetical input
    // outside that bound instead of panicking.
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    let mut out = Vec::with_capacity(3usize.saturating_add(bytes.len()));
    out.push(rank);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Build a composite key:
/// `rank:u8 || child_count:u16BE || repeated(child_length:u32BE || child_key)`.
///
/// `child_keys` must already be in canonical (sorted) order — the caller is
/// responsible for sorting the children recursively before building the
/// parent's key (steering §9.1).
#[must_use]
pub fn composite_key(rank: u8, child_keys: &[Vec<u8>]) -> Vec<u8> {
    let count = u16::try_from(child_keys.len()).unwrap_or(u16::MAX);
    let mut out = Vec::new();
    out.push(rank);
    out.extend_from_slice(&count.to_be_bytes());
    for key in child_keys {
        // A key is at most a few hundred bytes (16 predicates, 8 levels), so
        // the `u32` length is always lossless; the saturating fallback keeps
        // the function total rather than panicking.
        let len = u32::try_from(key.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(key);
    }
    out
}

/// The canonical key of an atomic predicate.
#[must_use]
pub fn predicate_key(predicate: &Predicate) -> Vec<u8> {
    match predicate {
        Predicate::WordPosition(v) => atomic_key(RANK_WORD_POSITION, word_position_value(*v)),
        Predicate::SyllablePosition(v) => {
            atomic_key(RANK_SYLLABLE_POSITION, syllable_position_value(*v))
        }
        Predicate::PreviousPhoneme(ipa) => atomic_key(RANK_PREVIOUS_PHONEME, ipa.as_str()),
        Predicate::NextPhoneme(ipa) => atomic_key(RANK_NEXT_PHONEME, ipa.as_str()),
    }
}

/// The canonical key of a condition expression.
///
/// For a composite node the children are sorted recursively first, and the
/// parent's key is then built from the **sorted** child keys
/// (steering §9.1). The `or` node's two operands are sorted the same way.
#[must_use]
pub fn condition_key(condition: &Condition) -> Vec<u8> {
    match condition {
        Condition::Predicate(p) => predicate_key(p),
        Condition::Group(children) => {
            let keys: Vec<Vec<u8>> = sorted_children(children)
                .into_iter()
                .map(condition_key)
                .collect();
            composite_key(RANK_GROUP, &keys)
        }
        Condition::Or(left, right) => {
            let keys: Vec<Vec<u8>> = sorted_operands(left, right)
                .into_iter()
                .map(condition_key)
                .collect();
            composite_key(RANK_OR, &keys)
        }
    }
}

/// The children of a `when` or `group`, ordered by their canonical key.
///
/// The sort is stable, so children that carry the *same* key (duplicate
/// predicates) keep their authored relative order. Because the key is
/// injective over condition trees, two children with the same key also
/// serialize identically — the relative order cannot change the output bytes.
#[must_use]
pub fn sorted_children(children: &[Condition]) -> Vec<&Condition> {
    let mut sorted: Vec<&Condition> = children.iter().collect();
    sorted.sort_by_key(|c| condition_key(c));
    sorted
}

/// The two operands of an `or`, ordered by their canonical key.
#[must_use]
pub fn sorted_operands<'a>(left: &'a Condition, right: &'a Condition) -> [&'a Condition; 2] {
    let mut sorted = [left, right];
    sorted.sort_by_key(|c| condition_key(c));
    sorted
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "condition-key tests assert exact byte layouts; a panic signals a canonicalization regression"
)]
mod tests {
    use super::*;
    use elang_core::ids::IpaIdentifierText;

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

    // ---- rank table (T1108) ------------------------------------------

    #[test]
    fn ranks_match_the_frozen_table() {
        assert_eq!(RANK_WORD_POSITION, 0);
        assert_eq!(RANK_SYLLABLE_POSITION, 1);
        assert_eq!(RANK_PREVIOUS_PHONEME, 2);
        assert_eq!(RANK_NEXT_PHONEME, 3);
        assert_eq!(RANK_GROUP, 4);
        assert_eq!(RANK_OR, 5);
    }

    #[test]
    fn predicate_keys_carry_their_rank() {
        assert_eq!(
            predicate_key(&Predicate::WordPosition(WordPosition::Initial))[0],
            0
        );
        assert_eq!(
            predicate_key(&Predicate::SyllablePosition(SyllablePosition::Onset))[0],
            1
        );
        assert_eq!(predicate_key(&Predicate::PreviousPhoneme(ipa("s")))[0], 2);
        assert_eq!(predicate_key(&Predicate::NextPhoneme(ipa("s")))[0], 3);
    }

    // ---- atomic key layout (T1109) -----------------------------------

    #[test]
    fn atomic_key_layout_is_rank_length_value() {
        let key = atomic_key(RANK_WORD_POSITION, "initial");
        assert_eq!(key, vec![0, 0, 7, b'i', b'n', b'i', b't', b'i', b'a', b'l']);
    }

    #[test]
    fn atomic_key_length_is_big_endian() {
        // A 256-byte value needs two length bytes; the high byte comes first.
        let value = "a".repeat(256);
        let key = atomic_key(RANK_NEXT_PHONEME, &value);
        assert_eq!(key[0], RANK_NEXT_PHONEME);
        assert_eq!(key[1], 0x01);
        assert_eq!(key[2], 0x00);
        assert_eq!(key.len(), 3 + 256);
    }

    #[test]
    fn atomic_key_uses_the_authored_bytes_of_the_reference() {
        // ADR 0009 E-2: the authored reference bytes go into the key — the
        // owning root is NOT substituted and the tie bar is NOT folded.
        let key = atomic_key(RANK_PREVIOUS_PHONEME, "t\u{035C}");
        assert_eq!(&key[3..], "t\u{035C}".as_bytes());
        assert_ne!(&key[3..], "t\u{0361}".as_bytes());
    }

    // ---- composite key layout (T1110) ---------------------------------

    #[test]
    fn composite_key_layout_is_rank_count_then_length_prefixed_children() {
        let child = atomic_key(RANK_WORD_POSITION, "initial");
        let key = composite_key(RANK_GROUP, std::slice::from_ref(&child));
        let mut expected = vec![RANK_GROUP, 0, 1];
        expected.extend_from_slice(&u32::try_from(child.len()).unwrap().to_be_bytes());
        expected.extend_from_slice(&child);
        assert_eq!(key, expected);
    }

    #[test]
    fn composite_key_count_is_big_endian() {
        let children: Vec<Vec<u8>> = (0..300).map(|_| atomic_key(RANK_GROUP, "x")).collect();
        let key = composite_key(RANK_GROUP, &children);
        assert_eq!(key[0], RANK_GROUP);
        assert_eq!(key[1], 0x01);
        assert_eq!(key[2], 0x2C);
    }

    // ---- recursive sorting (T1111) ------------------------------------

    #[test]
    fn children_are_sorted_by_rank_then_value() {
        let children = vec![
            next("a"),
            wp(WordPosition::Initial),
            prev("s"),
            sp(SyllablePosition::Onset),
        ];
        let sorted = sorted_children(&children);
        let names: Vec<&str> = sorted
            .iter()
            .map(|c| match c {
                Condition::Predicate(Predicate::WordPosition(_)) => "word-position",
                Condition::Predicate(Predicate::SyllablePosition(_)) => "syllable-position",
                Condition::Predicate(Predicate::PreviousPhoneme(_)) => "previous-phoneme",
                Condition::Predicate(Predicate::NextPhoneme(_)) => "next-phoneme",
                _ => "other",
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "word-position",
                "syllable-position",
                "previous-phoneme",
                "next-phoneme"
            ]
        );
    }

    #[test]
    fn children_with_the_same_rank_sort_by_value() {
        let children = vec![wp(WordPosition::Medial), wp(WordPosition::Final)];
        let sorted = sorted_children(&children);
        let values: Vec<&str> = sorted
            .iter()
            .map(|c| match c {
                Condition::Predicate(Predicate::WordPosition(v)) => word_position_value(*v),
                _ => "other",
            })
            .collect();
        assert_eq!(values, vec!["final", "medial"]);
    }

    #[test]
    fn composite_nodes_sort_after_atomic_nodes() {
        let group = Condition::group(vec![wp(WordPosition::Final), prev("s")]).unwrap();
        let children = vec![group.clone(), wp(WordPosition::Initial)];
        let sorted = sorted_children(&children);
        assert!(matches!(sorted[0], Condition::Predicate(_)));
        assert!(matches!(sorted[1], Condition::Group(_)));
    }

    #[test]
    fn or_operands_are_sorted() {
        let or = Condition::or(next("a"), wp(WordPosition::Initial));
        let Condition::Or(left, right) = &or else {
            panic!("`Condition::or` must build an `Or` node");
        };
        let sorted = sorted_operands(left.as_ref(), right.as_ref());
        assert!(matches!(
            sorted[0],
            Condition::Predicate(Predicate::WordPosition(_))
        ));
        assert!(matches!(
            sorted[1],
            Condition::Predicate(Predicate::NextPhoneme(_))
        ));
    }

    #[test]
    fn condition_key_of_a_group_embeds_the_sorted_child_keys() {
        let group = Condition::group(vec![next("a"), wp(WordPosition::Initial)]).unwrap();
        let key = condition_key(&group);
        assert_eq!(key[0], RANK_GROUP);
        assert_eq!(key[1], 0);
        assert_eq!(key[2], 2);
        // First embedded child is the word-position (rank 0), second the
        // next-phoneme (rank 3): the children were sorted before the key.
        let first_len = u32::from_be_bytes([key[3], key[4], key[5], key[6]]);
        assert_eq!(key[7], RANK_WORD_POSITION);
        assert_eq!(first_len as usize, 3 + "initial".len());
    }

    #[test]
    fn equal_keys_imply_equal_trees() {
        // Two structurally identical conditions produce identical keys.
        let a = Condition::group(vec![wp(WordPosition::Final), prev("s")]).unwrap();
        let b = Condition::group(vec![wp(WordPosition::Final), prev("s")]).unwrap();
        assert_eq!(condition_key(&a), condition_key(&b));
    }

    #[test]
    fn different_trees_produce_different_keys() {
        // A flat `group` and a nested `group` are different trees.
        let flat = Condition::group(vec![wp(WordPosition::Final), prev("s")]).unwrap();
        let nested = Condition::group(vec![
            Condition::group(vec![wp(WordPosition::Final), prev("s")]).unwrap(),
            prev("z"),
        ])
        .unwrap();
        assert_ne!(condition_key(&flat), condition_key(&nested));
    }
}
