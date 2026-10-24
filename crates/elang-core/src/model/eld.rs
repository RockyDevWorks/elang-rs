//! Immutable validated ELD phoneme-inventory model (REQ-CORE-MODEL-003,
//! REQ-ELD-*).
//!
//! Models the phoneme inventory from steering §6: a single `phonemes` root
//! holding groups (`consonants`, `vowels`, and up to 14 custom groups), each
//! group holding root phonemes, each root holding a romanization, an optional
//! `when`, and up to 16 allophones. Instances hold **already-validated** data:
//! the parser and semantic validator (Phases 9–11, `elang-eld`) build them
//! after enforcing the full grammar, the IPA allowlist, global IPA/romanization
//! uniqueness and reference resolution.
//!
//! The constructors here enforce the intrinsic per-node cardinality bounds that
//! a node can check on its own — a group is nonempty, a root has ≤ 16
//! allophones, the inventory has both required groups, ≤ 16 groups total and
//! ≤ 256 roots. Cross-node semantics (uniqueness, reference resolution) stay in
//! `elang-eld`; this model does not re-derive them.
//!
//! Immutability contract: fields are private, each type comes only from a
//! constructor, and there are no setters.

use crate::error::{Category, Error};
use crate::ids::{GroupName, IpaIdentifierText, Romanization};
use crate::model::when::WhenCondition;

/// Maximum direct allophones per root phoneme (steering §6.2).
const MAX_ALLOPHONES_PER_ROOT: usize = 16;
/// Maximum groups in an inventory, including `consonants`/`vowels` (steering §6.1).
const MAX_GROUPS: usize = 16;
/// Maximum root phonemes across all groups (steering §6.2).
const MAX_ROOT_PHONEMES: usize = 256;

/// A validated allophone of a root phoneme (steering §6.3).
///
/// A bare allophone (`allophone "ɾ"`) has no `roman` and no `when`; an explicit
/// block carries an optional explicit `roman` and/or `when`. Romanization
/// inheritance (an omitted `roman` taking the parent's value) is resolved by
/// the semantic layer, not stored here: `roman() == None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allophone {
    ipa: IpaIdentifierText,
    roman: Option<Romanization>,
    when: Option<WhenCondition>,
}

impl Allophone {
    /// Construct a validated allophone.
    #[must_use]
    pub fn new(
        ipa: IpaIdentifierText,
        roman: Option<Romanization>,
        when: Option<WhenCondition>,
    ) -> Self {
        Self { ipa, roman, when }
    }

    /// The allophone's IPA identifier.
    #[must_use]
    pub fn ipa(&self) -> &IpaIdentifierText {
        &self.ipa
    }

    /// The explicit romanization, or `None` to inherit the parent's value.
    #[must_use]
    pub fn roman(&self) -> Option<&Romanization> {
        self.roman.as_ref()
    }

    /// The optional `when` condition (independent of the parent root's).
    #[must_use]
    pub fn when(&self) -> Option<&WhenCondition> {
        self.when.as_ref()
    }

    /// Whether this is a bare allophone (no fields): the canonical writer emits
    /// it without a block (steering §6.3 / §9).
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.roman.is_none() && self.when.is_none()
    }
}

/// A validated root phoneme (steering §6.2).
///
/// A root has exactly one romanization, an optional `when`, and 0–16 direct
/// allophones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootPhoneme {
    ipa: IpaIdentifierText,
    roman: Romanization,
    when: Option<WhenCondition>,
    allophones: Vec<Allophone>,
}

impl RootPhoneme {
    /// Construct a validated root phoneme, enforcing the ≤ 16 direct-allophone
    /// bound (steering §6.2 / REQ-ELD-008).
    pub fn new(
        ipa: IpaIdentifierText,
        roman: Romanization,
        when: Option<WhenCondition>,
        allophones: Vec<Allophone>,
    ) -> Result<Self, Error> {
        if allophones.len() > MAX_ALLOPHONES_PER_ROOT {
            return Err(err(
                "ELD_TOO_MANY_ALLOPHONES",
                "root has more than 16 allophones",
            ));
        }
        Ok(Self {
            ipa,
            roman,
            when,
            allophones,
        })
    }

