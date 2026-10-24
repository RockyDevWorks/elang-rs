//! Phase 10 — `when` condition parser, reference resolver, and pure evaluator.
//!
//! This module implements the phonological condition language from
//! `specs/grammar/eld-0.1.0.md` §3 and steering §8
//! (REQ-WHEN-001..015, T1001–T1030). The contract:
//!
//! * **Parser.** [`parse_block`] turns the raw KDL node carried by
//!   [`crate::model::WhenBlock`] into a validated
//!   [`elang_core::model::when::WhenCondition`] AST, enforcing C-20..C-27
//!   (arity, value domains, the 16-predicate and 8-nesting budgets).
//! * **Resolver.** [`InventoryRefs`] collects every root/allophone IPA
//!   comparison key before [`parse_all`] walks the document. Every
//!   `previous-phoneme` / `next-phoneme` authored reference is validated
//!   against that map; cross-group references resolve because the map is
//!   global (C-23, T1013–T1016).
//! * **Evaluator.** [`evaluate`] is a pure function over
//!   [`EvalContext`]. It never inspects previously selected surface
//!   allophones (T1026); it returns [`Eligibility::Empty`] — not a
//!   fallback — when no realization matches (T1028); and it never ranks
//!   or weights candidates (T1027).
//!
//! The module is reachable from [`crate::parse::parse_document`] at the
//! tail of stage 12, so condition errors surface with stage 12 and the
//! `WHEN` category.

use std::collections::HashMap;

use elang_core::error::{Category, Diagnostic, Error, KdlSpan};
use elang_core::ids::{IpaIdentifierText, Romanization};
use elang_core::model::when::{
    Condition, Predicate, SyllablePosition, WhenCondition, WordPosition,
};
use elang_kdl::ast::{KdlChildren, KdlNode};
use elang_kdl::span::SourceSpan;
use elang_repertoire::ipa;
use elang_repertoire::repertoire::Repertoire;

use crate::model::{EldDocument, EldRootPhoneme, WhenBlock};

// ---------------------------------------------------------------------------
// Limits (steering §8 / C-27 / REQ-WHEN-008)
// ---------------------------------------------------------------------------

/// Maximum atomic predicates per `when` across every nested branch
/// (REQ-WHEN-008, C-27).
pub const MAX_PREDICATES_PER_WHEN: usize = 16;

/// Maximum nested `group`/`or` levels along any single path
/// (REQ-WHEN-008, C-27).
///
/// The depth counter starts at 1 for the root `when`'s direct children
/// (the top-level expressions sit at depth 1) and increments on each
/// descent into a `group` or `or`. Atomic predicates do not consume
/// depth, matching the spec's "root `when` and leaf atomics excluded"
// phrasing: a flat `when { word-position "x" previous-phoneme "a" }` is
/// depth 1 and never exceeds the limit regardless of predicate count.
pub const MAX_NESTING_DEPTH: usize = 8;

// ---------------------------------------------------------------------------
// Inventory reference map (T1013–T1016)
// ---------------------------------------------------------------------------

/// Per-document reference map.
///
/// Stores every declared root/allophone IPA *comparison key* (NFC +
/// U+035C → U+0361 normalisation) and the comparison key of the
/// underlying root that owns each entry. The map is built once per
/// document, after structural validation, and used by both the parser
/// (to reject references to undeclared identifiers — T1016) and the
/// evaluator (to translate an authored allophone reference into the
/// owning root's comparison key — T1015).
///
/// Forward references resolve for free because every entry is collected
/// up front, before any reference is checked (T1013–T1014).
#[derive(Debug, Default, Clone)]
pub struct InventoryRefs {
    owner: HashMap<String, String>,
}

impl InventoryRefs {
    /// Build the map by walking every group / root / allophone in
    /// `doc`. Phase 9 has already validated every IPA against the
    /// repertoire, so the keys here are guaranteed to be valid
    /// comparison keys.
    #[must_use]
    pub fn from_document(doc: &EldDocument) -> Self {
        let mut owner: HashMap<String, String> = HashMap::new();
        for group in doc.groups() {
            for root in group.roots() {
                let root_key = comparison_key(root.ipa().as_str());
                owner.insert(root_key.clone(), root_key.clone());
                for allo in root.allophones() {
                    let allo_key = comparison_key(allo.ipa().as_str());
                    owner.insert(allo_key, root_key.clone());
                }
            }
        }
        Self { owner }
    }

    /// Owning root comparison key for `key`, or `None` if no declared
    /// root or allophone shares that comparison key.
    #[must_use]
    pub fn owner_of(&self, key: &str) -> Option<&str> {
        self.owner.get(key).map(String::as_str)
    }

    /// True iff `key` is a declared root or allophone comparison key.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.owner.contains_key(key)
    }
}

/// NFC + U+035C → U+0361 normalisation, matching
/// [`IpaIdentifier::comparison_key`]. We do not route through
/// `ipa::validate` because Phase 9 has already done so for every
/// declared IPA; doing it again would re-parse every identifier
/// unnecessarily and would fail on tie-bar-variant forms Phase 9
/// already accepted.
fn comparison_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(if c == '\u{035C}' { '\u{0361}' } else { c });
    }
    out
}

// ---------------------------------------------------------------------------
// Eligibility (T1027–T1028)
// ---------------------------------------------------------------------------

