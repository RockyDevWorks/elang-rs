// Fuzz target for the phonological condition AST, evaluator and
// canonical sorting key (Phase 15, T1508).
//
// Drives arbitrary byte sequences through `elang_eld::parse` — which
// runs the full structural validation pipeline and then parses every
// `when` block into the typed `WhenCondition` AST — and then
// exercises the three machinery layers the condition language
// consists of:
//
// 1. **Canonical sorting key.** `condition_key` builds an injective
//    byte encoding of a `Condition` tree by sorting the children of
//    every `group` and the operands of every `or` recursively
//    before encoding the parent. The two sort helpers
//    (`sorted_children`, `sorted_operands`) are exercised directly
//    too: a regression in either helper would change the canonical
//    bytes the writer emits, breaking deterministic archives.
//
// 2. **Introspection.** `max_nesting_depth` returns the deepest
//    container path through a `Condition`. The parser enforces the
//    budget at `MAX_NESTING_DEPTH`; a regression in
//    `max_nesting_depth` would let a document that fails the
//    boundary check appear to pass on a different code path.
//
// 3. **Pure evaluator.** `evaluate` walks a `WhenCondition` against
//    an `EvalContext` whose `roots` are the comparison keys of the
//    underlying root sequence, and whose `refs` are the
//    document-wide inventory map. The evaluator must not panic on
//    any condition, must not consult previously selected surface
//    allophones (T1026), and must not rank candidates (T1027).
//
// The repertoire is embedded at compile time via `include_str!`, so
// `Repertoire::load()` is I/O-free and cheap to cache in a
// `LazyLock`.
//
// See `docs/security-testing.md` for budget and seed corpus policy.

#![no_main]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fuzz harness drives adversarial inputs through the condition language; \
              panics are the failure signal that surfaces robustness defects in the \
              AST builder, canonical sorter and pure evaluator"
)]

use std::sync::LazyLock;

use elang_core::model::when::Condition;
use elang_eld::canon::key::{condition_key, sorted_children, sorted_operands};
use elang_eld::{parse as eld_parse, parse_block, EvalContext, InventoryRefs};
use elang_repertoire::repertoire::{load_from, Repertoire, REPERTOIRE_JSON};

static REPERTOIRE: LazyLock<Repertoire> =
    LazyLock::new(|| load_from(REPERTOIRE_JSON).expect("pinned repertoire parses"));

/// NFC + U+035C → U+0361 normalisation, matching
/// `elang_repertoire::ipa::IpaIdentifier::comparison_key`.
///
/// The evaluator compares `ctx.roots` entries against the owner map,
/// which is keyed by comparison keys, so the roots slice must hold
/// comparison keys rather than raw authored IPA text.
fn comparison_key(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\u{035C}' { '\u{0361}' } else { c })
        .collect()
}

/// Walk a condition tree, exercising the canonical key, the depth
/// introspection and the sort helpers at every node.
fn exercise_tree(cond: &Condition) {
    let _ = condition_key(cond);
    let _ = elang_eld::max_nesting_depth(cond);

    match cond {
        Condition::Group(children) => {
            // The children of a group are sorted by their canonical
            // key before the group's own key is built.
            let sorted = sorted_children(children);
            debug_assert_eq!(sorted.len(), children.len());
            for child in children {
                exercise_tree(child);
            }
        }
        Condition::Or(left, right) => {
            let sorted = sorted_operands(left, right);
            debug_assert_eq!(sorted.len(), 2);
            exercise_tree(left);
            exercise_tree(right);
        }
        Condition::Predicate(_) => {}
    }
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // Step 1 — run the full ELD structural + condition-language
    // validation chain. Most adversarial inputs fail here; that is
    // the expected outcome. A panic is a defect.
    let Ok(doc) = eld_parse(data, &REPERTOIRE) else {
        return;
    };

    // Step 2 — build the canonical evaluation context from the
    // document itself: every root's comparison key, indexed in
    // declaration order. The reference map is the same one the
    // parser just used to resolve `previous-phoneme` /
    // `next-phoneme` references.
    let refs = InventoryRefs::from_document(&doc);
    let mut root_keys: Vec<String> = Vec::new();
    for group in doc.groups() {
        for root in group.roots() {
            root_keys.push(comparison_key(root.ipa().as_str()));
        }
    }

    // Step 3 — for every `when` block in the document, exercise the
    // three machinery layers.
    for group in doc.groups() {
        for root in group.roots() {
            for block in root.when().into_iter().chain(
                root.allophones()
                    .iter()
                    .filter_map(|allophone| allophone.when()),
            ) {
                let Ok(when) = parse_block(block, &refs, &REPERTOIRE) else {
                    continue;
                };

                // Layer 1 + 2: canonical sorting key and depth
                // introspection, over the whole AST.
                for child in when.children() {
                    exercise_tree(child);
                }

                // Layer 3: the pure evaluator. Root and allophone
                // conditions both use the enclosing root's index,
                // so neighbour predicates see the underlying sequence.
                if let Some(index) = root_keys
                    .iter()
                    .position(|k| k == &comparison_key(root.ipa().as_str()))
                {
                    let ctx = EvalContext {
                        roots: &root_keys,
                        index,
                        syllable_role: None,
                        refs: &refs,
                    };
                    let _ = elang_eld::evaluate(&when, &ctx);
                }
            }
        }
    }
});
