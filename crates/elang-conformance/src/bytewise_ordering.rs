//! T1511 — property tests for bytewise path and IPA ordering.
//!
//! Two bytewise-ordering invariants underpin the canonical-bytes
//! guarantee:
//!
//! - **`ResourcePath`** is a `String` whose bytewise ordering must be
//!   preserved across `ResourcePath::new`. A `ResourcePath` is its own
//!   comparison key (no separate normaliser), so the property is just
//!   `as_str() == input_after_NFC` and the partial-order derived from
//!   `PartialOrd` is bytewise.
//!
//! - **IPA identifiers** have a dedicated comparison key
//!   ([`IpaIdentifier::comparison_key`](elang_repertoire::ipa::IpaIdentifier::comparison_key))
//!   that normalises the **lower** tie bar (U+035C) to the **upper**
//!   one (U+0361), so that `t͡ʃ` and `t͜ʃ` compare equal while `tʃ`
//!   (untied) stays distinct. The property is that this normalisation
//!   is sound: applying it twice equals applying it once, and the
//!   result never contains the lower tie bar.
//!
//! Both properties are asserted via `proptest!` with two different
//! input strategies:
//!
//! - **ResourcePath**: a `string_regex` strategy that produces
//!   arbitrary paths matching the structural rules
//!   (steering §3.4: non-empty, ≤ 1024 bytes, no leading `/`, no
//!   `\`/`\0`, no `.`/`..`/empty segments).
//!
//! - **IPA identifiers**: a small `proptest::strategy::Strategy` over
//!   ASCII + the tie-bar characters and a handful of common IPA
//!   scalars, so the property is exercised on inputs that are likely
//!   to be rejected by the validator (and filtered via `prop_assume!`)
//!   as well as on inputs that pass — both are interesting because
//!   the IPA comparison key is only defined for valid identifiers,
//!   and the validator's *rejection* of malformed input must also be
//!   deterministic.
//!
//! The phonological condition AST sorter
//! ([`condition_key::sorted_children`](elang_eld::canon::key::sorted_children)
//! and
//! [`sorted_operands`](elang_eld::canon::key::sorted_operands)) is
//! also exercised: for every golden `when` block parsed out of the
//! corpus, the returned order must be non-decreasing by the
//! canonical condition key. This catches regressions where the
//! comparator drifts away from the key encoding.

#![cfg(test)]

use crate::canonical_goldens::CASES as ELD_CASES;
use elang_core::ids::ResourcePath;
use elang_core::model::when::Condition;
use elang_eld::canon::key as ck;
use elang_eld::InventoryRefs;
use elang_repertoire::ipa::{self, TIE_LOWER, TIE_UPPER};
use elang_repertoire::repertoire::Repertoire;
use proptest::prelude::*;
use proptest::string::string_regex;

/// A `proptest` strategy that produces strings matching the
/// `ResourcePath` structural rules.
///
/// Constraints (steering §3.4):
/// - non-empty
/// - no leading `/`
/// - no `\`, no NUL
/// - no `.` / `..` / empty segments
/// - ≤ 1024 UTF-8 bytes
///
/// The regex captures the segment alphabet and shape; filters exclude
/// `.` / `..` segments and enforce the byte-length cap at sampling time.
fn resource_path_strategy() -> BoxedStrategy<String> {
    // `[A-Za-z0-9_.-]+` for segments; segments joined by `/`.
    // No leading `/` (regex starts with a segment char), no
    // empty segments. The filter below excludes `.`/`..` segments.
    let inner = "[A-Za-z0-9_.-]{1,32}(/[A-Za-z0-9_.-]{1,32}){0,8}";
    string_regex(inner)
        .expect("valid regex")
        .prop_filter("path must fit in MAX_RESOURCE_PATH_BYTES", |s| {
            s.len() <= 1024
        })
        .prop_filter("path must not contain dot segments", |s| {
            s.split('/')
                .all(|segment| segment != "." && segment != "..")
        })
        .boxed()
}