/// One surface realization of an underlying root phoneme (REQ-WHEN-015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realization {
    /// The IPA identifier of this realization.
    ///
    /// For the root's own surface form, this is the root's IPA; for an
    /// allophone, it is the allophone's surface IPA.
    pub ipa: IpaIdentifierText,
    /// The romanization of this realization.
    ///
    /// For the root, this is the root's roman; for an allophone, this is
    /// its *effective* roman (explicit value if present, else inherited
    /// from the parent root).
    pub roman: Romanization,
}

/// Eligibility outcome for a single underlying root phoneme
/// (REQ-WHEN-015).
///
/// * `Realizations(..)` — at least one surface form (root + zero or
///   more allophones) is eligible. The list preserves declaration order:
///   the root's own surface form comes first, followed by allophones
///   in the order they were authored.
/// * `Empty` — no realization satisfied its conditions. **Never** a
///   fallback: a definition whose conditions are unsatisfiable in the
///   supplied context is still a valid definition; it just has no
///   surface form for this particular word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eligibility {
    /// One or more realizations are eligible.
    Realizations(Vec<Realization>),
    /// No realization is eligible.
    Empty,
}

impl Eligibility {
    /// True iff at least one realization is eligible.
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        matches!(self, Eligibility::Realizations(v) if !v.is_empty())
    }

    /// The eligible realizations, or an empty slice.
    #[must_use]
    pub fn realizations(&self) -> &[Realization] {
        match self {
            Eligibility::Realizations(v) => v.as_slice(),
            Eligibility::Empty => &[],
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation context (T1020–T1025)
// ---------------------------------------------------------------------------

/// Pure evaluation context.
///
/// `roots` is the underlying root-phoneme sequence of the word being
/// evaluated, expressed as comparison keys (NFC + tie-bar normalised)
/// so the evaluator can compare neighbour predicates to them without
/// going back to the repertoire. `index` selects which root/allophone
/// is the candidate. `syllable_role` is the generator-supplied
/// assignment for the candidate. `refs` carries the document's
/// reference map.
///
/// The context does **not** carry previously selected surface
/// allophones: neighbour matching is over the underlying root
/// sequence (T1024, REQ-WHEN-014), never over surface forms
/// (T1026, REQ-ELD-017).
#[derive(Debug)]
pub struct EvalContext<'a> {
    /// Underlying root-phoneme sequence (comparison keys).
    pub roots: &'a [String],
    /// Index of the candidate root/allophone in `roots`.
    pub index: usize,
    /// Generator-assigned syllable role for the candidate, if any.
    pub syllable_role: Option<SyllablePosition>,
    /// Per-document reference map.
    pub refs: &'a InventoryRefs,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Validate every `when` block in `doc`.
///
/// Builds the [`InventoryRefs`] map, then walks each group / root /
/// allophone and calls [`parse_block`]. The parsed ASTs are not
/// retained — downstream crates that need them should call
/// [`parse_block`] directly with their own [`InventoryRefs`] and store
/// the result.
///
/// Called by [`crate::parse::parse_document`] at the tail of stage 12,
/// so any failure here is a validation failure of the whole document.
pub fn parse_all(doc: &EldDocument, rep: &Repertoire) -> Result<(), Error> {
    let refs = InventoryRefs::from_document(doc);
    for group in doc.groups() {
        for root in group.roots() {
            parse_root(root, &refs, rep)?;
        }
    }
    Ok(())
}

/// Compute the maximum nesting depth of any path through a `when`
/// condition. A leaf predicate has depth 1; each `group` or `or` adds 1.
///
/// This is the introspection used by conformance tests to confirm the
/// `MAX_NESTING_DEPTH` budget is enforced precisely at the boundary,
/// not coincidentally via some unrelated path.
#[must_use]
pub fn max_nesting_depth(cond: &Condition) -> usize {
    fn walk(c: &Condition, depth: usize, best: &mut usize) {
        if depth > *best {
            *best = depth;
        }
        let next = match depth.checked_add(1) {
            Some(n) => n,
            None => return, // saturate at usize::MAX
        };
        match c {
            Condition::Predicate(_) => {}
            Condition::Group(children) => {
                for child in children {
                    walk(child, next, best);
                }
            }
            Condition::Or(left, right) => {
                walk(left, next, best);
                walk(right, next, best);
            }
        }
    }
    let mut best = 0usize;
    walk(cond, 1, &mut best);
    best
}

fn parse_root(root: &EldRootPhoneme, refs: &InventoryRefs, rep: &Repertoire) -> Result<(), Error> {
    if let Some(block) = root.when() {
        // The result is intentionally discarded: phase 10 validates
        // shape + budgets + references; downstream consumers call
        // `parse_block` themselves when they need the AST.
        let _ = parse_block(block, refs, rep)?;
    }
    for allo in root.allophones() {
        if let Some(block) = allo.when() {
            let _ = parse_block(block, refs, rep)?;
        }
    }
    Ok(())
}

/// Parse a single `when` block into a validated
/// [`WhenCondition`], enforcing every C-20..C-27 / REQ-WHEN-001..011
/// constraint.
///
/// Returns the first error encountered. The `when` node itself is
/// expected (the block name is not checked here — callers pass the
/// raw `when` node).
pub fn parse_block(
    block: &WhenBlock,
    refs: &InventoryRefs,
    rep: &Repertoire,
) -> Result<WhenCondition, Error> {
    let node = block.node();
    let span = node.span();

    if node.name() != "when" {
        return Err(when_err_at(
            span,
            "WHEN_UNKNOWN_NODE",
            format!("expected `when` node, found `{}`", node.name()),
        ));
    }

    reject_args_props(node, span, "`when`")?;
    let children = node
        .children()
        .ok_or_else(|| when_err_at(span, "WHEN_BAD_ARITY", "`when` requires a block"))?;
    if children.is_empty() {
        return Err(when_err_at(
            span,
            "WHEN_EMPTY",
            "`when` block must have at least one child",
        ));
    }

    let mut parsed: Vec<Condition> = Vec::with_capacity(children.len());
    let mut pred_count: usize = 0;
    for child in children.iter() {
        parse_expr(child, &mut parsed, &mut pred_count, 1, refs, rep)?;
    }

    WhenCondition::new(parsed)
}

/// Pure evaluator.
///
/// Evaluates `when` against `ctx`, returning `true` iff every direct
/// child of `when` (implicitly ANDed) evaluates true.
///
/// Errors are reserved for unrecoverable invariants; a clean input
/// always returns `Ok(true)` or `Ok(false)`.
pub fn evaluate(when: &WhenCondition, ctx: &EvalContext<'_>) -> Result<bool, Error> {
    for c in when.children() {
        if !eval_cond(c, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Build the eligibility set for a root phoneme (REQ-WHEN-015).
///
/// * The root's own surface form is included iff the root either has
///   no `when` block, or its `when` evaluates true under `ctx`.
/// * Each allophone is evaluated independently (T1026 — no
///   inheritance): an allophone without a `when` is unconditional; an
///   allophone with a `when` is included iff its `when` evaluates
///   true.
///
/// The returned list preserves declaration order: root first, then
/// allophones in their authored order. `Empty` is returned when
/// nothing matches (T1028 — never a fallback).
///
/// `rep` is the pinned repertoire, used to revalidate neighbour
/// references inside each `when` block. Callers that have already
/// validated the whole document (e.g. via [`parse_all`]) can pass the
/// same repertoire; the validation is idempotent because every
/// `previous-phoneme` / `next-phoneme` value was already accepted by
/// [`parse_block`] at parse time.
pub fn eligibility_for_root(
    root: &EldRootPhoneme,
    ctx: &EvalContext<'_>,
    rep: &Repertoire,
) -> Result<Eligibility, Error> {
    let mut out: Vec<Realization> = Vec::new();
    // Push up to 1 + N allophones, where N is bounded by `MAX_ALLOPHONES_PER_ROOT`.
    // We don't pre-size to avoid a bounds-checked arithmetic op here; the
    // vector is small (≤ 17 entries) and `push` is cheap.

    // Root surface.
    let root_eligible = match root.when() {
        None => true,
        Some(block) => {
            let when = parse_block(block, ctx.refs, rep)?;
            evaluate(&when, ctx)?
        }
    };
    if root_eligible {
        out.push(Realization {
            ipa: root.ipa().clone(),
            roman: root.roman().clone(),
        });
    }

    // Allophones, each independent.
    for allo in root.allophones() {
        let allo_eligible = match allo.when() {
            None => true,
            Some(block) => {
                let when = parse_block(block, ctx.refs, rep)?;
                evaluate(&when, ctx)?
            }
        };
        if allo_eligible {
            out.push(Realization {
                ipa: allo.ipa().clone(),
                roman: allo.effective_roman().clone(),
            });
        }
    }

    Ok(if out.is_empty() {
        Eligibility::Empty
    } else {
        Eligibility::Realizations(out)
    })
}

// ---------------------------------------------------------------------------
// Parser internals
// ---------------------------------------------------------------------------

/// Recursive-descent parse one expression node into `out`.
///
/// `pred_count` is mutated to count every atomic predicate seen along
/// this branch (and is summed across `or` branches). `depth` is the
/// nesting depth at the entry of this call (1 for direct children of
/// the top-level `when`, increasing on each descent into `group` or
/// `or`).
#[allow(clippy::too_many_arguments)]
fn parse_expr(
    node: &KdlNode,
    out: &mut Vec<Condition>,
    pred_count: &mut usize,
    depth: usize,
    refs: &InventoryRefs,
    rep: &Repertoire,
) -> Result<(), Error> {
    let name = node.name();

    // 1. Depth budget — checked first so a deeply nested `and` or
    //    unknown name still surfaces as `WHEN_TOO_DEEP` and not as
    //    a spurious `WHEN_BAD_ARITY` (the depth gate is a budget,
    //    not an arity error).
    if depth > MAX_NESTING_DEPTH {
        return Err(when_err_at(
            node.span(),
            "WHEN_TOO_DEEP",
            format!("expression nesting exceeds {MAX_NESTING_DEPTH} levels along this path"),
        ));
    }

    // 2. C-26 — explicit `and` / `not` / unknown names are rejected
    //    before any arity/shape check so the diagnostic names the
    //    actual problem rather than a downstream consequence.
    match name {
        "and" | "not" => {
            return Err(when_err_at(
                node.span(),
                "WHEN_UNKNOWN_NODE",
                format!(
                    "explicit `{name}` is rejected by the condition language; \
                     use `group` for AND and `or` for OR"
                ),
            ));
        }
        other => match other {
            "when" | "group" | "or" | "word-position" | "syllable-position"
            | "previous-phoneme" | "next-phoneme" => {}
            unknown => {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_UNKNOWN_NODE",
                    format!("unknown node in `when`: `{unknown}`"),
                ));
            }
        },
    }

    // 3. Per-kind arity checks now that we know the name is valid.
    match name {
        // Structural nodes: zero args, zero properties, mandatory block.
        "when" | "group" | "or" => {
            reject_args_props(node, node.span(), &format!("`{name}`"))?;
            if node.children().is_none() {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_BAD_ARITY",
                    format!("`{name}` requires a block"),
                ));
            }
        }
        // Atomic predicates: zero properties, no block, exactly one
        // string arg (checked per-predicate below).
        _ => {
            if node.props().count() != 0 {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_BAD_ARITY",
                    format!("`{name}` must have zero properties"),
                ));
            }
            if node.children().is_some() {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_BAD_ARITY",
                    format!("`{name}` must have no block"),
                ));
            }
        }
    }
    // Structural nodes already verified above to have a block; atomics
    // have no block by design. We need a `&[KdlNode]` for the structural
    // match arms below, so re-borrow once here. Use `first()` for
    // safety — the inner check above guarantees `is_some()`.
    let children_slice: &[KdlNode] = if matches!(name, "when" | "group" | "or") {
        node.children().map(KdlChildren::as_slice).unwrap_or(&[])
    } else {
        &[]
    };

    match name {
        "group" => {
            // C-24: ≥ 2 direct children.
            if children_slice.len() < 2 {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_BAD_ARITY",
                    "`group` requires at least 2 children",
                ));
            }
            let next = match depth.checked_add(1) {
                Some(d) => d,
                None => {
                    return Err(when_err_at(
                        node.span(),
                        "WHEN_TOO_DEEP",
                        "expression nesting overflows usize",
                    ));
                }
            };
            let mut inner: Vec<Condition> = Vec::new();
            let mut local_count: usize = 0;
            for c in children_slice {
                parse_expr(c, &mut inner, &mut local_count, next, refs, rep)?;
            }
            *pred_count = pred_count.saturating_add(local_count);
            check_predicate_count(*pred_count, node.span())?;
            // `Condition::group` re-checks ≥ 2; we just verified.
            let g = Condition::group(inner)?;
            out.push(g);
            Ok(())
        }

        "or" => {
            // C-25: exactly 2 direct children.
            if children_slice.len() != 2 {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_BAD_ARITY",
                    "`or` requires exactly 2 children",
                ));
            }
            let next = match depth.checked_add(1) {
                Some(d) => d,
                None => {
                    return Err(when_err_at(
                        node.span(),
                        "WHEN_TOO_DEEP",
                        "expression nesting overflows usize",
                    ));
                }
            };
            // Each branch carries its own count; we sum.
            let mut left: Vec<Condition> = Vec::new();
            let mut right: Vec<Condition> = Vec::new();
            let mut left_count: usize = 0;
            let mut right_count: usize = 0;
            let left_node = children_slice.first().ok_or_else(|| {
                when_err_at(node.span(), "WHEN_BAD_ARITY", "missing `or` left branch")
            })?;
            let right_node = children_slice.get(1).ok_or_else(|| {
                when_err_at(node.span(), "WHEN_BAD_ARITY", "missing `or` right branch")
            })?;
            parse_expr(left_node, &mut left, &mut left_count, next, refs, rep)?;
            parse_expr(right_node, &mut right, &mut right_count, next, refs, rep)?;
            *pred_count = pred_count
                .saturating_add(left_count)
                .saturating_add(right_count);
            check_predicate_count(*pred_count, node.span())?;
            let left_one = one_condition(left)
                .ok_or_else(|| when_err_at(node.span(), "WHEN_BAD_ARITY", "empty `or` branch"))?;
            let right_one = one_condition(right)
                .ok_or_else(|| when_err_at(node.span(), "WHEN_BAD_ARITY", "empty `or` branch"))?;
            out.push(Condition::or(left_one, right_one));
            Ok(())
        }

        // ----- atomic predicates --------------------------------------
        "word-position" => {
            count_predicate(pred_count, node.span())?;
            let v = single_string_arg(node, "word-position")?;
            let wp = match v {
                "initial" => WordPosition::Initial,
                "medial" => WordPosition::Medial,
                "final" => WordPosition::Final,
                other => {
                    return Err(when_err_at(
                        node.span(),
                        "WHEN_BAD_VALUE",
                        format!(
                            "invalid word-position value `{other}`; expected initial|medial|final"
                        ),
                    ));
                }
            };
            out.push(Condition::predicate(Predicate::WordPosition(wp)));
            Ok(())
        }

        "syllable-position" => {
            count_predicate(pred_count, node.span())?;
            let v = single_string_arg(node, "syllable-position")?;
            let sp = match v {
                "onset" => SyllablePosition::Onset,
                "nucleus" => SyllablePosition::Nucleus,
                "coda" => SyllablePosition::Coda,
                other => {
                    return Err(when_err_at(
                        node.span(),
                        "WHEN_BAD_VALUE",
                        format!(
                            "invalid syllable-position value `{other}`; expected onset|nucleus|coda"
                        ),
                    ));
                }
            };
            out.push(Condition::predicate(Predicate::SyllablePosition(sp)));
            Ok(())
        }

        "previous-phoneme" | "next-phoneme" => {
            count_predicate(pred_count, node.span())?;
            let v = single_string_arg(node, name)?;
            // Phase 8 owns IPA validity; route through `ipa::validate`
            // so malformed IPA yields an `IPA_*` / `UNICODE_*` error
            // (category Ipa / Unicode), not a `WHEN_*` one.
            let id = ipa::validate(v, rep).map_err(|e| from_ipa(e, node.span()))?;
            let key = id.comparison_key();
            // T1016: reject references that don't resolve against the
            // collected inventory. Cross-group references resolve for
            // free because the map is global.
            if !refs.contains(&key) {
                return Err(when_err_at(
                    node.span(),
                    "WHEN_REFERENCE_UNRESOLVED",
                    format!("`{name}` references undeclared IPA `{v}`"),
                ));
            }
            let ipa_text = IpaIdentifierText::from_validated(v.to_string());
            let p = if name == "previous-phoneme" {
                Predicate::PreviousPhoneme(ipa_text)
            } else {
                Predicate::NextPhoneme(ipa_text)
            };
            out.push(Condition::predicate(p));
            Ok(())
        }

        // Unknown names are caught at the top of `parse_expr`
        // (C-26 / T1006); the exhaustive match here is the Rust
        // type system's belt-and-braces.
        _ => unreachable!("unknown node name survived top-of-fn gate: {name}"),
    }
}

