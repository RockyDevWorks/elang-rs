//! Canonical ELD serializer (Phase 11 — T1101..T1121, REQ-CANON-001..009).
//!
//! Implements the canonical ELD serialization profile locked in steering §9
//! and §9.1, together with
//! [ADR 0009](../../../../docs/decisions/0009-residual-eld-canonicalization-edge-cases.md)
//! (the residual edge cases). The public description of the profile lives in
//! [`docs/public/format/canonical.md`](../../../../docs/public/format/canonical.md).
//!
//! # What the writer guarantees
//!
//! * **One `phonemes` root.** The inventory is emitted as exactly one
//!   `phonemes` node with a nonempty block (T1101).
//! * **Canonical ordering.** Groups are `consonants`, `vowels`, then custom
//!   groups in ASCII bytewise order (T1102); roots and allophones are sorted
//!   by their **authored NFC IPA UTF-8 bytes** (T1103, T1104). Per ADR 0009
//!   E-1 the tie-bar-folded comparison key is used for *uniqueness* only,
//!   never for ordering or emission.
//! * **Canonical child order.** A root emits `roman`, then `when` (if any),
//!   then its allophones (T1105); an allophone emits an explicit `roman`
//!   (if any), then `when` (if any) (T1106); a fieldless allophone is a bare
//!   leaf with no braces (T1107).
//! * **Canonical condition order.** The direct children of a `when`, a
//!   `group` and an `or` are sorted recursively by the Canonical Condition
//!   Sorting Key v1 (T1108..T1111) — see [`key`].
//! * **Authored-value preservation.** Duplicate predicates, contradictory
//!   conditions, the original Boolean tree nesting, the authored IPA
//!   reference (including its tie-bar variant) and the romanization case are
//!   all preserved verbatim (T1112, T1113). The writer performs **no**
//!   Boolean simplification, no flattening, no alias rewriting and no
//!   silent Unicode normalization (T1117).
//! * **Canonical formatting.** Four ASCII spaces per nesting level, one
//!   declaration per line, LF line endings, no blank lines, no comments and
//!   exactly one final LF (T1114, T1115). Strings are standard
//!   double-quoted and escape **only** `"` and `\` (T1116).
//!
//! # What the writer does NOT do
//!
//! * It does not validate. The input is an already-validated
//!   [`PhonemeInventory`]; the writer never repairs a model it is given.
//! * It does not implement the ELD grammar, the IPA repertoire validator or
//!   the condition evaluator — those belong to Phases 8, 9 and 10.
//! * It does not re-read or re-parse its own output. Use
//!   [`crate::inventory::from_document`] to turn a document into the model
//!   this writer consumes.
//!
//! # Idempotence
//!
//! `canon(canon(x)) == canon(x)` (T1118): the ordering rules are total
//! orders, the condition sort is stable over an injective key, and the
//! emitted form re-parses into the same model.

use elang_core::model::eld::{Allophone, Group, GroupKind, PhonemeInventory, RootPhoneme};
use elang_core::model::when::{Condition, Predicate, WhenCondition};

pub mod key;

/// Serialize a validated ELD inventory into canonical KDL 2.0 bytes.
///
/// The result always ends with exactly one LF, contains no trailing
/// whitespace, no blank lines and no comments, and is byte-identical for
/// two inventories that differ only in declaration order.
#[must_use]
pub fn to_string(inventory: &PhonemeInventory) -> String {
    let mut out = String::with_capacity(1024);
    write_inventory(&mut out, inventory);
    out
}

/// Serialize a validated ELD inventory into canonical KDL 2.0 bytes.
///
/// Equivalent to `to_string(inventory).into_bytes()` but avoids an
/// intermediate `String` reallocation for callers writing straight into an
/// output buffer.
#[must_use]
pub fn to_bytes(inventory: &PhonemeInventory) -> Vec<u8> {
    to_string(inventory).into_bytes()
}

// ---------------------------------------------------------------------------
// Document structure
// ---------------------------------------------------------------------------

fn write_inventory(out: &mut String, inventory: &PhonemeInventory) {
    push_line(out, 0, "phonemes {");
    for group in canonical_groups(inventory) {
        write_group(out, group);
    }
    push_line(out, 0, "}");
}

fn write_group(out: &mut String, group: &Group) {
    push_line(out, 1, &format!("{} {{", group.kind().name()));
    for root in canonical_roots(group) {
        write_root(out, root);
    }
    push_line(out, 1, "}");
}

fn write_root(out: &mut String, root: &RootPhoneme) {
    push_line(
        out,
        2,
        &format!("phoneme {} {{", quote(root.ipa().as_str())),
    );
    // Child order: `roman`, `when` (if any), then the allophones (T1105).
    push_line(out, 3, &format!("roman {}", quote(root.roman().as_str())));
    if let Some(when) = root.when() {
        write_when(out, 3, when);
    }
    for allophone in canonical_allophones(root) {
        write_allophone(out, allophone);
    }
    push_line(out, 2, "}");
}