    /// The root's IPA identifier.
    #[must_use]
    pub fn ipa(&self) -> &IpaIdentifierText {
        &self.ipa
    }

    /// The required romanization.
    #[must_use]
    pub fn roman(&self) -> &Romanization {
        &self.roman
    }

    /// The optional `when` condition.
    #[must_use]
    pub fn when(&self) -> Option<&WhenCondition> {
        self.when.as_ref()
    }

    /// The direct allophones (0–16).
    #[must_use]
    pub fn allophones(&self) -> &[Allophone] {
        &self.allophones
    }
}

/// The kind of a phoneme group: the two reserved groups or a custom one
/// (steering §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKind {
    /// The mandatory `consonants` group.
    Consonants,
    /// The mandatory `vowels` group.
    Vowels,
    /// A custom group with a validated name.
    Custom(GroupName),
}

impl GroupKind {
    /// The group's name as it appears in the ELD.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            GroupKind::Consonants => "consonants",
            GroupKind::Vowels => "vowels",
            GroupKind::Custom(n) => n.as_str(),
        }
    }
}

/// A validated phoneme group: a kind plus ≥ 1 root phoneme (steering §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    kind: GroupKind,
    roots: Vec<RootPhoneme>,
}

impl Group {
    /// Construct a validated group, enforcing the "nonempty" invariant
    /// (steering §6.1 / REQ-ELD-006).
    pub fn new(kind: GroupKind, roots: Vec<RootPhoneme>) -> Result<Self, Error> {
        if roots.is_empty() {
            return Err(err(
                "ELD_GROUP_EMPTY",
                "group must contain at least one root phoneme",
            ));
        }
        Ok(Self { kind, roots })
    }

    /// The group kind.
    #[must_use]
    pub fn kind(&self) -> &GroupKind {
        &self.kind
    }

    /// The group's root phonemes (≥ 1).
    #[must_use]
    pub fn roots(&self) -> &[RootPhoneme] {
        &self.roots
    }
}

/// An immutable, validated ELD phoneme inventory: the `phonemes` root
/// (steering §6).
///
/// Enforces the inventory-level cardinalities a model can check on its own:
/// exactly one `consonants` group and one `vowels` group are present, group
/// names are unique, there are ≤ 16 groups total and ≤ 256 root phonemes
/// across all groups (steering §6.1/§6.2 / REQ-ELD-003..007). Global IPA and
/// romanization uniqueness and `when` reference resolution are enforced by the
/// semantic layer before these groups are built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhonemeInventory {
    groups: Vec<Group>,
}

impl PhonemeInventory {
    /// Assemble a validated inventory from its groups.
    pub fn new(groups: Vec<Group>) -> Result<Self, Error> {
        if groups.len() > MAX_GROUPS {
            return Err(err("ELD_TOO_MANY_GROUPS", "more than 16 groups"));
        }

        let mut consonants = 0usize;
        let mut vowels = 0usize;
        let mut names: Vec<&str> = Vec::with_capacity(groups.len());
        let mut root_total = 0usize;
        for g in &groups {
            match g.kind() {
                GroupKind::Consonants => consonants = consonants.saturating_add(1),
                GroupKind::Vowels => vowels = vowels.saturating_add(1),
                GroupKind::Custom(_) => {}
            }
            let name = g.kind().name();
            if names.contains(&name) {
                return Err(err("ELD_GROUP_DUPLICATE", "duplicate group name"));
            }
            names.push(name);
            root_total = root_total.saturating_add(g.roots().len());
        }

        if consonants != 1 || vowels != 1 {
            return Err(err(
                "ELD_MISSING_REQUIRED_GROUP",
                "inventory needs exactly one consonants and one vowels group",
            ));
        }
        if root_total > MAX_ROOT_PHONEMES {
            return Err(err("ELD_TOO_MANY_ROOTS", "more than 256 root phonemes"));
        }

        Ok(Self { groups })
    }

