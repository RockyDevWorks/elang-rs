//! Phase 11 handoff — build the core validated inventory model.
//!
//! [`elang_core::model::eld::PhonemeInventory`] is the *validated* ELD model
//! that the canonical writer ([`crate::canon`]) consumes. Phase 9 builds the
//! structural [`EldDocument`], Phase 10 parses the `when` blocks into the
//! typed condition AST, and this module joins the two into the core model.
//!
//! The join is deliberately thin: it copies the already-validated IPA text,
//! romanizations and group kinds across, and asks Phase 10's
//! [`parse_block`] for each `when` block's AST. No
//! validation rule is re-implemented here — the inventory constructors in
//! `elang-core` remain the authority for the inventory-level cardinalities
//! (≤ 16 groups, ≤ 256 roots, exactly one `consonants` and one `vowels`
//! group, nonempty groups), and they are the only thing that can fail.
//!
//! The romanization *inheritance* rule (C-16) is preserved by keeping the
//! allophone's **explicit** romanization in the core model: `None` means
//! "inherit the parent root's value", exactly as
//! [`elang_core::model::eld::Allophone`] documents. The canonical writer
//! therefore emits an inherited allophone without a `roman` line, which is
//! what makes canonical output a fixpoint.

use elang_core::error::Error;
use elang_core::model::eld::{Allophone, Group, GroupKind, PhonemeInventory, RootPhoneme};
use elang_repertoire::repertoire::Repertoire;

use crate::model::{EldAllophone, EldDocument, EldGroupKind, EldRootPhoneme};
use crate::when::{parse_block, InventoryRefs};

/// Build the core [`PhonemeInventory`] from a structurally validated ELD
/// document.
///
/// Every `when` block in `doc` is parsed into a validated
/// [`elang_core::model::when::WhenCondition`] using Phase 10's
/// [`parse_block`] against the document's own
/// reference map, so cross-references resolve exactly as they do during
/// validation.
///
/// `rep` is the pinned IPA repertoire. It is needed because a
/// `previous-phoneme` / `next-phoneme` reference is an IPA identifier and
/// Phase 8 owns its validity.
///
/// # Errors
///
/// Returns the first diagnostic raised while parsing a `when` block
/// (category `WHEN`, stage 12) or while assembling the inventory
/// (category `ELD_SCHEMA`).
pub fn from_document(doc: &EldDocument, rep: &Repertoire) -> Result<PhonemeInventory, Error> {
    let refs = InventoryRefs::from_document(doc);
    let mut groups = Vec::with_capacity(doc.groups().len());
    for group in doc.groups() {
        let kind = match group.kind() {
            EldGroupKind::Consonants => GroupKind::Consonants,
            EldGroupKind::Vowels => GroupKind::Vowels,
            EldGroupKind::Custom(name) => GroupKind::Custom(name.clone()),
        };
        let mut roots = Vec::with_capacity(group.roots().len());
        for root in group.roots() {
            roots.push(root_to_core(root, &refs, rep)?);
        }
        groups.push(Group::new(kind, roots)?);
    }
    PhonemeInventory::new(groups)
}

fn root_to_core(
    root: &EldRootPhoneme,
    refs: &InventoryRefs,
    rep: &Repertoire,
) -> Result<RootPhoneme, Error> {
    let when = match root.when() {
        Some(block) => Some(parse_block(block, refs, rep)?),
        None => None,
    };
    let mut allophones = Vec::with_capacity(root.allophones().len());
    for allophone in root.allophones() {
        allophones.push(allophone_to_core(allophone, refs, rep)?);
    }
    RootPhoneme::new(root.ipa().clone(), root.roman().clone(), when, allophones)
}

fn allophone_to_core(
    allophone: &EldAllophone,
    refs: &InventoryRefs,
    rep: &Repertoire,
) -> Result<Allophone, Error> {
    let when = match allophone.when() {
        Some(block) => Some(parse_block(block, refs, rep)?),
        None => None,
    };
    // `explicit_roman()` is `None` for an inherited romanization; the core
    // model uses the same `None` to mean "inherit" (C-16).
    Ok(Allophone::new(
        allophone.ipa().clone(),
        allophone.explicit_roman().cloned(),
        when,
    ))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "inventory tests assert exact model shapes; a panic signals a regression"
)]
mod tests {
    use super::*;

    fn rep() -> Repertoire {
        elang_repertoire::repertoire::load().expect("repertoire must load")
    }

    const DOC: &str = "phonemes {\n  consonants {\n    phoneme \"t\" {\n      roman \"t\"\n      allophone \"ɾ\"\n      allophone \"tʰ\" {\n        roman \"th\"\n        when {\n          word-position \"initial\"\n        }\n      }\n    }\n  }\n  vowels {\n    phoneme \"a\" {\n      roman \"a\"\n    }\n  }\n}\n";

    #[test]
    fn builds_the_core_inventory() {
        let doc = crate::parse::parse(DOC.as_bytes(), &rep()).expect("valid ELD");
        let inv = from_document(&doc, &rep()).expect("inventory builds");
        assert_eq!(inv.groups().len(), 2);
        assert_eq!(inv.root_count(), 2);
    }

    #[test]
    fn allophone_romanization_inheritance_is_preserved() {
        let doc = crate::parse::parse(DOC.as_bytes(), &rep()).expect("valid ELD");
        let inv = from_document(&doc, &rep()).expect("inventory builds");
        let consonants = &inv.groups()[0];
        let root = &consonants.roots()[0];
        // The first allophone is bare: no explicit roman, no when.
        let bare = &root.allophones()[0];
        assert!(bare.roman().is_none());
        assert!(bare.when().is_none());
        assert!(bare.is_bare());
        // The second allophone carries an explicit roman and a when.
        let fielded = &root.allophones()[1];
        assert_eq!(fielded.roman().map(|r| r.as_str()), Some("th"));
        assert!(fielded.when().is_some());
    }

    #[test]
    fn group_kinds_are_carried_across() {
        let doc = crate::parse::parse(DOC.as_bytes(), &rep()).expect("valid ELD");
        let inv = from_document(&doc, &rep()).expect("inventory builds");
        assert!(matches!(inv.groups()[0].kind(), GroupKind::Consonants));
        assert!(matches!(inv.groups()[1].kind(), GroupKind::Vowels));
    }
}