fn write_allophone(out: &mut String, allophone: &Allophone) {
    // A fieldless allophone is a bare leaf — never `allophone "x" {}`
    // (T1107, C-10, ADR 0009 E-4).
    if allophone.is_bare() {
        push_line(
            out,
            3,
            &format!("allophone {}", quote(allophone.ipa().as_str())),
        );
        return;
    }
    push_line(
        out,
        3,
        &format!("allophone {} {{", quote(allophone.ipa().as_str())),
    );
    // Child order: explicit `roman` (if any), then `when` (if any) (T1106).
    // An inherited romanization is not emitted — inheritance is implicit.
    if let Some(roman) = allophone.roman() {
        push_line(out, 4, &format!("roman {}", quote(roman.as_str())));
    }
    if let Some(when) = allophone.when() {
        write_when(out, 4, when);
    }
    push_line(out, 3, "}");
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

fn write_when(out: &mut String, depth: usize, when: &WhenCondition) {
    push_line(out, depth, "when {");
    for child in key::sorted_children(when.children()) {
        write_condition(out, depth.saturating_add(1), child);
    }
    push_line(out, depth, "}");
}

fn write_condition(out: &mut String, depth: usize, condition: &Condition) {
    match condition {
        Condition::Predicate(p) => push_line(
            out,
            depth,
            &format!("{} {}", predicate_name(p), quote(predicate_value(p))),
        ),
        Condition::Group(children) => {
            push_line(out, depth, "group {");
            for child in key::sorted_children(children) {
                write_condition(out, depth.saturating_add(1), child);
            }
            push_line(out, depth, "}");
        }
        Condition::Or(left, right) => {
            push_line(out, depth, "or {");
            for operand in key::sorted_operands(left.as_ref(), right.as_ref()) {
                write_condition(out, depth.saturating_add(1), operand);
            }
            push_line(out, depth, "}");
        }
    }
}

fn predicate_name(predicate: &Predicate) -> &'static str {
    match predicate {
        Predicate::WordPosition(_) => "word-position",
        Predicate::SyllablePosition(_) => "syllable-position",
        Predicate::PreviousPhoneme(_) => "previous-phoneme",
        Predicate::NextPhoneme(_) => "next-phoneme",
    }
}

fn predicate_value(predicate: &Predicate) -> &str {
    match predicate {
        Predicate::WordPosition(v) => key::word_position_value(*v),
        Predicate::SyllablePosition(v) => key::syllable_position_value(*v),
        // The **authored** reference, exactly as written in the source
        // (ADR 0009 E-2) — never the owning root's identifier.
        Predicate::PreviousPhoneme(ipa) => ipa.as_str(),
        Predicate::NextPhoneme(ipa) => ipa.as_str(),
    }
}

// ---------------------------------------------------------------------------
// Canonical ordering
// ---------------------------------------------------------------------------

/// The groups in canonical order: `consonants`, `vowels`, then custom groups
/// in ASCII bytewise name order (T1102).
fn canonical_groups(inventory: &PhonemeInventory) -> Vec<&Group> {
    let mut groups: Vec<&Group> = inventory.groups().iter().collect();
    groups.sort_by(|a, b| {
        group_rank(a)
            .cmp(&group_rank(b))
            .then_with(|| a.kind().name().as_bytes().cmp(b.kind().name().as_bytes()))
    });
    groups
}

fn group_rank(group: &Group) -> u8 {
    match group.kind() {
        GroupKind::Consonants => 0,
        GroupKind::Vowels => 1,
        GroupKind::Custom(_) => 2,
    }
}

/// The roots of a group in canonical order: by authored NFC IPA UTF-8 bytes
/// (T1103, ADR 0009 E-1).
fn canonical_roots(group: &Group) -> Vec<&RootPhoneme> {
    let mut roots: Vec<&RootPhoneme> = group.roots().iter().collect();
    roots.sort_by(|a, b| a.ipa().as_str().as_bytes().cmp(b.ipa().as_str().as_bytes()));
    roots
}

/// The allophones of a root in canonical order: by authored NFC IPA UTF-8
/// bytes (T1104, ADR 0009 E-1).
fn canonical_allophones(root: &RootPhoneme) -> Vec<&Allophone> {
    let mut allophones: Vec<&Allophone> = root.allophones().iter().collect();
    allophones.sort_by(|a, b| a.ipa().as_str().as_bytes().cmp(b.ipa().as_str().as_bytes()));
    allophones
}

// ---------------------------------------------------------------------------
// Formatting primitives
// ---------------------------------------------------------------------------