    /// All groups in declaration order (canonical ordering is applied by the
    /// serializer, not stored here).
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Total number of root phonemes across all groups.
    #[must_use]
    pub fn root_count(&self) -> usize {
        self.groups.iter().map(|g| g.roots().len()).sum()
    }
}

fn err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::EldSchema, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::when::{Condition, Predicate, WordPosition};

    fn ipa(s: &str) -> IpaIdentifierText {
        IpaIdentifierText::from_validated(s.to_string())
    }

    fn roman(s: &str) -> Romanization {
        Romanization::new(s).unwrap()
    }

    fn root(ipa_s: &str, roman_s: &str) -> RootPhoneme {
        RootPhoneme::new(ipa(ipa_s), roman(roman_s), None, vec![]).unwrap()
    }

    #[test]
    fn allophone_bare_vs_fielded() {
        let bare = Allophone::new(ipa("ɾ"), None, None);
        assert!(bare.is_bare());
        assert!(bare.roman().is_none());

        let when = WhenCondition::new(vec![Condition::predicate(Predicate::WordPosition(
            WordPosition::Initial,
        ))])
        .unwrap();
        let fielded = Allophone::new(ipa("tʰ"), Some(roman("th")), Some(when));
        assert!(!fielded.is_bare());
        assert_eq!(fielded.roman().unwrap().as_str(), "th");
        assert!(fielded.when().is_some());
    }

    #[test]
    fn root_rejects_too_many_allophones() {
        let allos: Vec<Allophone> = (0..17)
            .map(|i| Allophone::new(ipa(&format!("x{i}")), None, None))
            .collect();
        let e = RootPhoneme::new(ipa("t"), roman("t"), None, allos).unwrap_err();
        assert_eq!(e.code(), "ELD_TOO_MANY_ALLOPHONES");

        let allos16: Vec<Allophone> = (0..16)
            .map(|i| Allophone::new(ipa(&format!("x{i}")), None, None))
            .collect();
        assert!(RootPhoneme::new(ipa("t"), roman("t"), None, allos16).is_ok());
    }

    #[test]
    fn group_rejects_empty() {
        assert_eq!(
            Group::new(GroupKind::Consonants, vec![])
                .unwrap_err()
                .code(),
            "ELD_GROUP_EMPTY"
        );
    }

    #[test]
    fn group_kind_name() {
        assert_eq!(GroupKind::Consonants.name(), "consonants");
        assert_eq!(GroupKind::Vowels.name(), "vowels");
        assert_eq!(
            GroupKind::Custom(GroupName::new_custom("diphthongs").unwrap()).name(),
            "diphthongs"
        );
    }

    fn minimal_groups() -> Vec<Group> {
        vec![
            Group::new(GroupKind::Consonants, vec![root("t", "t")]).unwrap(),
            Group::new(GroupKind::Vowels, vec![root("a", "a")]).unwrap(),
        ]
    }

    #[test]
    fn inventory_accepts_minimal_valid() {
        let inv = PhonemeInventory::new(minimal_groups()).unwrap();
        assert_eq!(inv.groups().len(), 2);
        assert_eq!(inv.root_count(), 2);
    }

    #[test]
    fn inventory_accepts_custom_group() {
        let mut groups = minimal_groups();
        groups.push(
            Group::new(
                GroupKind::Custom(GroupName::new_custom("diphthongs").unwrap()),
                vec![root("aɪ", "ai")],
            )
            .unwrap(),
        );
        let inv = PhonemeInventory::new(groups).unwrap();
        assert_eq!(inv.groups().len(), 3);
        assert_eq!(inv.root_count(), 3);
    }

    #[test]
    fn inventory_requires_both_mandatory_groups() {
        // Missing vowels.
        let only_cons = vec![Group::new(GroupKind::Consonants, vec![root("t", "t")]).unwrap()];
        assert_eq!(
            PhonemeInventory::new(only_cons).unwrap_err().code(),
            "ELD_MISSING_REQUIRED_GROUP"
        );
    }

    #[test]
    fn inventory_rejects_duplicate_consonants() {
        let dup = vec![
            Group::new(GroupKind::Consonants, vec![root("t", "t")]).unwrap(),
            Group::new(GroupKind::Consonants, vec![root("k", "k")]).unwrap(),
            Group::new(GroupKind::Vowels, vec![root("a", "a")]).unwrap(),
        ];
        // Two `consonants` groups collide on name first.
        assert_eq!(
            PhonemeInventory::new(dup).unwrap_err().code(),
            "ELD_GROUP_DUPLICATE"
        );
    }

    #[test]
    fn inventory_rejects_too_many_groups() {
        let mut groups = minimal_groups();
        for i in 0..15usize {
            groups.push(
                Group::new(
                    GroupKind::Custom(GroupName::new_custom(format!("grp{}", slug(i))).unwrap()),
                    vec![root(&format!("c{i}"), "a")],
                )
                .unwrap(),
            );
        }
        assert!(groups.len() > MAX_GROUPS);
        assert_eq!(
            PhonemeInventory::new(groups).unwrap_err().code(),
            "ELD_TOO_MANY_GROUPS"
        );
    }

    #[test]
    fn inventory_rejects_too_many_roots() {
        // 257 roots spread across the two mandatory groups.
        let cons_roots: Vec<RootPhoneme> = (0..256).map(|i| root(&format!("c{i}"), "a")).collect();
        let vow_roots = vec![root("a", "b")];
        let groups = vec![
            Group::new(GroupKind::Consonants, cons_roots).unwrap(),
            Group::new(GroupKind::Vowels, vow_roots).unwrap(),
        ];
        assert_eq!(
            PhonemeInventory::new(groups).unwrap_err().code(),
            "ELD_TOO_MANY_ROOTS"
        );
    }

    // Helper: map an index to a lowercase-letter slug so custom group names stay
    // within `[a-z]+(?:-[a-z]+)*`.
    fn slug(i: usize) -> String {
        let letters = "abcdefghijklmnopqrstuvwxyz";
        let idx = i.rem_euclid(letters.chars().count());
        letters.chars().nth(idx).unwrap_or('a').to_string()
    }

    // -----------------------------------------------------------------------
    // Accessor-coverage tests (Allophone / RootPhoneme / Group / Inventory)
    // -----------------------------------------------------------------------

    #[test]
    fn allophone_accessors_return_expected_values() {
        let a = Allophone::new(ipa("ɾ"), None, None);
        assert_eq!(a.ipa().as_str(), "ɾ");
        assert!(a.roman().is_none());
        assert!(a.when().is_none());
    }

    #[test]
    fn root_phoneme_accessors_return_expected_values() {
        let w = WhenCondition::new(vec![Condition::predicate(Predicate::WordPosition(
            WordPosition::Initial,
        ))])
        .unwrap();
        let r = RootPhoneme::new(
            ipa("t"),
            roman("t"),
            Some(w.clone()),
            vec![Allophone::new(ipa("ɾ"), None, None)],
        )
        .unwrap();
        assert_eq!(r.ipa().as_str(), "t");
        assert_eq!(r.roman().as_str(), "t");
        assert_eq!(r.when(), Some(&w));
        assert_eq!(r.allophones().len(), 1);
    }

    #[test]
    fn group_accessors_return_expected_values() {
        let g = Group::new(
            GroupKind::Consonants,
            vec![RootPhoneme::new(ipa("t"), roman("t"), None, vec![]).unwrap()],
        )
        .unwrap();
        assert_eq!(g.kind().name(), "consonants");
        assert_eq!(g.roots().len(), 1);
    }
}
