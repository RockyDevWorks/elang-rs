//! ELD structural schema validator (T0901–T0930).
//!
//! The validator runs over a [`elang_kdl::ast::KdlDocument`] produced by the
//! generic parser and enforces every constraint labelled `[C-nn]` in
//! `specs/grammar/eld-0.1.0.md` that belongs to Phase 9 (C-01..C-16).
//!
//! Constraints C-20..C-27 (the `when` condition language) are **Phase 10's**
//! contract and are deliberately **not** enforced here. Phase 9 enforces only
//! placement (at most one `when` per root/allophone, nowhere else) and
//! independence (no inheritance).
//!
//! # Error codes
//!
//! | Constraint | Code |
//! |---|---|
//! | C-01 / empty file | `ELD_EMPTY` / `ELD_TOPLEVEL_INVALID` |
//! | C-02..C-05 group rules | `ELD_GROUP_INVALID`, `ELD_GROUP_DUPLICATE` |
//! | C-06..C-08 root rules | `ELD_PHONEME_INVALID` |
//! | C-09 IPA uniqueness | `ELD_IPA_DUPLICATE` |
//! | C-10..C-12 allophone rules | `ELD_ALLOPHONE_INVALID` |
//! | C-13..C-14 roman shape | `ROMAN_INVALID_NODE`, `ROMAN_BAD_FORMAT` |
//! | C-15 explicit roman uniqueness | `ROMAN_DUPLICATE` |
//! | Unknown child | `ELD_UNKNOWN_FIELD` |
//! | IPA validity | `IPA_*` / `UNICODE_*` (propagated from Phase 8) |

use std::collections::{BTreeMap, BTreeSet};

use elang_core::error::{Category, Error};
use elang_core::ids::{GroupName, IpaIdentifierText, Romanization};
use elang_kdl::ast::{KdlChildren, KdlDocument, KdlNode};
use elang_kdl::span::SourceSpan;
use elang_kdl::value::KdlValue;
use elang_repertoire::ipa;
use elang_repertoire::repertoire::Repertoire;

use crate::error::{eld_err, eld_err_at, from_core, from_ipa, from_kdl, roman_err_at};
use crate::model::{EldAllophone, EldDocument, EldGroup, EldGroupKind, EldRootPhoneme, WhenBlock};

/// Maximum groups in an inventory, including `consonants`/`vowels` (C-03).
const MAX_GROUPS: usize = 16;
/// Maximum root phonemes across all groups (C-08).
const MAX_ROOT_PHONEMES: usize = 256;
/// Maximum direct allophones per root (C-07 / C-12).
const MAX_ALLOPHONES_PER_ROOT: usize = 16;

/// Parse `input` as an ELD document and return the validated structural model.
///
/// The repertoire is a caller-provided, pinned artifact. The Phase 8 IPA
/// validator is the sole authority for IPA validity and comparison keys.
pub fn parse(input: &[u8], repertoire: &Repertoire) -> Result<EldDocument, Error> {
    let doc = elang_kdl::parse(input).map_err(from_kdl)?;
    parse_document(&doc, repertoire)
}

/// Parse a KDL document that has **already** been tokenised by `elang-kdl`.
///
/// This split exists so the reader pipeline can run the KDL parser once with
/// shared budgets and feed the resulting `KdlDocument` into multiple schema
/// validators.
pub fn parse_document(doc: &KdlDocument, repertoire: &Repertoire) -> Result<EldDocument, Error> {
    let groups = validate_top_level(doc, repertoire)?;
    Ok(EldDocument::new(groups))
}

// --------------------------------------------------------------------------
// Top-level: C-01
// --------------------------------------------------------------------------