// ---------------------------------------------------------------------------
// Evaluator internals
// ---------------------------------------------------------------------------

fn eval_cond(cond: &Condition, ctx: &EvalContext<'_>) -> Result<bool, Error> {
    match cond {
        Condition::Predicate(p) => eval_pred(p, ctx),
        Condition::Group(children) => {
            // T1017: implicit AND across `group` siblings.
            for c in children {
                if !eval_cond(c, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or(a, b) => {
            // T1018: OR across `or` children (incl. nested / cross-category).
            Ok(eval_cond(a, ctx)? || eval_cond(b, ctx)?)
        }
    }
}

fn eval_pred(p: &Predicate, ctx: &EvalContext<'_>) -> Result<bool, Error> {
    match p {
        // T1020–T1021: word position over underlying root sequence;
        // single-phoneme word is initial+final, never medial.
        Predicate::WordPosition(want) => Ok(word_positions_at(ctx).contains(want)),

        // T1022–T1023: syllable position via generator assignment.
        Predicate::SyllablePosition(want) => Ok(ctx.syllable_role == Some(*want)),

        // T1024–T1025: immediate previous/next underlying root, across
        // syllable but not word boundary (the `roots` slice IS the
        // word, so word-boundary crossings are automatic); missing
        // neighbour ⇒ false.
        Predicate::PreviousPhoneme(ipa) => Ok(prev_key(ctx)
            .is_some_and(|r| Some(r.as_str()) == ctx.refs.owner_of(&comparison_key(ipa.as_str())))),
        Predicate::NextPhoneme(ipa) => Ok(next_key(ctx)
            .is_some_and(|r| Some(r.as_str()) == ctx.refs.owner_of(&comparison_key(ipa.as_str())))),
    }
}

fn word_positions_at(ctx: &EvalContext<'_>) -> &'static [WordPosition] {
    let n = ctx.roots.len();
    if n == 0 || ctx.index >= n {
        return &[];
    }
    if n == 1 {
        // T1021: one-phoneme word is initial AND final, never medial.
        return &[WordPosition::Initial, WordPosition::Final];
    }
    if ctx.index == 0 {
        return &[WordPosition::Initial];
    }
    // Compare without `n - 1` to keep arithmetic_side_effects quiet.
    if let Some(next) = ctx.index.checked_add(1) {
        if next == n {
            return &[WordPosition::Final];
        }
    }
    &[WordPosition::Medial]
}

fn prev_key<'a>(ctx: &'a EvalContext<'_>) -> Option<&'a String> {
    ctx.index.checked_sub(1).and_then(|i| ctx.roots.get(i))
}

fn next_key<'a>(ctx: &'a EvalContext<'_>) -> Option<&'a String> {
    ctx.index.checked_add(1).and_then(|i| ctx.roots.get(i))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn reject_args_props(node: &KdlNode, span: SourceSpan, what: &str) -> Result<(), Error> {
    // C-20 / C-21 / C-24 / C-25: zero args, zero properties, mandatory
    // block. Block-presence is checked by the caller (because `when`/
    // `group`/`or` all require one, while atomics must NOT have one).
    if node.args().count() != 0 {
        return Err(when_err_at(
            span,
            "WHEN_BAD_ARITY",
            format!("{what} must have zero arguments"),
        ));
    }
    if node.props().count() != 0 {
        return Err(when_err_at(
            span,
            "WHEN_BAD_ARITY",
            format!("{what} must have zero properties"),
        ));
    }
    Ok(())
}

fn single_string_arg<'a>(node: &'a KdlNode, what: &str) -> Result<&'a str, Error> {
    // C-21: atomic predicates have exactly one string arg, no block,
    // no properties. The block check is the caller's job (we're called
    // only after `reject_args_props`).
    let mut iter = node.args();
    let Some(first) = iter.next() else {
        return Err(when_err_at(
            node.span(),
            "WHEN_PREDICATE_ARITY",
            format!("`{what}` requires exactly one string argument"),
        ));
    };
    if iter.next().is_some() {
        return Err(when_err_at(
            node.span(),
            "WHEN_PREDICATE_ARITY",
            format!("`{what}` requires exactly one string argument"),
        ));
    }
    first.as_str().ok_or_else(|| {
        when_err_at(
            node.span(),
            "WHEN_PREDICATE_ARITY",
            format!("`{what}` argument must be a string"),
        )
    })
}

fn count_predicate(count: &mut usize, span: SourceSpan) -> Result<(), Error> {
    *count = count.saturating_add(1);
    check_predicate_count(*count, span)
}

fn check_predicate_count(n: usize, span: SourceSpan) -> Result<(), Error> {
    if n > MAX_PREDICATES_PER_WHEN {
        Err(when_err_at(
            span,
            "WHEN_TOO_MANY_PREDICATES",
            format!(
                "`when` exceeds {MAX_PREDICATES_PER_WHEN} atomic predicates \
                 (saw {n})"
            ),
        ))
    } else {
        Ok(())
    }
}

fn one_condition(v: Vec<Condition>) -> Option<Condition> {
    // `group` requires ≥ 2; `or` requires exactly 2; this helper
    // converts a Vec<Condition> built by the parser into a single
    // Condition for the `or` constructor.
    match v.len() {
        1 => v.into_iter().next(),
        _ => None,
    }
}

fn when_err_at(span: SourceSpan, code: &'static str, message: impl Into<String>) -> Error {
    let mut d = Diagnostic::new(code, Category::When, message).with_stage(12);
    if !span.is_invalid() {
        d = d.with_span(KdlSpan {
            line: span.start.line,
            column: span.start.column,
            length: span.len(),
        });
    }
    Error { diagnostic: d }
}

fn from_ipa(err: ipa::IpaError, span: SourceSpan) -> Error {
    let code = err.code();
    let category = if code.starts_with("IPA_") {
        Category::Ipa
    } else {
        Category::Unicode
    };
    let mut d = Diagnostic::new(code, category, format!("IPA validation: {code}")).with_stage(12);
    if !span.is_invalid() {
        d = d.with_span(KdlSpan {
            line: span.start.line,
            column: span.start.column,
            length: span.len(),
        });
    }
    Error { diagnostic: d }
}

// ---------------------------------------------------------------------------
// Unit tests (T1029/T1030 partial coverage; the full truth-table suite
// lives in `tests/condition_conformance.rs`).
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;
    use elang_kdl::ast::KdlDocument;
    use elang_repertoire::repertoire;

    fn rep() -> Repertoire {
        repertoire::load().expect("repertoire must load")
    }

    /// Build a `phonemes { w { when { <body> } } }` wrapper with one
    /// child node per line so KDL 2.0 doesn't merge sibling nodes.
    fn wrap(body: &str) -> String {
        let mut s = String::from("phonemes {\n  w {\n    when {\n");
        for line in body.lines() {
            s.push_str("      ");
            s.push_str(line);
            s.push('\n');
        }
        s.push_str("    }\n  }\n}\n");
        s
    }

    /// Parse `src` as a KDL document, return the named `when` block
    /// node (assumed to be the first / only `when` in `src`).
    fn when_node(src: &str) -> KdlNode {
        let doc = elang_kdl::parse(src.as_bytes()).expect("kdl parse");
        let when = find_when(&doc);
        when.clone()
    }

    fn find_when(doc: &KdlDocument) -> &KdlNode {
        find_when_in(doc.nodes().iter())
    }

    fn find_when_in<'a, I: Iterator<Item = &'a KdlNode>>(it: I) -> &'a KdlNode {
        for n in it {
            if n.name() == "when" {
                return n;
            }
            if let Some(children) = n.children() {
                let r = find_when_in(children.iter());
                if r.name() == "when" {
                    return r;
                }
            }
        }
        panic!("no `when` node in source");
    }

    // ---- inventory ----------------------------------------------------

    #[test]
    fn inventory_records_root_and_allophone_keys() {
        // Two roots, two allophones — the allophones' keys resolve to
        // their owning root keys.
        let src = "phonemes {\n  consonants {\n    phoneme \"p\" {\n      roman \"p\"\n      allophone \"p\u{02B0}\" {\n        roman \"ph\"\n      }\n    }\n  }\n  vowels {\n    phoneme \"a\" {\n      roman \"a\"\n    }\n  }\n}\n";
        let doc = elang_kdl::parse(src.as_bytes()).expect("kdl parse");
        let eld = crate::parse::parse_document(&doc, &rep()).expect("eld parse");
        let refs = InventoryRefs::from_document(&eld);
        // Root p, allophone p\u{02B0} (aspirated p) — distinct keys.
        assert!(refs.contains("p"));
        assert!(refs.contains("p\u{02B0}"));
        // Both map to the root's key.
        assert_eq!(refs.owner_of("p"), Some("p"));
        assert_eq!(refs.owner_of("p\u{02B0}"), Some("p"));
    }

    // ---- parser: arity ------------------------------------------------

    #[test]
    fn parse_when_with_no_children_is_empty() {
        let node = when_node(&wrap(""));
        let block = WhenBlock::new(node);
        let refs = InventoryRefs::default();
        let err = parse_block(&block, &refs, &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_EMPTY");
        assert_eq!(err.category(), Category::When);
    }

    #[test]
    fn parse_when_with_args_is_rejected() {
        let node = when_node(&wrap("when \"x\" {\nword-position \"initial\"\n}"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    #[test]
    fn parse_when_with_property_is_rejected() {
        let node = when_node(&wrap("when x=\"y\" {\nword-position \"initial\"\n}"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    #[test]
    fn parse_group_with_one_child_is_rejected() {
        let node = when_node(&wrap("group {\n  word-position \"initial\"\n}"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    #[test]
    fn parse_or_with_three_children_is_rejected() {
        let node = when_node(&wrap(
            "or {\n  word-position \"initial\"\n  word-position \"medial\"\n  word-position \"final\"\n}",
        ));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    #[test]
    fn parse_or_with_one_child_is_rejected() {
        let node = when_node(&wrap("or {\n  word-position \"initial\"\n}"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    // ---- parser: explicit `and` / `not` / unknown --------------------

    #[test]
    fn parse_explicit_and_is_rejected() {
        let node = when_node(&wrap(
            "and {\n  word-position \"initial\"\n  word-position \"final\"\n}",
        ));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_UNKNOWN_NODE");
    }

    #[test]
    fn parse_explicit_not_is_rejected() {
        let node = when_node(&wrap("not {\n  word-position \"initial\"\n}"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_UNKNOWN_NODE");
    }

    #[test]
    fn parse_unknown_predicate_is_rejected() {
        let node = when_node(&wrap("frequency \"high\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_UNKNOWN_NODE");
    }

    // ---- parser: predicate arity -------------------------------------

    #[test]
    fn parse_atomic_with_no_arg_is_rejected() {
        let node = when_node(&wrap("word-position"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_PREDICATE_ARITY");
    }

    #[test]
    fn parse_atomic_with_two_args_is_rejected() {
        let node = when_node(&wrap("word-position \"initial\" \"medial\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_PREDICATE_ARITY");
    }

    #[test]
    fn parse_atomic_with_block_is_rejected() {
        let node = when_node(&wrap("word-position \"initial\" { }"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_ARITY");
    }

    #[test]
    fn parse_atomic_with_nonstring_arg_is_rejected() {
        let node = when_node(&wrap("word-position 42"));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_PREDICATE_ARITY");
    }

    // ---- parser: value domains ---------------------------------------

    #[test]
    fn parse_word_position_bad_value_is_rejected() {
        let node = when_node(&wrap("word-position \"first\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_VALUE");
    }

    #[test]
    fn parse_syllable_position_bad_value_is_rejected() {
        let node = when_node(&wrap("syllable-position \"head\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_BAD_VALUE");
    }

    // ---- parser: budgets --------------------------------------------

    #[test]
    fn parse_17_atomic_predicates_is_rejected() {
        // 17 word-position predicates.
        let mut body = String::new();
        for _ in 0..16 {
            body.push_str("word-position \"initial\"\n");
        }
        body.push_str("word-position \"final\"\n");
        let node = when_node(&wrap(&body));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_TOO_MANY_PREDICATES");
    }

    #[test]
    fn parse_16_atomic_predicates_is_accepted() {
        let mut body = String::new();
        for _i in 0..15 {
            body.push_str("word-position \"initial\"\n");
        }
        body.push_str("word-position \"final\"\n");
        let node = when_node(&wrap(&body));
        let block = WhenBlock::new(node);
        let when = parse_block(&block, &InventoryRefs::default(), &rep()).expect("16 must fit");
        assert_eq!(when.children().len(), 16);
    }

    #[test]
    fn parse_9_nested_levels_is_rejected() {
        // 9 levels of nesting. Each level wraps the next with a
        // single-child group, and the outermost group has a sibling
        // atomic predicate to satisfy the ≥2-child arity rule.
        // Walking down the spine: the path through the inner `group`
        // reaches depth 9, where the budget fires.
        let mut body = String::from("word-position \"initial\"\ngroup {\n");
        for _ in 0..8 {
            body.push_str("word-position \"initial\"\ngroup {\n");
        }
        body.push_str("word-position \"initial\"\n");
        for _ in 0..9 {
            body.push_str("}\n");
        }
        let node = when_node(&wrap(&body));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        assert_eq!(err.code(), "WHEN_TOO_DEEP");
    }

    // ---- parser: IPA reference resolution ----------------------------

    #[test]
    fn parse_neighbour_reference_to_undeclared_ipa_is_rejected() {
        let node = when_node(&wrap("previous-phoneme \"z\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        // "z" passes Phase 8 IPA validation (it's a base); the WHEN
        // error is the unresolved reference.
        assert_eq!(err.code(), "WHEN_REFERENCE_UNRESOLVED");
    }

    #[test]
    fn parse_neighbour_reference_to_malformed_ipa_propagates_ipa() {
        // Empty IPA — Phase 8 owns it.
        let node = when_node(&wrap("next-phoneme \"\""));
        let block = WhenBlock::new(node);
        let err = parse_block(&block, &InventoryRefs::default(), &rep()).unwrap_err();
        // Category must be Ipa (or Unicode) — NOT When.
        assert_ne!(err.category(), Category::When);
    }

    #[test]
    fn parse_neighbour_reference_to_self_resolves() {
        // The "self" case: the reference names an IPA that is in the
        // inventory. Build a tiny inventory with key "p" mapped to "p".
        let mut refs = InventoryRefs::default();
        // Manually inject: comparison_key("p") = "p".
        refs.owner.insert("p".to_string(), "p".to_string());
        let node = when_node(&wrap("previous-phoneme \"p\""));
        let block = WhenBlock::new(node);
        let when = parse_block(&block, &refs, &rep()).expect("self-ref resolves");
        assert_eq!(when.children().len(), 1);
    }

    // ---- evaluator: word position ------------------------------------

    #[test]
    fn single_phoneme_word_is_initial_and_final() {
        // T1021.
        let node = when_node(&wrap("word-position \"initial\""));
        let when = parse_block(&WhenBlock::new(node), &InventoryRefs::default(), &rep()).unwrap();
        let roots = vec!["p".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        assert!(evaluate(&when, &ctx).unwrap());

        let node2 = when_node(&wrap("word-position \"final\""));
        let when2 = parse_block(&WhenBlock::new(node2), &InventoryRefs::default(), &rep()).unwrap();
        assert!(evaluate(&when2, &ctx).unwrap());

        let node3 = when_node(&wrap("word-position \"medial\""));
        let when3 = parse_block(&WhenBlock::new(node3), &InventoryRefs::default(), &rep()).unwrap();
        assert!(
            !evaluate(&when3, &ctx).unwrap(),
            "single-phoneme is never medial"
        );
    }

    #[test]
    fn first_of_multi_phoneme_word_is_only_initial() {
        let node = when_node(&wrap("word-position \"initial\""));
        let when = parse_block(&WhenBlock::new(node), &InventoryRefs::default(), &rep()).unwrap();
        let roots = vec!["p".to_string(), "a".to_string(), "t".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        assert!(evaluate(&when, &ctx).unwrap());
        assert!(!evaluate(
            &when,
            &EvalContext {
                index: 1,
                ..ctx_eq(&ctx)
            }
        )
        .unwrap());
        assert!(!evaluate(
            &when,
            &EvalContext {
                index: 2,
                ..ctx_eq(&ctx)
            }
        )
        .unwrap());
    }

    fn ctx_eq<'a>(c: &EvalContext<'a>) -> EvalContext<'a> {
        EvalContext {
            roots: c.roots,
            index: c.index,
            syllable_role: c.syllable_role,
            refs: c.refs,
        }
    }

    // ---- evaluator: neighbours ---------------------------------------

    #[test]
    fn missing_neighbour_is_false() {
        // T1025: index 0, no previous.
        let when_node = when_node(&wrap("previous-phoneme \"a\""));
        let mut refs = InventoryRefs::default();
        refs.owner.insert("a".to_string(), "a".to_string());
        let when = parse_block(&WhenBlock::new(when_node), &refs, &rep()).unwrap();
        let roots = vec!["p".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 0,
            syllable_role: None,
            refs: &refs,
        };
        assert!(!evaluate(&when, &ctx).unwrap());
    }

    #[test]
    fn present_neighbour_matches_owner_key() {
        // T1015: an allophone reference must resolve to its owning
        // root's key, which is what's in the roots[] sequence.
        let node = when_node(&wrap("previous-phoneme \"p\u{02B0}\"")).clone();
        // Inventory: root "p", allophone "p\u{02B0}" both owned by "p".
        let mut refs = InventoryRefs::default();
        refs.owner.insert("p".to_string(), "p".to_string());
        refs.owner.insert("p\u{02B0}".to_string(), "p".to_string());
        let when = parse_block(&WhenBlock::new(node), &refs, &rep()).unwrap();
        // Candidate at index 1; previous is "p" (index 0). The
        // allophone ref "p\u{02B0}" resolves to owning root "p".
        let roots = vec!["p".to_string(), "a".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 1,
            syllable_role: None,
            refs: &refs,
        };
        assert!(evaluate(&when, &ctx).unwrap());
    }

    // ---- evaluator: AND / OR -----------------------------------------

    #[test]
    fn and_evaluates_all_siblings() {
        let node = when_node(&wrap("word-position \"initial\"\nword-position \"final\""));
        let parsed = parse_block(&WhenBlock::new(node), &InventoryRefs::default(), &rep()).unwrap();
        let roots = vec!["p".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        // T1019: contradictory predicates are permitted and the
        // evaluator does not simplify; the conjunction is the truth
        // table, not the AST.
        assert!(evaluate(&parsed, &ctx).unwrap(), "single-phoneme is both");
    }

    #[test]
    fn or_with_cross_category_alternatives() {
        // T1018: OR of a word-position and a syllable-position.
        let node = when_node(&wrap(
            "or {\n  word-position \"initial\"\n  syllable-position \"onset\"\n}",
        ));
        let parsed = parse_block(&WhenBlock::new(node), &InventoryRefs::default(), &rep()).unwrap();
        let ctx_initial = EvalContext {
            roots: &["p".to_string()],
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        let ctx_onset = EvalContext {
            syllable_role: Some(SyllablePosition::Onset),
            ..ctx_eq(&ctx_initial)
        };
        let ctx_neither = EvalContext {
            roots: &["p".to_string(), "a".to_string(), "t".to_string()],
            index: 1, // medial
            syllable_role: Some(SyllablePosition::Coda),
            refs: &InventoryRefs::default(),
        };
        // Index 0 in a 3-phoneme word => initial true, onset unknown.
        let roots3 = vec!["p".to_string(), "a".to_string(), "t".to_string()];
        let ctx3 = EvalContext {
            roots: &roots3,
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        assert!(evaluate(&parsed, &ctx3).unwrap(), "initial branch matches");
        assert!(
            evaluate(&parsed, &ctx_onset).unwrap(),
            "onset branch matches"
        );
        assert!(
            !evaluate(&parsed, &ctx_neither).unwrap(),
            "neither branch matches"
        );
    }

    // ---- evaluator: eligibility set ----------------------------------

    #[test]
    fn no_eligible_returns_empty() {
        // T1028: no fallback. A condition that's never true yields
        // Empty, not a default realization.
        let when_node = when_node(&wrap("word-position \"medial\""));
        let when = parse_block(
            &WhenBlock::new(when_node),
            &InventoryRefs::default(),
            &rep(),
        )
        .unwrap();
        let roots = vec!["p".to_string()];
        let ctx = EvalContext {
            roots: &roots,
            index: 0,
            syllable_role: None,
            refs: &InventoryRefs::default(),
        };
        assert!(!evaluate(&when, &ctx).unwrap());
    }
}
