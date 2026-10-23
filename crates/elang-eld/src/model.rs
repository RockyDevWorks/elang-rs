//! Phase 9 structural model for ELD phoneme inventories.
//!
//! This module defines the **Phase 9 handoff** model: a structurally validated
//! ELD document in which group shapes, root/allophone cardinalities,
//! romanization syntax and global uniqueness have been checked, but the `when`
//! condition language has **not** been parsed or validated.
//!
//! # Phase 10 handoff
//!
//! The [`WhenBlock`] type carries a `when` node **verbatim** (as an
//! [`elang_kdl::ast::KdlNode`]) for Phase 10 to parse into a
//! [`elang_core::model::when::WhenCondition`]. The payload is intentionally
//! unvalidated — Phase 10 owns:
//!
//! * the atomic-predicate value domains (`word-position`, `syllable-position`,
//!   `previous-phoneme`, `next-phoneme`);
//! * the AND-container semantics (`when { … }` / `group { … }`);
//! * the binary `or { … }` semantics;
//! * the depth and predicate budgets (C-27);
//! * IPA reference resolution (C-23);
//! * the evaluator.
//!
//! Phase 9 enforces only that each root/allophone has **at most one** `when`
//! (C-07, C-11), that `when` is a direct child of a root or allophone and
//! nowhere else, and that root and allophone `when` are independent (REQ-ELD-017).
//!
//! # IPA validity
//!
//! The Phase 8 IPA validator (`elang_repertoire::ipa::validate`) is the sole
//! authority for IPA identifier validity, tie-bar normalisation and comparison
//! keys. This module stores validated IPA text via
//! [`elang_core::ids::IpaIdentifierText::from_validated`].

use elang_core::ids::{GroupName, IpaIdentifierText, Romanization};
use elang_kdl::ast::KdlNode;

/// A structurally validated ELD document.
///
/// Contains the two mandatory groups (`consonants`, `vowels`) and up to 14
/// custom groups in declaration order (T0927 — order carries no priority).
#[derive(Debug, Clone, PartialEq)]
pub struct EldDocument {
    groups: Vec<EldGroup>,
}

impl EldDocument {
    /// Construct a validated document from its groups.
    pub(crate) fn new(groups: Vec<EldGroup>) -> Self {
        Self { groups }
    }

    /// All groups in declaration order.
    #[must_use]
    pub fn groups(&self) -> &[EldGroup] {
        &self.groups
    }
}

/// One phoneme group: `consonants`, `vowels`, or a custom group.
#[derive(Debug, Clone, PartialEq)]
pub struct EldGroup {
    kind: EldGroupKind,
    roots: Vec<EldRootPhoneme>,
}

impl EldGroup {
    /// Construct a group.
    pub(crate) fn new(kind: EldGroupKind, roots: Vec<EldRootPhoneme>) -> Self {
        Self { kind, roots }
    }

    /// The group kind.
    #[must_use]
    pub fn kind(&self) -> &EldGroupKind {
        &self.kind
    }

    /// The root phonemes in declaration order.
    #[must_use]
    pub fn roots(&self) -> &[EldRootPhoneme] {
        &self.roots
    }
}

/// The kind of a phoneme group.
#[derive(Debug, Clone, PartialEq)]
pub enum EldGroupKind {
    /// The mandatory `consonants` group.
    Consonants,
    /// The mandatory `vowels` group.
    Vowels,
    /// A custom group with a validated name (`[a-z]+(?:-[a-z]+)*`, 1–32 ASCII).
    Custom(GroupName),
}

impl EldGroupKind {
    /// The group's name as it appears in the ELD.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            EldGroupKind::Consonants => "consonants",
            EldGroupKind::Vowels => "vowels",
            EldGroupKind::Custom(n) => n.as_str(),
        }
    }
}

/// A root phoneme (C-06..C-09).
///
/// A root has exactly one `roman`, optionally one `when`, and 0–16 direct
/// allophones (C-07). Only these children are permitted (C-07).
#[derive(Debug, Clone, PartialEq)]
pub struct EldRootPhoneme {
    ipa: IpaIdentifierText,
    roman: Romanization,
    when: Option<WhenBlock>,
    allophones: Vec<EldAllophone>,
}

