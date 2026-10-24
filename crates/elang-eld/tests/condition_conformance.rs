//! Integration tests for Phase 10 — the phonological condition language
//! (T1029, T1030, REQ-WHEN-001..015).
//!
//! These exercise the parser and evaluator through the public API:
//! [`parse_document`] builds the validated model, then the per-block
//! parser runs at stage 12 and surfaces its diagnostics. The evaluator
//! is invoked separately against representative contexts.
//!
//! The companion unit tests in `src/when.rs` cover the parser internals
//! and isolated evaluator branches; this file covers composed
//! scenarios and the acceptance-criteria truth-table vectors from
//! Issue #20.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "integration tests assert exact codes/behaviour; a panic signals a regression"
)]

use elang_core::model::when::{SyllablePosition, WordPosition};
use elang_eld::{
    eligibility_for_root, evaluate, max_nesting_depth, parse, parse_block, Category, Eligibility,
    EvalContext, InventoryRefs, MAX_NESTING_DEPTH,
};
use elang_repertoire::repertoire;

fn rep() -> elang_repertoire::repertoire::Repertoire {
    repertoire::load().expect("repertoire must load")
}

fn parse_ok(src: &str) -> elang_eld::EldDocument {
    let r = rep();
    parse(src.as_bytes(), &r).expect("parse should succeed")
}

fn parse_err(src: &str) -> elang_eld::Error {
    let r = rep();
    parse(src.as_bytes(), &r).expect_err("parse should fail")
}

/// Build an [`InventoryRefs`] whose root keys are exactly the given
/// `(ipa, roman)` pairs. The roman values must be unique (the
/// romanization uniqueness rule from Phase 9). This exercises the
/// real reference-collection path.
fn with_keys(pairs: &[(&str, &str)]) -> InventoryRefs {
    let mut body = String::from("phonemes {\n");
    body.push_str("  consonants {\n");
    for (k, r) in pairs {
        body.push_str(&format!("    phoneme \"{k}\" {{ roman \"{r}\" }}\n"));
    }
    body.push_str("  }\n");
    // Pick a default vowel IPA that the caller didn't declare, so we never
    // collide with the caller's keys.
    let candidates = ["i", "u", "e", "o"];
    let vowel = candidates
        .iter()
        .find(|c| !pairs.iter().any(|(k, _)| k == *c))
        .copied()
        .unwrap_or("a");
    body.push_str(&format!(
        "  vowels {{ phoneme \"{vowel}\" {{ roman \"vw\" }} }}\n"
    ));
    body.push_str("}\n");
    let doc = parse_ok(&body);
    InventoryRefs::from_document(&doc)
}

fn when_of_root(doc: &elang_eld::EldDocument) -> &elang_eld::WhenBlock {
    doc.groups()
        .first()
        .and_then(|g| g.roots().first())
        .and_then(|r| r.when())
        .expect("when block present")
}

// ===========================================================================
// Condition parsing (T1001..T1007)
// ===========================================================================

#[test]
fn valid_when_block_passes_stage_12() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                word-position \"initial\"\n\
                previous-phoneme \"a\"\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src);
}

#[test]
fn empty_when_block_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {
            roman \"p\"
            when { }
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_EMPTY");
    assert_eq!(e.category(), Category::When);
}

#[test]
fn two_when_blocks_on_one_root_are_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when { word-position \"initial\" }\n\
            when { word-position \"final\" }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "ELD_PHONEME_INVALID");
}

#[test]
fn or_with_one_child_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                or { word-position \"initial\" }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_BAD_ARITY");
}

#[test]
fn group_with_one_child_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                group { word-position \"initial\" }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_BAD_ARITY");
}

#[test]
fn explicit_and_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                and {\n\
                    word-position \"initial\"\n\
                    word-position \"final\"\n\
                }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_UNKNOWN_NODE");
}

#[test]
fn explicit_not_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                not { word-position \"initial\" }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_UNKNOWN_NODE");
}

#[test]
fn unknown_predicate_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { frequency \"high\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_UNKNOWN_NODE");
}

#[test]
fn atomic_with_no_arg_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { word-position } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_PREDICATE_ARITY");
}

#[test]
fn atomic_with_two_args_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { word-position \"initial\" \"medial\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_PREDICATE_ARITY");
}