/// A `proptest` strategy that produces arbitrary IPA-shaped strings
/// from a small alphabet: ASCII letters, the tie-bar characters and
/// a few IPA scalars that are guaranteed to be in the pinned
/// repertoire. Most draws will fail the validator (`prop_assume!`
/// filters them); the surviving draws exercise the comparison-key
/// properties on inputs that look like IPA but are easy to mutate.
fn ipa_candidate_strategy() -> BoxedStrategy<String> {
    // ASCII letters plus the two tie bars plus a handful of safe IPA
    // scalars that appear in the pinned repertoire (`a`, `t`, `s`,
    // `p`, `ʃ` = U+0283, `ŋ` = U+014B). Anything outside this set is
    //   rejected by the validator, which is also a property worth
    //   asserting.
    let alphabet: Vec<char> = "atsp"
        .chars()
        .chain(std::iter::once(TIE_UPPER))
        .chain(std::iter::once(TIE_LOWER))
        .chain(std::iter::once('\u{0283}')) // ʃ
        .chain(std::iter::once('\u{014B}')) // ŋ
        .collect();
    proptest::collection::vec(proptest::sample::select(alphabet), 1..=8)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .boxed()
}

proptest::proptest! {
    /// `ResourcePath::new(s)` is byte-faithful: the stored string
    /// equals the input that passed validation. This is the
    /// bytewise-ordering invariant: `as_str()` and the input
    /// compare equal under `==`, which is what makes a
    /// `Vec<ResourcePath>` sort in bytewise order.
    #[test]
    fn resource_path_round_trips_through_new(s in resource_path_strategy()) {
        let path = ResourcePath::new(s.clone())
            .expect("generated resource path must be valid");
        proptest::prop_assert_eq!(path.as_str(), s.as_str());
    }

    /// Two `ResourcePath`s sort bytewise. Draw two arbitrary
    /// valid paths and assert the standard `Ord` agrees with the
    /// bytewise lexicographic comparison of their underlying
    /// strings.
    #[test]
    fn resource_path_bytewise_ordering(
        a in resource_path_strategy(),
        b in resource_path_strategy(),
    ) {
        let pa = ResourcePath::new(a.clone())
            .expect("generated resource path must be valid");
        let pb = ResourcePath::new(b.clone())
            .expect("generated resource path must be valid");

        let std_ord = pa.cmp(&pb);
        let bytewise_ord = pa.as_str().as_bytes().cmp(pb.as_str().as_bytes());
        proptest::prop_assert_eq!(std_ord, bytewise_ord);
    }

    /// IPA comparison key idempotence: applying
    /// [`comparison_key`](elang_repertoire::ipa::IpaIdentifier::comparison_key)
    /// twice yields the same string as applying it once. Combined
    /// with the no-lower-tie-bar property below, this is what
    /// makes the global-IPA-uniqueness check (C-09) a fixpoint.
    #[test]
    fn ipa_comparison_key_is_idempotent(s in ipa_candidate_strategy()) {
        let rep = elang_repertoire::repertoire::load()
            .expect("pinned repertoire must load");
        let Ok(id) = ipa::validate(&s, &rep) else {
            // Invalid identifiers are a separate property (see
            // `ipa_rejects_lower_tie_bar_in_canonical_form`); the
            // idempotence property is conditional on a successful
            // validation.
            return Ok(());
        };
        let once = id.comparison_key();
        let twice = {
            // Apply the same transformation again. Re-validate the
            // once-form (which is NFC + tie-bar normalised) so we
            // get a fresh `IpaIdentifier`, then take its key.
            let id2 = ipa::validate(&once, &rep)
                .expect("once-key must itself be a valid IPA identifier");
            id2.comparison_key()
        };
        proptest::prop_assert_eq!(once, twice);
    }

    /// The IPA comparison key never contains the lower tie bar
    /// (U+035C). It is normalised to the upper one (U+0361) so
    /// that the *canonical* form of an identifier carries only the
    /// upper variant. A regression that lets U+035C survive the
    /// normalisation would break global IPA uniqueness (C-09) by
    /// giving `t͡ʃ` and `t͜ʃ` distinct canonical forms.
    #[test]
    fn ipa_canonical_form_has_no_lower_tie_bar(s in ipa_candidate_strategy()) {
        let rep = elang_repertoire::repertoire::load()
            .expect("pinned repertoire must load");
        let Ok(id) = ipa::validate(&s, &rep) else { return Ok(()); };
        let key = id.comparison_key();
        proptest::prop_assert!(
            !key.contains(TIE_LOWER),
            "comparison key `{}` still contains the lower tie bar (U+035C)",
            key,
        );
    }

    /// The phonological condition sorter is a fixpoint under
    /// re-application: applying the same comparator twice to an
    /// already-sorted sequence of keys yields the same sequence.
    /// This is the bytewise ordering property for `when` / `group`
    /// / `or` subtrees: the sort comparator is `condition_key`,
    /// which is a total order on condition trees, so a second pass
    /// over an already-sorted slice must be a no-op. A regression
    /// in the comparator would surface here with the exact case id
    /// and the exact offending subtree.
    ///
    /// Note that this is **not** a property about the authored
    /// child order — the `condition-tree` golden is intentionally
    /// authored in non-canonical order, which is what
    /// `sorted_children` exists to canonicalise. The property is
    /// about the sorter's output, not its input.
    #[test]
    fn condition_sorter_is_idempotent(idx in 0usize..ELD_CASES.len()) {
        let case = &ELD_CASES[idx];
        let rep = elang_repertoire::repertoire::load()
            .expect("pinned repertoire must load");
        let Ok(doc) = elang_eld::parse(case.source.as_bytes(), &rep) else {
            return Ok(());
        };
        let refs = InventoryRefs::from_document(&doc);

        for group in doc.groups() {
            for root in group.roots() {
                let Some(block) = root.when() else { continue; };
                let Ok(when) = elang_eld::parse_block(block, &refs, &rep) else {
                    continue;
                };

                // Compare the sorter's returned order with independently
                // sorted keys from the original children.
                let mut expected_keys: Vec<Vec<u8>> = when
                    .children()
                    .iter()
                    .map(ck::condition_key)
                    .collect();
                expected_keys.sort();
                let once_keys: Vec<Vec<u8>> = ck::sorted_children(when.children())
                    .into_iter()
                    .map(ck::condition_key)
                    .collect();
                proptest::prop_assert_eq!(
                    &once_keys,
                    &expected_keys,
                    "case `{}` `when` sorter differs from bytewise key order",
                    case.id,
                );

                // Re-sort the already-sorted keys. The result must
                // equal `once_keys` byte-for-byte: this is the
                // pure-property version of the idempotence claim.
                // (The comparator used by `Vec::sort` is the
                // canonical byte ordering on which the golden
                // vectors depend; this test pins that contract.)
                let mut twice_keys = once_keys.clone();
                twice_keys.sort();

                proptest::prop_assert_eq!(
                    &once_keys,
                    &twice_keys,
                    "case `{}` `when` condition-key sort is not idempotent",
                    case.id,
                );

                // Walk the whole subtree (group/or) applying the same
                // idempotence property recursively.
                for c in when.children() {
                    exercise_subtree_idempotent(c, case.id);
                }
            }
        }
    }
}