impl EldRootPhoneme {
    /// Construct a root phoneme.
    pub(crate) fn new(
        ipa: IpaIdentifierText,
        roman: Romanization,
        when: Option<WhenBlock>,
        allophones: Vec<EldAllophone>,
    ) -> Self {
        Self {
            ipa,
            roman,
            when,
            allophones,
        }
    }

    /// The root's IPA identifier.
    #[must_use]
    pub fn ipa(&self) -> &IpaIdentifierText {
        &self.ipa
    }

    /// The required romanization (always explicit — C-07).
    #[must_use]
    pub fn roman(&self) -> &Romanization {
        &self.roman
    }

    /// The optional `when` block, if present.
    #[must_use]
    pub fn when(&self) -> Option<&WhenBlock> {
        self.when.as_ref()
    }

    /// The direct allophones in declaration order (T0927).
    #[must_use]
    pub fn allophones(&self) -> &[EldAllophone] {
        &self.allophones
    }
}

/// A directly declared allophone of a root phoneme (C-10..C-12, C-16).
///
/// A bare allophone (`allophone "pʰ"` with no block) has no explicit roman and
/// no when. An allophone with fields has a nonempty block containing at most one
/// `roman` and at most one `when`.
///
/// # Romanization inheritance (C-16)
///
/// An allophone that omits `roman` inherits the parent root's exact original-case
/// value. [`effective_roman`](Self::effective_roman) resolves this at model
/// construction time; the inherited value is **not** a second explicit declaration
/// (C-15 exemption).
///
/// # `when` independence (REQ-ELD-017 / T0926)
///
/// An allophone's `when` is **never** inherited from its root. An allophone with
/// no `when` is unconditional regardless of its root's `when`.
#[derive(Debug, Clone, PartialEq)]
pub struct EldAllophone {
    ipa: IpaIdentifierText,
    explicit_roman: Option<Romanization>,
    effective_roman: Romanization,
    when: Option<WhenBlock>,
}

impl EldAllophone {
    /// Construct an allophone with inheritance resolution.
    ///
    /// `inherited_from_root` is the parent root's `Romanization`; it is used
    /// when `explicit_roman` is `None`.
    pub(crate) fn new(
        ipa: IpaIdentifierText,
        explicit_roman: Option<Romanization>,
        inherited_from_root: &Romanization,
        when: Option<WhenBlock>,
    ) -> Self {
        let effective_roman = explicit_roman
            .clone()
            .unwrap_or_else(|| inherited_from_root.clone());
        Self {
            ipa,
            explicit_roman,
            effective_roman,
            when,
        }
    }

    /// The allophone's IPA identifier.
    #[must_use]
    pub fn ipa(&self) -> &IpaIdentifierText {
        &self.ipa
    }

    /// The explicit romanization, or `None` if inherited.
    #[must_use]
    pub fn explicit_roman(&self) -> Option<&Romanization> {
        self.explicit_roman.as_ref()
    }

    /// The resolved romanization (inherited or explicit — C-16).
    #[must_use]
    pub fn effective_roman(&self) -> &Romanization {
        &self.effective_roman
    }

    /// The optional `when` block (never inherited — REQ-ELD-017).
    #[must_use]
    pub fn when(&self) -> Option<&WhenBlock> {
        self.when.as_ref()
    }

    /// Whether this allophone is bare (no explicit roman and no when).
    ///
    /// A bare allophone is emitted without braces in canonical output.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.explicit_roman.is_none() && self.when.is_none()
    }
}

/// An unparsed `when` block, carried verbatim for Phase 10.
///
/// Phase 9 enforces placement and cardinality (at most one per root/allophone,
/// nowhere else) but does **not** parse, validate, or evaluate the contents.
/// The node and its children are passed through unmodified for Phase 10.
#[derive(Debug, Clone, PartialEq)]
pub struct WhenBlock {
    node: KdlNode,
}

impl WhenBlock {
    /// Wrap a `when` KDL node.
    pub(crate) fn new(node: KdlNode) -> Self {
        Self { node }
    }

    /// The raw `when` KDL node.
    #[must_use]
    pub fn node(&self) -> &KdlNode {
        &self.node
    }

    /// The block's children (the expressions inside `{ … }`), or `&[]` when
    /// the `when` node has no block.
    #[must_use]
    pub fn children(&self) -> &[KdlNode] {
        self.node.children().map(|c| c.as_slice()).unwrap_or(&[])
    }
}