/// Validate the top-level `phonemes` node and return the group list.
///
/// Order of checks:
/// 1. document shape (C-01 / ELD_EMPTY),
/// 2. group shape and names (C-02..C-05),
/// 3. per-root shape (C-06, C-07),
/// 4. per-allophone shape (C-10..C-12),
/// 5. IPA validity of each identifier,
/// 6. romanization syntax (C-13, C-14),
/// 7. global uniqueness: IPA (C-09) and explicit roman (C-15),
/// 8. global cardinalities: roots (C-08) and groups (C-03).
fn validate_top_level(doc: &KdlDocument, repertoire: &Repertoire) -> Result<Vec<EldGroup>, Error> {
    let nodes = doc.nodes();

    // C-02 / ELD_EMPTY: empty / whitespace / comment-only file.
    if nodes.is_empty() {
        return Err(eld_err(
            "ELD_EMPTY",
            "ELD document is empty or contains only whitespace/comments",
        ));
    }

    // C-01: exactly one top-level node named "phonemes".
    if nodes.len() != 1 {
        let span = nodes.get(1).map_or(SourceSpan::INVALID, |n| n.span());
        return Err(eld_err_at(
            "ELD_TOPLEVEL_INVALID",
            format!(
                "expected exactly one top-level `phonemes` node, found {}",
                nodes.len()
            ),
            span,
        ));
    }
    // Safety: len == 1, so get(0) is Some.
    let phonemes = nodes.first().ok_or_else(|| {
        eld_err(
            "ELD_TOPLEVEL_INVALID",
            "internal: expected one top-level node",
        )
    })?;
    if phonemes.name() != "phonemes" {
        return Err(eld_err_at(
            "ELD_TOPLEVEL_INVALID",
            format!(
                "expected top-level `phonemes` node, found `{}`",
                phonemes.name()
            ),
            phonemes.span(),
        ));
    }

    // C-01: type annotation on phonemes.
    if phonemes.ty().is_some() {
        return Err(eld_err_at(
            "ELD_TOPLEVEL_INVALID",
            "`phonemes` must not have a type annotation",
            phonemes.span(),
        ));
    }

    // C-01: zero args, zero properties.
    let mut entry_iter = phonemes.entries().iter();
    if let Some(entry) = entry_iter.next() {
        return Err(eld_err_at(
            "ELD_TOPLEVEL_INVALID",
            "`phonemes` must not have arguments or properties",
            entry.span(),
        ));
    }

    // C-01: explicit nonempty block.
    let ph_children = require_nonempty_block(phonemes, "ELD_TOPLEVEL_INVALID", "phonemes")?;

    // --- Group validation (C-02..C-05) ---
    let group_nodes: Vec<&KdlNode> = ph_children.iter().collect();
    let group_count = group_nodes.len();

    // C-03: ≤ 16 groups total.
    if group_count > MAX_GROUPS {
        let err_span = group_nodes
            .get(MAX_GROUPS)
            .map_or(SourceSpan::INVALID, |n| n.span());
        return Err(eld_err_at(
            "ELD_GROUP_INVALID",
            format!(
                "at most {} groups allowed, found {}",
                MAX_GROUPS, group_count
            ),
            err_span,
        ));
    }

    let mut groups: Vec<EldGroup> = Vec::with_capacity(group_count);
    let mut seen_group_names: BTreeSet<String> = BTreeSet::new();
    let mut has_consonants = false;
    let mut has_vowels = false;

    // Collect all IPA identifiers and explicit romans for global uniqueness.
    let mut ipa_keys: BTreeMap<String, String> = BTreeMap::new();
    let mut roman_folds: BTreeMap<String, String> = BTreeMap::new();

    // We need to track the global root count as we go (C-08).
    let mut global_root_count: usize = 0;

    for g_node in &group_nodes {
        // C-05: type annotation on group.
        if g_node.ty().is_some() {
            return Err(eld_err_at(
                "ELD_GROUP_INVALID",
                format!("group `{}` must not have a type annotation", g_node.name()),
                g_node.span(),
            ));
        }

        // C-05: zero args, zero properties on group.
        reject_entries(g_node, "ELD_GROUP_INVALID")?;

        // C-05: mandatory child block (nonempty — C-02/C-03).
        let g_children = require_nonempty_block(g_node, "ELD_GROUP_INVALID", g_node.name())?;

        let g_name = g_node.name().to_string();

        // C-04/C-05: only direct `phoneme` children.
        if let Some(bad) = phoneme_first(b"phoneme", g_children) {
            return Err(eld_err_at(
                "ELD_UNKNOWN_FIELD",
                format!(
                    "group `{}` may only contain `phoneme` children, found `{}`",
                    g_name,
                    bad.name()
                ),
                bad.span(),
            ));
        }

        let roots: Vec<&KdlNode> = g_children
            .iter()
            .filter(|c| c.name() == "phoneme")
            .collect();

        // C-02/C-03: group must be nonempty.
        if roots.is_empty() {
            return Err(eld_err_at(
                "ELD_GROUP_INVALID",
                format!("group `{}` must have at least one `phoneme`", g_name),
                g_node.span(),
            ));
        }

        // Determine group kind.
        let kind = match g_name.as_str() {
            "consonants" => {
                if has_consonants {
                    return Err(eld_err_at(
                        "ELD_GROUP_INVALID",
                        "duplicate `consonants` group",
                        g_node.span(),
                    ));
                }
                has_consonants = true;
                EldGroupKind::Consonants
            }
            "vowels" => {
                if has_vowels {
                    return Err(eld_err_at(
                        "ELD_GROUP_INVALID",
                        "duplicate `vowels` group",
                        g_node.span(),
                    ));
                }
                has_vowels = true;
                EldGroupKind::Vowels
            }
            custom => {
                if has_consonants && custom == "consonants" {
                    return Err(eld_err_at(
                        "ELD_GROUP_DUPLICATE",
                        "reserved name `consonants` reused as custom group",
                        g_node.span(),
                    ));
                }
                if has_vowels && custom == "vowels" {
                    return Err(eld_err_at(
                        "ELD_GROUP_DUPLICATE",
                        "reserved name `vowels` reused as custom group",
                        g_node.span(),
                    ));
                }
                if seen_group_names.contains(custom) {
                    return Err(eld_err_at(
                        "ELD_GROUP_DUPLICATE",
                        format!("duplicate group name `{}`", custom),
                        g_node.span(),
                    ));
                }
                seen_group_names.insert(custom.to_string());
                let gn = GroupName::new_custom(custom).map_err(|e| from_core(e, g_node.span()))?;
                EldGroupKind::Custom(gn)
            }
        };

        // Process each root phoneme in this group.
        let mut group_roots: Vec<EldRootPhoneme> = Vec::with_capacity(roots.len());

        for r_node in &roots {
            // C-06: type annotation on phoneme.
            if r_node.ty().is_some() {
                return Err(eld_err_at(
                    "ELD_PHONEME_INVALID",
                    "`phoneme` must not have a type annotation",
                    r_node.span(),
                ));
            }

            // C-06: exactly one argument, must be a string.
            let ipa_arg = require_single_string_arg(r_node, "ELD_PHONEME_INVALID")?;
            let ipa_text = ipa_arg.as_str().ok_or_else(|| {
                eld_err_at(
                    "ELD_PHONEME_INVALID",
                    "`phoneme` argument must be a string",
                    ipa_arg.span(),
                )
            })?;

            // C-06: zero properties.
            reject_props(r_node, "ELD_PHONEME_INVALID")?;

            // C-06: mandatory nonempty block.
            let r_children = require_nonempty_block(r_node, "ELD_PHONEME_INVALID", "phoneme")?;

            // C-07: scan children — exactly one `roman`, zero-or-one `when`, 0..16 `allophone`.
            let mut roman_nodes: Vec<&KdlNode> = Vec::new();
            let mut when_nodes: Vec<&KdlNode> = Vec::new();
            let mut allophone_nodes: Vec<&KdlNode> = Vec::new();

            for child in r_children.iter() {
                if child.ty().is_some() {
                    return Err(eld_err_at(
                        "ELD_PHONEME_INVALID",
                        format!(
                            "`phoneme` child `{}` must not have a type annotation",
                            child.name()
                        ),
                        child.span(),
                    ));
                }
                match child.name() {
                    "roman" => {
                        roman_nodes.push(child);
                    }
                    "when" => {
                        when_nodes.push(child);
                    }
                    "allophone" => {
                        allophone_nodes.push(child);
                        if allophone_nodes.len() > MAX_ALLOPHONES_PER_ROOT {
                            return Err(eld_err_at(
                                "ELD_ALLOPHONE_INVALID",
                                "root phoneme cannot have more than 16 allophones",
                                child.span(),
                            ));
                        }
                    }
                    other => {
                        return Err(eld_err_at(
                            "ELD_UNKNOWN_FIELD",
                            format!("unknown root child `{}`", other),
                            child.span(),
                        ));
                    }
                }
            }

            // C-07: exactly one `roman`.
            if roman_nodes.is_empty() {
                return Err(eld_err_at(
                    "ELD_PHONEME_INVALID",
                    "`phoneme` must have exactly one `roman` child",
                    r_node.span(),
                ));
            }
            if roman_nodes.len() > 1 {
                let span = roman_nodes.get(1).map_or(SourceSpan::INVALID, |n| n.span());
                return Err(eld_err_at(
                    "ELD_PHONEME_INVALID",
                    "`phoneme` must have exactly one `roman` child (found at least 2)",
                    span,
                ));
            }

            // C-07: at most one `when`.
            if when_nodes.len() > 1 {
                let span = when_nodes.get(1).map_or(SourceSpan::INVALID, |n| n.span());
                return Err(eld_err_at(
                    "ELD_PHONEME_INVALID",
                    "`phoneme` must have at most one `when` child",
                    span,
                ));
            }

            // --- IPA validation (Phase 8) ---
            let ipa_id = validate_ipa(ipa_text, r_node.span(), repertoire)?;
            let ipa_key = ipa_id.comparison_key();
            let ipa_stored = IpaIdentifierText::from_validated(ipa_id.original.clone());

            // --- Romanization validation (C-13, C-14) ---
            // Safety: we checked roman_nodes is non-empty and <= 1.
            let roman_node = roman_nodes
                .first()
                .ok_or_else(|| eld_err("ELD_PHONEME_INVALID", "internal: roman node expected"))?;
            let roman = validate_roman_node(roman_node)?;

            // --- When block (carried verbatim for Phase 10) ---
            let when = when_nodes
                .first()
                .map(|w_node| WhenBlock::new((*w_node).clone()));

            // --- Process allophones ---
            let mut allos: Vec<(EldAllophone, String)> = Vec::with_capacity(allophone_nodes.len());
            for a_node in &allophone_nodes {
                let allo = validate_allophone(a_node, &roman, repertoire)?;
                allos.push(allo);
            }

            // --- Global uniqueness checks (C-09, C-15) ---
            // Check IPA uniqueness for the root.
            if let Some(_existing_name) = ipa_keys.get(&ipa_key) {
                return Err(eld_err_at(
                    "ELD_IPA_DUPLICATE",
                    format!("duplicate IPA identifier `{}`", ipa_text),
                    r_node.span(),
                ));
            }
            ipa_keys.insert(ipa_key.clone(), ipa_text.to_string());

            // Check IPA uniqueness for allophones.
            for ((allo, allo_ipa_key), a_node) in allos.iter().zip(&allophone_nodes) {
                if let Some(existing_name) = ipa_keys.get(allo_ipa_key) {
                    return Err(eld_err_at(
                        "ELD_IPA_DUPLICATE",
                        format!(
                            "duplicate IPA identifier `{}` (already declared as `{}`)",
                            allo.ipa().as_str(),
                            existing_name
                        ),
                        a_node.span(),
                    ));
                }
                ipa_keys.insert(allo_ipa_key.clone(), allo.ipa().as_str().to_string());
            }

            // Check explicit roman uniqueness (C-15).
            let roman_fold = roman.fold_key();
            if let Some(existing_rval) = roman_folds.get(&roman_fold) {
                return Err(roman_err_at(
                    "ROMAN_DUPLICATE",
                    format!(
                        "duplicate romanization `{}` (case-insensitive collision with `{}`)",
                        roman.as_str(),
                        existing_rval
                    ),
                    roman_node.span(),
                ));
            }
            roman_folds.insert(roman_fold.clone(), roman.as_str().to_string());

            // Check explicit roman uniqueness for allophones too.
            for ((allo, _), a_node) in allos.iter().zip(&allophone_nodes) {
                if let Some(explicit_r) = allo.explicit_roman() {
                    let er_fold = explicit_r.fold_key();
                    if let Some(existing_val) = roman_folds.get(&er_fold) {
                        return Err(roman_err_at(
                            "ROMAN_DUPLICATE",
                            format!(
                                "duplicate romanization `{}` (case-insensitive collision with `{}`)",
                                explicit_r.as_str(),
                                existing_val
                            ),
                            a_node.span(),
                        ));
                    }
                    roman_folds.insert(er_fold, explicit_r.as_str().to_string());
                }
            }

            // C-08: global root count (check after validation, before pushing).
            global_root_count = global_root_count.saturating_add(1);
            if global_root_count > MAX_ROOT_PHONEMES {
                return Err(eld_err_at(
                    "ELD_PHONEME_INVALID",
                    format!(
                        "too many root phonemes: at most {} allowed",
                        MAX_ROOT_PHONEMES
                    ),
                    r_node.span(),
                ));
            }

            group_roots.push(EldRootPhoneme::new(
                ipa_stored,
                roman,
                when,
                allos.into_iter().map(|(allo, _)| allo).collect(),
            ));
        }

        groups.push(EldGroup::new(kind, group_roots));
    }

    // C-02: must have both required groups.
    if !has_consonants {
        return Err(eld_err(
            "ELD_GROUP_INVALID",
            "missing required group `consonants`",
        ));
    }
    if !has_vowels {
        return Err(eld_err(
            "ELD_GROUP_INVALID",
            "missing required group `vowels`",
        ));
    }

    Ok(groups)
}