#[test]
fn atomic_with_property_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { word-position x=\"initial\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_BAD_ARITY");
}

// ===========================================================================
// Predicate validation (T1008..T1010)
// ===========================================================================

#[test]
fn word_position_must_be_known() {
    for bad in ["first", "middle", "end"] {
        let src = format!(
            "phonemes {{\n  consonants {{ phoneme \"p\" {{ roman \"p\"\n      when {{ word-position \"{bad}\" }}\n  }} }}\n  vowels {{ phoneme \"a\" {{ roman \"a\" }} }}\n}}\n"
        );
        let e = parse_err(&src);
        assert_eq!(e.code(), "WHEN_BAD_VALUE", "value {bad:?} should fail");
    }
}

#[test]
fn syllable_position_must_be_known() {
    for bad in ["head", "tail", "rhyme"] {
        let src = format!(
            "phonemes {{\n  consonants {{ phoneme \"p\" {{ roman \"p\"\n      when {{ syllable-position \"{bad}\" }}\n  }} }}\n  vowels {{ phoneme \"a\" {{ roman \"a\" }} }}\n}}\n"
        );
        let e = parse_err(&src);
        assert_eq!(e.code(), "WHEN_BAD_VALUE", "value {bad:?} should fail");
    }
}

#[test]
fn neighbour_ipa_must_be_in_repertoire() {
    // `\u{E000}` (Private-Use Area) is never in the IPA allowlist,
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { previous-phoneme \"\\u{E000}\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_ne!(e.category(), Category::When, "IPA validity is Phase 8's");
}

// ===========================================================================
// Budgets (T1011, T1012)
// ===========================================================================

#[test]
fn seventeen_atomic_predicates_is_rejected() {
    let mut body =
        String::from("phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when {\n");
    for _ in 0..16 {
        body.push_str("      word-position \"initial\"\n");
    }
    body.push_str("      word-position \"final\"\n");
    body.push_str("    } } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n");
    let e = parse_err(&body);
    assert_eq!(e.code(), "WHEN_TOO_MANY_PREDICATES");
}

#[test]
fn sixteen_atomic_predicates_is_accepted() {
    let mut body =
        String::from("phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when {\n");
    for _ in 0..15 {
        body.push_str("      word-position \"initial\"\n");
    }
    body.push_str("      word-position \"final\"\n");
    body.push_str("    } } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n");
    parse_ok(&body);
}

#[test]
fn nine_nested_levels_is_rejected() {
    let mut body = String::from(
        "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when {\n      word-position \"initial\"\n      group {\n",
    );
    // 6 more nested `group` levels → outer + 6 = 7 groups + `or` at
    // depth 9; the word-position children of `or` reach depth 9 and
    // exceed the budget of 8.
    for _ in 0..6 {
        body.push_str("      word-position \"initial\"\n      group {\n");
    }
    // Innermost (level 9) gets two siblings so it passes arity; the
    // depth budget fires before any evaluator decision is made.
    body.push_str("      word-position \"initial\"\n      or {\n        word-position \"initial\"\n        word-position \"final\"\n      }\n");
    for _ in 0..7 {
        body.push_str("      }\n");
    }
    body.push_str("    } } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n");
    let e = parse_err(&body);
    assert_eq!(e.code(), "WHEN_TOO_DEEP", "depth 9 must trip the budget");
}

#[test]
fn eight_nested_levels_is_accepted() {
    let mut body = String::from(
        "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when {\n      word-position \"initial\"\n      group {\n",
    );
    // 5 more nested `group` levels. Outer `group` + 5 loop iterations = 6
    // groups; combined with the innermost `or`, the deepest leaf (a
    // word-position inside `or`) sits at depth 8, exactly meeting the
    // budget without exceeding it.
    for _ in 0..5 {
        body.push_str("      word-position \"initial\"\n      group {\n");
    }
    // Innermost group gets two siblings so it passes arity.
    body.push_str("      word-position \"initial\"\n      or {\n        word-position \"initial\"\n        word-position \"final\"\n      }\n");
    for _ in 0..6 {
        body.push_str("      }\n");
    }
    body.push_str("    } } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n");
    let doc = parse_ok(&body);
    // Introspect the parsed AST to confirm the deepest path really is 8,
    // not coincidentally less (which would mean the test exercises the
    // budget at the wrong point). Phase 10 validates but does not store
    // the AST, so we call `parse_block` ourselves on the same block.
    let block = when_of_root(&doc);
    let ast =
        parse_block(block, &InventoryRefs::from_document(&doc), &rep()).expect("block parses");
    // Top-level child of `when` is the leading word-position predicate;
    // the deeply-nested `group`/`or` chain is the second child.
    let cond = ast
        .children()
        .get(1)
        .expect("when has the deeply-nested group as its second child");
    let depth = max_nesting_depth(cond);
    assert_eq!(depth, 8, "deepest path should be exactly 8 levels deep");
    assert_eq!(
        elang_eld::MAX_NESTING_DEPTH,
        8,
        "budget constant must be 8 for this assertion to be meaningful"
    );
}

#[test]
fn max_nesting_depth_constant_is_eight() {
    // Pin the budget itself: this is the value the boundary tests above
    // rely on. If anyone bumps it without also revisiting the conformance
    // tests, this assertion breaks first and forces a deliberate update.
    assert_eq!(MAX_NESTING_DEPTH, 8);
}

// ===========================================================================
// Reference resolution (T1013..T1016)
// ===========================================================================

#[test]
fn forward_reference_resolves() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { previous-phoneme \"a\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src);
}

#[test]
fn self_reference_resolves() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { previous-phoneme \"p\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src);
}

#[test]
fn cross_group_reference_resolves() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { next-phoneme \"a\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src);
}

#[test]
fn allophone_reference_resolves_to_owning_root() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            allophone \"p\\u{02B0}\" {\n\
                roman \"ph\"\n\
                when { previous-phoneme \"a\" }\n\
            } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src);
}