/// Append one 4-space-per-level indented line followed by a single LF.
///
/// `depth` is the nesting level: `0` is top level, `1` is a block child.
fn push_line(out: &mut String, depth: usize, text: &str) {
    // 4 spaces per level. `depth` is a small compile-time-controlled constant
    // at every call site, so a saturating multiply keeps the lint policy
    // (`arithmetic_side_effects`) satisfied without an `expect`.
    let indent = 4usize.saturating_mul(depth);
    for _ in 0..indent {
        out.push(' ');
    }
    out.push_str(text);
    out.push('\n');
}

/// Quote a value as a canonical KDL 2.0 double-quoted string.
///
/// Per ADR 0009 E-3 the writer escapes **only** the double quote (`\"`) and
/// the backslash (`\\`) and emits no other escape sequence — no `\n`, `\t`,
/// `\r`, `\/`, `\b`, `\f`, nor any `\u{…}` form. That is exhaustive for valid
/// input: every ELD string value is validated NFC and free of controls
/// (`Cc`), `Cf`, `Co`, `Cn` and noncharacters, so no character that would
/// need escaping can appear. Every other permitted Unicode scalar value is
/// emitted directly as UTF-8 and is never silently normalized (T1116, T1117).
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "canonical-writer tests assert exact golden bytes; a panic signals a canonicalization regression"
)]
mod tests {
    use super::*;
    use elang_core::ids::{IpaIdentifierText, Romanization};
    use elang_core::model::when::{SyllablePosition, WordPosition};

    fn ipa(text: &str) -> IpaIdentifierText {
        IpaIdentifierText::from_validated(text.to_string())
    }

    fn roman(text: &str) -> Romanization {
        Romanization::new(text).expect("valid romanization")
    }

    fn condition_word_position(value: WordPosition) -> Condition {
        Condition::predicate(Predicate::WordPosition(value))
    }

    fn condition_previous(text: &str) -> Condition {
        Condition::predicate(Predicate::PreviousPhoneme(ipa(text)))
    }

    #[test]
    fn quote_escapes_only_quote_and_backslash() {
        assert_eq!(quote("plain"), "\"plain\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("a\\b"), "\"a\\\\b\"");
        // No other escape form is emitted.
        assert_eq!(quote("a\nb"), "\"a\nb\"");
        assert_eq!(quote("a\tb"), "\"a\tb\"");
    }

    #[test]
    fn quote_preserves_unicode_directly() {
        assert_eq!(quote("tʰ"), "\"tʰ\"");
        assert_eq!(quote("t\u{035C}"), "\"t\u{035C}\"");
    }

    #[test]
    fn push_line_indents_by_four_spaces_per_level() {
        let mut out = String::new();
        push_line(&mut out, 0, "a");
        push_line(&mut out, 1, "b");
        push_line(&mut out, 2, "c");
        assert_eq!(out, "a\n    b\n        c\n");
    }

    #[test]
    fn group_rank_orders_consonants_then_vowels_then_custom() {
        use elang_core::model::eld::Group;
        use elang_core::model::eld::RootPhoneme;
        let root = RootPhoneme::new(ipa("t"), roman("t"), None, vec![]).unwrap();
        let cons = Group::new(GroupKind::Consonants, vec![root.clone()]).unwrap();
        let vow = Group::new(GroupKind::Vowels, vec![root.clone()]).unwrap();
        let custom = Group::new(
            GroupKind::Custom(
                elang_core::ids::GroupName::new_custom("diphthongs").expect("valid name"),
            ),
            vec![root],
        )
        .unwrap();
        assert!(group_rank(&cons) < group_rank(&vow));
        assert!(group_rank(&vow) < group_rank(&custom));
    }

    #[test]
    fn predicate_names_and_values_match_the_condition_language() {
        assert_eq!(
            predicate_name(&Predicate::WordPosition(WordPosition::Initial)),
            "word-position"
        );
        assert_eq!(
            predicate_value(&Predicate::WordPosition(WordPosition::Initial)),
            "initial"
        );
        assert_eq!(
            predicate_name(&Predicate::SyllablePosition(SyllablePosition::Coda)),
            "syllable-position"
        );
        assert_eq!(
            predicate_value(&Predicate::SyllablePosition(SyllablePosition::Coda)),
            "coda"
        );
        assert_eq!(
            predicate_name(&Predicate::PreviousPhoneme(ipa("s"))),
            "previous-phoneme"
        );
        assert_eq!(predicate_value(&Predicate::PreviousPhoneme(ipa("s"))), "s");
        assert_eq!(
            predicate_name(&Predicate::NextPhoneme(ipa("s"))),
            "next-phoneme"
        );
        assert_eq!(predicate_value(&Predicate::NextPhoneme(ipa("s"))), "s");
    }

    #[test]
    fn condition_helpers_are_exercised() {
        // The helpers build the two condition shapes the writer emits most.
        let wp = condition_word_position(WordPosition::Final);
        let prev = condition_previous("s");
        assert!(matches!(wp, Condition::Predicate(_)));
        assert!(matches!(prev, Condition::Predicate(_)));
    }
}