// --------------------------------------------------------------------------
// Allophone validation (C-10..C-12, C-16)
// --------------------------------------------------------------------------

fn validate_allophone(
    node: &KdlNode,
    parent_roman: &Romanization,
    repertoire: &Repertoire,
) -> Result<(EldAllophone, String), Error> {
    // C-10: type annotation on allophone.
    if node.ty().is_some() {
        return Err(eld_err_at(
            "ELD_ALLOPHONE_INVALID",
            "`allophone` must not have a type annotation",
            node.span(),
        ));
    }

    // C-10: exactly one argument, must be a string.
    let ipa_arg = require_single_string_arg(node, "ELD_ALLOPHONE_INVALID")?;
    let ipa_text = ipa_arg.as_str().ok_or_else(|| {
        eld_err_at(
            "ELD_ALLOPHONE_INVALID",
            "`allophone` argument must be a string",
            ipa_arg.span(),
        )
    })?;

    // C-10: zero properties.
    reject_props(node, "ELD_ALLOPHONE_INVALID")?;

    // C-10: bare leaf allowed; explicit empty block invalid.
    if let Some(ch) = node.children() {
        if ch.is_empty() {
            return Err(eld_err_at(
                "ELD_ALLOPHONE_INVALID",
                "explicit empty `{}` block on allophone is not allowed; \
                 use a bare leaf `allophone \"x\"`",
                node.span(),
            ));
        }
    }

    // IPA validation.
    let ipa_id = validate_ipa(ipa_text, node.span(), repertoire)?;
    let ipa_key = ipa_id.comparison_key();
    let ipa_stored = IpaIdentifierText::from_validated(ipa_id.original.clone());

    // Scan children for `roman` and `when`.
    let mut roman_nodes: Vec<&KdlNode> = Vec::new();
    let mut when_nodes: Vec<&KdlNode> = Vec::new();

    if let Some(ch) = node.children() {
        for child in ch.iter() {
            if child.ty().is_some() {
                return Err(eld_err_at(
                    "ELD_ALLOPHONE_INVALID",
                    format!(
                        "`allophone` child `{}` must not have a type annotation",
                        child.name()
                    ),
                    child.span(),
                ));
            }
            match child.name() {
                "roman" => {
                    roman_nodes.push(child);
                    if roman_nodes.len() > 1 {
                        return Err(eld_err_at(
                            "ELD_ALLOPHONE_INVALID",
                            "allophone cannot have more than one `roman`",
                            child.span(),
                        ));
                    }
                }
                "when" => {
                    when_nodes.push(child);
                    if when_nodes.len() > 1 {
                        return Err(eld_err_at(
                            "ELD_ALLOPHONE_INVALID",
                            "allophone cannot have more than one `when`",
                            child.span(),
                        ));
                    }
                }
                other => {
                    return Err(eld_err_at(
                        "ELD_UNKNOWN_FIELD",
                        format!("unknown allophone child `{}`", other),
                        child.span(),
                    ));
                }
            }
        }
    }

    // C-16: explicit roman or inherit from parent.
    let explicit_roman = roman_nodes
        .first()
        .map(|rn| validate_roman_node(rn))
        .transpose()?;

    // When block (verbatim for Phase 10).
    let when = when_nodes.first().map(|wn| WhenBlock::new((*wn).clone()));

    Ok((
        EldAllophone::new(ipa_stored, explicit_roman, parent_roman, when),
        ipa_key,
    ))
}