#[test]
fn missing_reference_is_rejected() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { previous-phoneme \"x\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let e = parse_err(src);
    assert_eq!(e.code(), "WHEN_REFERENCE_UNRESOLVED");
}

// ===========================================================================
// Evaluator truth table (T1020..T1025, T1029)
// ===========================================================================

#[test]
fn truth_table_word_position() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { word-position \"initial\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");

    let three = vec!["p".to_string(), "a".to_string(), "p".to_string()];
    let one = vec!["p".to_string()];
    let two = vec!["p".to_string(), "a".to_string()];

    let eval_at = |idx, roots: &[String]| -> bool {
        evaluate(
            &when,
            &EvalContext {
                roots,
                index: idx,
                syllable_role: None,
                refs: &refs,
            },
        )
        .unwrap()
    };
    assert!(eval_at(0, &three), "index 0 in 3-phoneme is initial");
    assert!(!eval_at(1, &three), "index 1 in 3-phoneme is not initial");
    assert!(!eval_at(2, &three), "index 2 in 3-phoneme is not initial");
    assert!(eval_at(0, &one), "single-phoneme is initial");
    assert!(eval_at(0, &two), "first of 2-phoneme is initial");
    assert!(!eval_at(1, &two), "last of 2-phoneme is not initial");
}

#[test]
fn truth_table_single_phoneme_is_both_initial_and_final() {
    // A single-phoneme root is *both* initial and final simultaneously
    // (Medial is impossible with one phoneme), so the conjunction
    // `initial ∧ final` holds.
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                word-position \"initial\"\n\
                word-position \"final\"\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    let one = vec!["p".to_string()];
    let ctx = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    assert!(evaluate(&when, &ctx).unwrap());
}

#[test]
fn truth_table_syllable_position() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { syllable-position \"onset\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    let roots = vec!["p".to_string()];

    let ctx_onset = EvalContext {
        roots: &roots,
        index: 0,
        syllable_role: Some(SyllablePosition::Onset),
        refs: &refs,
    };
    let ctx_nucleus = EvalContext {
        syllable_role: Some(SyllablePosition::Nucleus),
        roots: &roots,
        index: 0,
        refs: &refs,
    };
    let ctx_none = EvalContext {
        roots: &roots,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    assert!(evaluate(&when, &ctx_onset).unwrap(), "onset matches");
    assert!(!evaluate(&when, &ctx_nucleus).unwrap(), "nucleus doesn't");
    assert!(!evaluate(&when, &ctx_none).unwrap(), "no role doesn't");
}

#[test]
fn truth_table_neighbours_across_syllable_boundary() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when {\n                previous-phoneme \"a\"\n                next-phoneme \"a\"\n            } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp"), ("a", "va")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    let roots = vec!["a".to_string(), "p".to_string(), "a".to_string()];
    let ctx = EvalContext {
        roots: &roots,
        index: 1,
        syllable_role: None,
        refs: &refs,
    };
    assert!(evaluate(&when, &ctx).unwrap());
}