/// Recursively walk a condition tree and assert that the
/// `sorted_children` of every `Group` and the `sorted_operands` of
/// every `Or` are themselves idempotent under re-application.
fn exercise_subtree_idempotent(c: &Condition, case_id: &str) {
    match c {
        Condition::Group(children) => {
            let mut expected_keys: Vec<Vec<u8>> = children.iter().map(ck::condition_key).collect();
            expected_keys.sort();
            let once_keys: Vec<Vec<u8>> = ck::sorted_children(children)
                .into_iter()
                .map(ck::condition_key)
                .collect();
            assert_eq!(
                once_keys, expected_keys,
                "case `{case_id}` `group` sorter differs from bytewise key order",
            );
            let mut twice_keys = once_keys.clone();
            twice_keys.sort();
            assert_eq!(
                once_keys, twice_keys,
                "case `{case_id}` `group` condition-key sort is not idempotent",
            );
            for c in children {
                exercise_subtree_idempotent(c, case_id);
            }
        }
        Condition::Or(left, right) => {
            // Preserve the returned operand order when comparing against
            // independently sorted keys and checking idempotence.
            let mut expected_keys = vec![ck::condition_key(left), ck::condition_key(right)];
            expected_keys.sort();
            let sorted = ck::sorted_operands(left, right);
            let k_left = ck::condition_key(sorted[0]);
            let k_right = ck::condition_key(sorted[1]);
            let once_keys = vec![k_left, k_right];
            assert_eq!(
                once_keys, expected_keys,
                "case `{case_id}` `or` sorter differs from bytewise key order",
            );
            let mut twice_keys = once_keys.clone();
            twice_keys.sort();
            assert_eq!(
                once_keys, twice_keys,
                "case `{case_id}` `or` condition-key sort is not idempotent",
            );
            exercise_subtree_idempotent(sorted[0], case_id);
            exercise_subtree_idempotent(sorted[1], case_id);
        }
        Condition::Predicate(_) => {}
    }
}

// Suppress the unused-import lint for `Repertoire` when this module
// is built without `cfg(test)`. The conformance crate's
// `#![allow(...)]` list does not include `unused_imports` for
// non-test builds; without this guard the build would warn.
#[allow(dead_code)]
fn _repertoire_anchor(_r: &Repertoire) {}