// --------------------------------------------------------------------------
// Romanization node validation (C-13, C-14)
// --------------------------------------------------------------------------

fn validate_roman_node(node: &KdlNode) -> Result<Romanization, Error> {
    if node.ty().is_some() {
        return Err(roman_err_at(
            "ROMAN_INVALID_NODE",
            "`roman` must not have a type annotation",
            node.span(),
        ));
    }

    // C-13: exactly one string arg, no props, no block.
    let value = require_single_string_arg(node, "ROMAN_INVALID_NODE")?;
    let s = value.as_str().ok_or_else(|| {
        roman_err_at(
            "ROMAN_INVALID_NODE",
            "`roman` argument must be a string",
            value.span(),
        )
    })?;

    if node.props().next().is_some() {
        return Err(roman_err_at(
            "ROMAN_INVALID_NODE",
            "node `roman` must not have properties",
            node.span(),
        ));
    }

    if node.children().is_some() {
        return Err(roman_err_at(
            "ROMAN_INVALID_NODE",
            "`roman` must not have a child block",
            node.span(),
        ));
    }

    // C-14: value matches [A-Za-z]{1,8}; elang-core enforces this.
    Romanization::new(s).map_err(|e| from_core(e, node.span()))
}

// --------------------------------------------------------------------------
// IPA validation (delegates to Phase 8)
// --------------------------------------------------------------------------