#[test]
fn truth_table_missing_neighbour_is_false() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { previous-phoneme \"a\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp"), ("a", "va")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    let one = vec!["p".to_string()];
    let ctx = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    assert!(!evaluate(&when, &ctx).unwrap());
}

#[test]
fn truth_table_repeated_and_contradictory_predicates_kept_verbatim() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when {\n\
                word-position \"initial\"\n\
                word-position \"final\"\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    assert_eq!(when.children().len(), 2);

    let one = vec!["p".to_string()];
    let three = vec!["p".to_string(), "a".to_string(), "p".to_string()];
    let ctx_single = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    assert!(evaluate(&when, &ctx_single).unwrap());
    let ctx_multi_first = EvalContext {
        roots: &three,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    assert!(!evaluate(&when, &ctx_multi_first).unwrap());
}

#[test]
fn truth_table_or_with_cross_category_alternatives() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when {\n\
                or {\n\
                    word-position \"initial\"\n\
                    syllable-position \"onset\"\n\
                }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let when = parse_block(when_of_root(&doc), &refs, &rep()).expect("parses");
    let one = vec!["p".to_string()];
    let two = vec!["p".to_string(), "a".to_string()];
    let ctx_initial = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    let ctx_onset = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: Some(SyllablePosition::Onset),
        refs: &refs,
    };
    let ctx_multi_first = EvalContext {
        roots: &two,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    let ctx_multi_medial = EvalContext {
        roots: &two,
        index: 1,
        syllable_role: Some(SyllablePosition::Coda),
        refs: &refs,
    };
    assert!(
        evaluate(&when, &ctx_initial).unwrap(),
        "single-phoneme is initial"
    );
    assert!(evaluate(&when, &ctx_onset).unwrap(), "onset matches");
    assert!(
        evaluate(&when, &ctx_multi_first).unwrap(),
        "first-of-multi is initial"
    );
    assert!(
        !evaluate(&when, &ctx_multi_medial).unwrap(),
        "medial+coda matches neither"
    );
}

// ===========================================================================
// Eligibility semantics (T1026..T1028)
// ===========================================================================

#[test]
fn root_and_allophone_conditions_are_independent() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" {\n\
            roman \"p\"\n\
            when { word-position \"initial\" }\n\
            allophone \"p\\u{02B0}\" { roman \"ph\" }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let root = &doc.groups()[0].roots()[0];

    let one = vec!["p".to_string()];
    let three = vec!["p".to_string(), "a".to_string(), "p".to_string()];
    let ctx_initial = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    let ctx_medial = EvalContext {
        roots: &three,
        index: 1,
        syllable_role: None,
        refs: &refs,
    };

    let elig_initial = eligibility_for_root(root, &ctx_initial, &rep()).expect("evaluate root");
    match elig_initial {
        Eligibility::Realizations(v) => {
            assert!(!v.is_empty());
            assert!(v.iter().any(|r| r.ipa.as_str() == "p"));
            assert!(v.iter().any(|r| r.ipa.as_str() == "p\u{02B0}"));
        }
        Eligibility::Empty => panic!("initial context must produce a realization"),
    }

    let elig_medial = eligibility_for_root(root, &ctx_medial, &rep()).expect("evaluate root");
    match elig_medial {
        Eligibility::Realizations(v) => {
            assert!(v.iter().any(|r| r.ipa.as_str() == "p\u{02B0}"));
            assert!(
                !v.iter().any(|r| r.ipa.as_str() == "p"),
                "root must be excluded: word-position \"initial\" does not hold"
            );
        }
        Eligibility::Empty => panic!("unconditional allophone must keep the set non-empty"),
    }
}

#[test]
fn no_eligible_returns_empty_set_not_fallback() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            when { word-position \"medial\" } } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp")]);
    let root = &doc.groups()[0].roots()[0];
    let one = vec!["p".to_string()];
    let ctx = EvalContext {
        roots: &one,
        index: 0,
        syllable_role: None,
        refs: &refs,
    };
    let elig = eligibility_for_root(root, &ctx, &rep()).expect("evaluate root");
    assert!(matches!(elig, Eligibility::Empty));
}