fn validate_ipa(
    text: &str,
    span: SourceSpan,
    repertoire: &Repertoire,
) -> Result<ipa::IpaIdentifier, Error> {
    ipa::validate(text, repertoire).map_err(|e| from_ipa(e, span))
}

// --------------------------------------------------------------------------
// Shared node-shape primitives
// --------------------------------------------------------------------------

/// Reject any properties on `node`.
fn reject_props(node: &KdlNode, code: &'static str) -> Result<(), Error> {
    if node.props().next().is_some() {
        return Err(eld_err_at(
            code,
            format!("node `{}` must not have properties", node.name()),
            node.span(),
        ));
    }
    Ok(())
}

/// Reject all entries (arguments + properties) — used for group nodes.
fn reject_entries(node: &KdlNode, code: &'static str) -> Result<(), Error> {
    let mut entries = node.entries().iter();
    if let Some(entry) = entries.next() {
        return Err(eld_err_at(
            code,
            format!(
                "node `{}` must not have arguments or properties",
                node.name()
            ),
            entry.span(),
        ));
    }
    Ok(())
}

/// Require `node` to have exactly one argument (used to pluck the IPA string
/// or romanization).
fn require_single_string_arg<'a>(
    node: &'a KdlNode,
    code: &'static str,
) -> Result<&'a KdlValue, Error> {
    let mut args = node.args();
    let first = args.next().ok_or_else(|| {
        let mut e = eld_err_at(
            code,
            format!("node `{}` must have exactly one argument", node.name()),
            node.span(),
        );
        if node.name() == "roman" {
            e.diagnostic.category = Category::Roman;
        }
        e
    })?;
    if args.next().is_some() {
        let mut e = eld_err_at(
            code,
            format!("node `{}` must have exactly one argument", node.name()),
            node.span(),
        );
        return Err(if node.name() == "roman" {
            e.diagnostic.category = Category::Roman;
            e
        } else {
            e
        });
    }
    Ok(first)
}

/// Require a child block on `node`, returning it; also checks it is nonempty.
fn require_nonempty_block<'a>(
    node: &'a KdlNode,
    code: &'static str,
    name: &str,
) -> Result<&'a KdlChildren, Error> {
    let children = node.children().ok_or_else(|| {
        eld_err_at(
            code,
            format!("`{name}` must have a child block"),
            node.span(),
        )
    })?;
    if children.is_empty() {
        return Err(eld_err_at(
            code,
            format!("`{name}` block must not be empty"),
            node.span(),
        ));
    }
    Ok(children)
}

/// Find the first child whose name is NOT the expected name, or `None` if all
/// children match.
fn phoneme_first<'a>(expected_name: &[u8], children: &'a KdlChildren) -> Option<&'a KdlNode> {
    children
        .iter()
        .find(|c| c.name().as_bytes() != expected_name)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "tests assert exact diagnostic codes/categories; a panic signals a regression"
)]
mod tests {
    use super::*;
    use elang_core::error::Category;
    use elang_repertoire::repertoire;

    fn rep() -> Repertoire {
        repertoire::load().expect("repertoire must load")
    }

    fn parse_ok(src: &str, rep: &Repertoire) -> EldDocument {
        parse(src.as_bytes(), rep).expect("parse should succeed")
    }

    fn parse_err(src: &str, rep: &Repertoire) -> Error {
        parse(src.as_bytes(), rep).expect_err("parse should fail")
    }

    // ---- Valid documents ----