#[test]
fn evaluator_does_not_use_surface_for_underlying_match() {
    // Phase 8 owns the rejection and we never reach Phase 10's reference
    // resolver.
    let src = "phonemes {\n\
        consonants { phoneme \"p\" { roman \"p\"\n\
            allophone \"p\\u{02B0}\" {\n\
                roman \"ph\"\n\
                when { previous-phoneme \"a\" }\n\
            }\n\
        } }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    let doc = parse_ok(src);
    let refs = with_keys(&[("p", "cp"), ("a", "va")]);
    let root = &doc.groups()[0].roots()[0];

    let seq = vec!["a".to_string(), "p".to_string()];
    let ctx = EvalContext {
        roots: &seq,
        index: 1,
        syllable_role: None,
        refs: &refs,
    };
    let elig = eligibility_for_root(root, &ctx, &rep()).expect("evaluate");
    match elig {
        Eligibility::Realizations(v) => {
            assert!(v.iter().any(|r| r.ipa.as_str() == "p\u{02B0}"));
        }
        Eligibility::Empty => panic!("allophone must be eligible"),
    }
}

// ===========================================================================
// Tie-bar normalisation (T1014/REQ-CANON-006)
// ===========================================================================

#[test]
fn tie_bar_variants_resolve_to_same_root() {
    // Both tie-bar forms (U+0361 upper, U+035C lower) of `tʃ` are valid
    // IPA in the v1 allowlist; declaring one is enough because the
    // comparison key normalises the bar to U+0361.
    let src_upper = "phonemes {\n\
        consonants {\n\
            phoneme \"p\" { roman \"p\"\n\
                when { previous-phoneme \"t\\u{0361}s\" }\n\
            }\n\
            phoneme \"t\\u{0361}s\" { roman \"c\" }\n\
        }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src_upper);

    let src_lower = "phonemes {\n\
        consonants {\n\
            phoneme \"p\" { roman \"p\"\n\
                when { previous-phoneme \"t\\u{035C}s\" }\n\
            }\n\
            phoneme \"t\\u{0361}s\" { roman \"c\" }\n\
        }\n\
        vowels { phoneme \"a\" { roman \"a\" } }\n\
    }";
    parse_ok(src_lower);
}

// ===========================================================================
// Registry codes — spot-check each emitted code from Phase 10.
// ===========================================================================

#[test]
fn registry_codes_are_used() {
    let cases: &[(&str, &str)] = &[
        (
            "WHEN_EMPTY",
            "phonemes {\n  consonants { phoneme \"p\" { roman \"p\"\n    when { }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
        (
            "WHEN_BAD_ARITY",
            "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when { or { word-position \"initial\" } }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
        (
            "WHEN_UNKNOWN_NODE",
            "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when { not { word-position \"initial\" } }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
        (
            "WHEN_BAD_VALUE",
            "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when { word-position \"first\" }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
        (
            "WHEN_PREDICATE_ARITY",
            "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when { word-position }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
        (
            "WHEN_REFERENCE_UNRESOLVED",
            "phonemes {\n  consonants { phoneme \"p\" {\n    roman \"p\"\n    when { previous-phoneme \"x\" }\n  } }\n  vowels { phoneme \"a\" { roman \"a\" } }\n}\n",
        ),
    ];
    for (code, src) in cases {
        let e = parse_err(src);
        assert_eq!(e.code(), *code, "code mismatch for {code}");
        assert_eq!(e.category(), Category::When);
    }
}

// ===========================================================================
// WordPosition re-export sanity
// ===========================================================================

#[test]
fn word_position_variants_reachable() {
    let _ = WordPosition::Initial;
    let _ = WordPosition::Medial;
    let _ = WordPosition::Final;
}