    #[test]
    fn minimal_valid() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        assert_eq!(doc.groups().len(), 2);
    }

    #[test]
    fn root_when_allophone_no_when_is_unconditional() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants { phoneme \"t\" {\n        roman \"t\"\n        when { word-position \"initial\" }\n        allophone \"ɾ\"\n    } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        assert!(doc.groups()[0].roots()[0].when().is_some());
        assert!(doc.groups()[0].roots()[0].allophones()[0].when().is_none());
        assert!(doc.groups()[0].roots()[0].allophones()[0].is_bare());
    }

    #[test]
    fn bare_allophone_accepted() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants { phoneme \"t\" { roman \"t\"\n        allophone \"tʰ\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        assert!(doc.groups()[0].roots()[0].allophones()[0].is_bare());
    }

    #[test]
    fn allophone_roman_inheritance_and_explicit() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants { phoneme \"t\" {\n        roman \"t\"\n        allophone \"ɾ\"\n        allophone \"tʰ\" { roman \"th\" }\n    } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        let root = &doc.groups()[0].roots()[0];
        assert!(root.allophones()[0].explicit_roman().is_none());
        assert_eq!(root.allophones()[0].effective_roman().as_str(), "t");
        assert_eq!(
            root.allophones()[1].explicit_roman().unwrap().as_str(),
            "th"
        );
        assert_eq!(root.allophones()[1].effective_roman().as_str(), "th");
    }

    #[test]
    fn identical_when_blocks_both_accepted() {
        let r = rep();
        parse_ok(
            "phonemes {\n    consonants { phoneme \"t\" {\n        roman \"t\"\n        allophone \"ɾ\" { when { word-position \"initial\" } }\n        allophone \"ʔ\" { when { word-position \"initial\" } }\n    } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
    }

    #[test]
    fn custom_groups_accepted() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    sibilants { phoneme \"s\" { roman \"s\" } }\n    labio-velars { phoneme \"w\" { roman \"w\" } }\n}",
            &r,
        );
        assert_eq!(doc.groups().len(), 4);
    }

    #[test]
    fn tied_and_untied_are_distinct() {
        let r = rep();
        let doc = parse_ok(
            "phonemes {\n    consonants {\n        phoneme \"tʃ\" { roman \"c\" }\n        phoneme \"t͡ʃ\" { roman \"ch\" }\n    }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        assert_eq!(doc.groups()[0].roots().len(), 2);
    }

    #[test]
    fn upper_and_lower_tie_collide() {
        let r = rep();
        let e = parse_err(
            "phonemes {\n    consonants {\n        phoneme \"t͡ʃ\" { roman \"c\" }\n        phoneme \"t͜ʃ\" { roman \"ch\" }\n    }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
        assert_eq!(e.code(), "ELD_IPA_DUPLICATE");
    }

    // ---- Invalid documents ----

    #[test]
    fn empty_file() {
        let r = rep();
        assert_eq!(parse_err("", &r).code(), "ELD_EMPTY");
    }

    #[test]
    fn whitespace_only() {
        let r = rep();
        assert_eq!(parse_err("  \n  \t  \n", &r).code(), "ELD_EMPTY");
    }

    #[test]
    fn comment_only() {
        let r = rep();
        assert_eq!(parse_err("// just a comment\n", &r).code(), "ELD_EMPTY");
    }

    #[test]
    fn two_top_level_nodes() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }\nphonemes {}",
                &r
            ).code(),
            "ELD_TOPLEVEL_INVALID"
        );
    }

    #[test]
    fn top_level_not_phonemes() {
        let r = rep();
        assert_eq!(parse_err("foo {}", &r).code(), "ELD_TOPLEVEL_INVALID");
    }

    #[test]
    fn phonemes_with_arg() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes \"x\" { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_TOPLEVEL_INVALID"
        );
    }

    #[test]
    fn phonemes_empty_block() {
        let r = rep();
        assert_eq!(parse_err("phonemes {}", &r).code(), "ELD_TOPLEVEL_INVALID");
    }

    #[test]
    fn missing_vowels() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } } }",
                &r
            )
            .code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn missing_consonants() {
        let r = rep();
        assert_eq!(
            parse_err("phonemes { vowels { phoneme \"a\" { roman \"a\" } } }", &r).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn duplicate_vowels() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    vowels { phoneme \"e\" { roman \"e\" } } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn custom_group_uppercase() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    Consonants { phoneme \"b\" { roman \"b\" } } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn custom_group_underscore() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    labio_velars { phoneme \"w\" { roman \"w\" } } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn custom_group_33_chars() {
        let r = rep();
        let long_name = "a".repeat(33);
        assert_eq!(
            parse_err(
                &format!(
                    "phonemes {{ consonants {{ phoneme \"p\" {{ roman \"p\" }} }}\n    vowels {{ phoneme \"a\" {{ roman \"a\" }} }}\n    {long_name} {{ phoneme \"b\" {{ roman \"b\" }} }} }}"
                ),
                &r,
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn custom_group_32_chars_accepted() {
        let r = rep();
        let name = "a".repeat(32);
        parse_ok(
            &format!(
                "phonemes {{ consonants {{ phoneme \"p\" {{ roman \"p\" }} }}\n    vowels {{ phoneme \"a\" {{ roman \"a\" }} }}\n    {name} {{ phoneme \"b\" {{ roman \"b\" }} }} }}"
            ),
            &r,
        );
    }

    #[test]
    fn duplicate_group_name() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    clicks { phoneme \"ǀ\" { roman \"x\" } }\n    clicks { phoneme \"ǁ\" { roman \"y\" } } }",
                &r
            ).code(),
            "ELD_GROUP_DUPLICATE"
        );
    }

    #[test]
    fn custom_group_named_vowels() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    vowels { phoneme \"e\" { roman \"e\" } } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn empty_custom_group() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n    emptygrp { } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn group_with_property() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants key=\"x\" { phoneme \"p\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_GROUP_INVALID"
        );
    }

    #[test]
    fn group_with_non_phoneme_child() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" }\n    foo \"bar\" }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn seventeen_groups() {
        let r = rep();
        let mut body = String::from("consonants { phoneme \"p\" { roman \"p\" } }\n");
        body.push_str("vowels { phoneme \"a\" { roman \"a\" } }\n");
        for i in 0..15 {
            let name = format!("g{i}");
            let ipa = format!("b{i}");
            let roman = format!("x{i}");
            body.push_str(&format!(
                "{name} {{ phoneme \"{ipa}\" {{ roman \"{roman}\" }} }}\n"
            ));
        }
        let src = format!("phonemes {{ {} }}", body);
        let e = parse_err(&src, &r);
        assert_eq!(e.code(), "ELD_GROUP_INVALID");
    }

    #[test]
    fn phoneme_no_arg() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn phoneme_two_args() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" \"x\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn phoneme_non_string_arg() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme 42 { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn phoneme_with_property() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" key=\"v\" { roman \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn phoneme_empty_block() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn root_no_roman() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { allophone \"b\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn root_two_romans() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        roman \"b\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn root_two_whens() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        when { word-position \"initial\" }\n    when { word-position \"final\" } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_PHONEME_INVALID"
        );
    }

    #[test]
    fn root_seventeen_allophones() {
        let r = rep();
        let mut src = String::from("phonemes { consonants { phoneme \"p\" {\n    roman \"p\"\n");
        for i in 0..17 {
            src.push_str(&format!("    allophone \"x{i}\"\n"));
        }
        src.push_str("} }\n    vowels { phoneme \"a\" { roman \"a\" } } }");
        let e = parse_err(&src, &r);
        assert_eq!(e.code(), "ELD_ALLOPHONE_INVALID");
    }

    #[test]
    fn root_with_description() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        description \"labial\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn root_with_distribution() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        distribution \"all\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn unknown_root_child() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        frequency \"high\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn allophone_no_arg() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_two_args() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"x\" \"y\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_non_string_arg() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone 42 } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_with_property() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"x\" key=\"v\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_empty_block() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"x\" {} } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_nested_allophone() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"tʰ\" { allophone \"kʰ\" } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn allophone_two_romans() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"tʰ\" { roman \"th\"\n            roman \"dh\" } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ELD_ALLOPHONE_INVALID");
    }

    #[test]
    fn allophone_two_whens() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"tʰ\" { when { word-position \"initial\" }\n    when { word-position \"final\" } } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_ALLOPHONE_INVALID"
        );
    }

    #[test]
    fn allophone_unknown_child() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"tʰ\" { frequency \"high\" } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn roman_no_arg() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_INVALID_NODE");
        assert_eq!(e.category(), Category::Roman);
    }

    #[test]
    fn roman_two_args() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman \"p\" \"b\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_INVALID_NODE");
        assert_eq!(e.category(), Category::Roman);
    }

    #[test]
    fn roman_non_string_arg() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman 42 } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_INVALID_NODE");
        assert_eq!(e.category(), Category::Roman);
    }

    #[test]
    fn roman_with_property() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman \"p\" key=\"v\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_INVALID_NODE");
        assert_eq!(e.category(), Category::Roman);
    }

    #[test]
    fn roman_with_block() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" { } } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ROMAN_INVALID_NODE"
        );
    }

    #[test]
    fn roman_bad_format() {
        let r = rep();
        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman \"9\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_BAD_FORMAT");
        assert_eq!(e.category(), Category::Roman);

        let e = parse_err(
            "phonemes { consonants { phoneme \"p\" { roman \"\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_BAD_FORMAT");
    }

    #[test]
    fn roman_too_long() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"toolongvalue\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ROMAN_BAD_FORMAT"
        );
    }

    #[test]
    fn roman_case_insensitive_collision() {
        let r = rep();
        let e = parse_err(
            "phonemes {\n    consonants { phoneme \"t\" { roman \"th\" } }\n    vowels { phoneme \"a\" { roman \"TH\" } }\n}",
            &r,
        );
        assert_eq!(e.code(), "ROMAN_DUPLICATE");
        assert_eq!(e.category(), Category::Roman);
    }

    #[test]
    fn inherited_romans_do_not_collide() {
        let r = rep();
        parse_ok(
            "phonemes {\n    consonants { phoneme \"t\" {\n        roman \"t\"\n        allophone \"ɾ\"\n        allophone \"ʔ\"\n    } }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}",
            &r,
        );
    }

    #[test]
    fn ipa_collision_two_roots() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" }\n    phoneme \"p\" { roman \"b\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_IPA_DUPLICATE"
        );
    }

    #[test]
    fn ipa_collision_root_and_allophone() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\"\n        allophone \"p\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_IPA_DUPLICATE"
        );
    }

    #[test]
    fn ipa_not_in_repertoire() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"\u{1D00}\" { roman \"x\" } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "IPA_NOT_IN_REPERTOIRE"
        );
    }

    #[test]
    fn when_as_group_child_is_unknown_field() {
        let r = rep();
        assert_eq!(
            parse_err(
                "phonemes { consonants { phoneme \"p\" { roman \"p\" }\n    when { } }\n    vowels { phoneme \"a\" { roman \"a\" } } }",
                &r
            ).code(),
            "ELD_UNKNOWN_FIELD"
        );
    }
}
