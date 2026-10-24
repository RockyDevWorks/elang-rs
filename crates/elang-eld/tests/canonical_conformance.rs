//! Canonical ELD serialization conformance (Phase 11 — T1101..T1121).
//!
//! Every expectation in this file is written out by hand from the locked
//! profile (steering §9 / §9.1, ADR 0009), so the suite does not merely
//! re-assert what the implementation happens to produce: a change in the
//! emitted bytes fails the test.
//!
//! The published golden files live in `fixtures/valid/canonical/` and are
//! verified by `elang-conformance` (`gen_canonical_goldens`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration tests assert exact canonical bytes; a panic signals a canonicalization regression"
)]

use elang_repertoire::repertoire::{self, Repertoire};

fn rep() -> Repertoire {
    repertoire::load().expect("pinned repertoire must load")
}

/// Parse, build the validated model and serialize it canonically.
fn canonical(source: &str) -> String {
    let rep = rep();
    let doc = elang_eld::parse(source.as_bytes(), &rep).expect("source must be a valid ELD");
    let inventory = elang_eld::inventory::from_document(&doc, &rep).expect("model must build");
    elang_eld::canon::to_string(&inventory)
}

/// Parse and build the validated model without serializing it.
fn model(source: &str) -> elang_core::model::eld::PhonemeInventory {
    let rep = rep();
    let doc = elang_eld::parse(source.as_bytes(), &rep).expect("source must be a valid ELD");
    elang_eld::inventory::from_document(&doc, &rep).expect("model must build")
}

// ---------------------------------------------------------------------------
// Document structure and ordering (T1101..T1104)
// ---------------------------------------------------------------------------

const MINIMAL: &str = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;

#[test]
fn exactly_one_phonemes_root_is_emitted() {
    let out = canonical(MINIMAL);
    assert_eq!(out.matches("phonemes {").count(), 1);
    assert!(out.starts_with("phonemes {\n"));
    assert!(out.ends_with("}\n"));
}

#[test]
fn groups_are_ordered_consonants_vowels_then_custom() {
    // The source declares `vowels`, then `diphthongs`, then `consonants`.
    let source = r#"phonemes {
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    diphthongs {
        phoneme "aɪ" {
            roman "ai"
        }
    }
    consonants {
        phoneme "t" {
            roman "t"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    diphthongs {
        phoneme "aɪ" {
            roman "ai"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn custom_groups_are_sorted_ascii_bytewise() {
    // `diphthongs` sorts before `nasals`; both sort after the reserved pair.
    let source = r#"phonemes {
    nasals {
        phoneme "n" {
            roman "n"
        }
    }
    consonants {
        phoneme "t" {
            roman "t"
        }
    }
    diphthongs {
        phoneme "aɪ" {
            roman "ai"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let consonants = out.find("consonants {").expect("consonants group");
    let vowels = out.find("vowels {").expect("vowels group");
    let diphthongs = out.find("diphthongs {").expect("diphthongs group");
    let nasals = out.find("nasals {").expect("nasals group");
    assert!(consonants < vowels);
    assert!(vowels < diphthongs);
    assert!(diphthongs < nasals);
}

#[test]
fn roots_are_sorted_by_authored_ipa_utf8_bytes() {
    // `k` (0x6B) < `t` (0x74) < `z` (0x7A) bytewise, even though the source
    // declares them in the opposite order.
    let source = r#"phonemes {
    consonants {
        phoneme "z" {
            roman "z"
        }
        phoneme "t" {
            roman "t"
        }
        phoneme "k" {
            roman "k"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "k" {
            roman "k"
        }
        phoneme "t" {
            roman "t"
        }
        phoneme "z" {
            roman "z"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn ordering_is_bytewise_not_codepoint_count() {
    // `tʰ` is "t" + U+02B0 (3 bytes, first byte 0x74) and `ɾ` is U+027E
    // (3 bytes, first byte 0xE6). Bytewise order puts `tʰ` first even
    // though both spellings have the same number of codepoints.
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "ɾ"
            allophone "tʰ" {
                roman "th"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                roman "th"
            }
            allophone "ɾ"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn ordering_uses_authored_bytes_not_the_tie_bar_folded_key() {
    // ADR 0009 E-1: ordering uses the **authored** NFC bytes, never the
    // tie-bar-folded comparison key.
    //
    // `t͜ʃ` (lower tie bar) is authored as 74 CC 81 C8 83 and `t͡a` (upper
    // tie bar) as 74 CD 81 61. Authored bytewise order puts `t͜ʃ` first
    // (0xCC < 0xCD at index 1). Folding the lower bar onto the upper one
    // would compare 74 CD 81 C8 83 against 74 CD 81 61 and reverse the
    // order. The two identifiers tie *different* units, so they are not
    // tie-bar equivalents and both may be declared.
    let source = r#"phonemes {
    consonants {
        phoneme "t͡a" {
            roman "TYA"
        }
        phoneme "t͜ʃ" {
            roman "TCH"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let tied_sh = out.find("t͜ʃ").expect("authored lower tie bar is emitted");
    let tied_a = out.find("t͡a").expect("authored upper tie bar is emitted");
    assert!(
        tied_sh < tied_a,
        "authored-byte order must place `t͜ʃ` before `t͡a`"
    );
}

// ---------------------------------------------------------------------------
// Child ordering and bare leaves (T1105..T1107)
// ---------------------------------------------------------------------------

#[test]
fn root_children_are_roman_then_when_then_allophones() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            allophone "ɾ"
            when {
                word-position "initial"
            }
            roman "t"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
            }
            allophone "ɾ"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn allophone_children_are_roman_then_when() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                when {
                    word-position "initial"
                }
                roman "th"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                roman "th"
                when {
                    word-position "initial"
                }
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn fieldless_allophone_is_a_bare_leaf() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "ɾ"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert!(out.contains("allophone \"ɾ\"\n"), "bare leaf expected");
    assert!(
        !out.contains("allophone \"ɾ\" {"),
        "a fieldless allophone must never be emitted with braces"
    );
}

#[test]
fn an_allophone_without_a_roman_inherits_and_emits_no_roman_line() {
    // C-16: an omitted `roman` inherits the parent's value. Inheritance is
    // implicit, so the canonical form carries no `roman` line — which is
    // what makes the canonical form a fixpoint.
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                when {
                    word-position "initial"
                }
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    let expected = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                when {
                    word-position "initial"
                }
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

// ---------------------------------------------------------------------------
// Condition ordering (T1108..T1111)
// ---------------------------------------------------------------------------

const CONDITIONS: &str = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                next-phoneme "a"
                word-position "initial"
                group {
                    syllable-position "coda"
                    previous-phoneme "s"
                }
                or {
                    next-phoneme "k"
                    previous-phoneme "z"
                }
            }
        }
        phoneme "s" {
            roman "s"
        }
        phoneme "k" {
            roman "k"
        }
        phoneme "z" {
            roman "z"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;

#[test]
fn condition_children_are_sorted_by_the_canonical_key() {
    let out = canonical(CONDITIONS);
    let expected = r#"phonemes {
    consonants {
        phoneme "k" {
            roman "k"
        }
        phoneme "s" {
            roman "s"
        }
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
                next-phoneme "a"
                group {
                    syllable-position "coda"
                    previous-phoneme "s"
                }
                or {
                    previous-phoneme "z"
                    next-phoneme "k"
                }
            }
        }
        phoneme "z" {
            roman "z"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(out, expected);
}

#[test]
fn condition_key_ranks_are_the_frozen_ones() {
    use elang_core::model::when::{Condition, Predicate, WordPosition};
    use elang_eld::canon::key;
    assert_eq!(key::RANK_WORD_POSITION, 0);
    assert_eq!(key::RANK_SYLLABLE_POSITION, 1);
    assert_eq!(key::RANK_PREVIOUS_PHONEME, 2);
    assert_eq!(key::RANK_NEXT_PHONEME, 3);
    assert_eq!(key::RANK_GROUP, 4);
    assert_eq!(key::RANK_OR, 5);
    let key = key::condition_key(&Condition::predicate(Predicate::WordPosition(
        WordPosition::Initial,
    )));
    assert_eq!(key, vec![0, 0, 7, b'i', b'n', b'i', b't', b'i', b'a', b'l']);
}

// ---------------------------------------------------------------------------
// Authored-value and semantic preservation (T1112, T1113, T1117)
// ---------------------------------------------------------------------------

#[test]
fn duplicate_predicates_are_preserved() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
                word-position "initial"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert_eq!(
        out.matches("word-position \"initial\"").count(),
        2,
        "duplicate predicates must not be collapsed"
    );
}

#[test]
fn contradictory_conditions_are_preserved() {
    // `word-position "initial"` and `word-position "final"` contradict each
    // other for a multi-phoneme word; the writer must not simplify them.
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
                word-position "final"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert!(out.contains("word-position \"final\""));
    assert!(out.contains("word-position \"initial\""));
    assert_eq!(out.matches("word-position").count(), 2);
}

#[test]
fn boolean_tree_nesting_is_preserved() {
    // A flat `group` and a nested `group` are different trees and must stay
    // different — the writer performs no flattening.
    let flat = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                group {
                    word-position "initial"
                    previous-phoneme "s"
                }
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let nested = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                group {
                    group {
                        word-position "initial"
                        previous-phoneme "s"
                    }
                    next-phoneme "a"
                }
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let flat_out = canonical(flat);
    let nested_out = canonical(nested);
    assert_ne!(flat_out, nested_out, "tree shapes must stay distinct");
    // The flat tree has one `group`; the nested tree has two.
    assert_eq!(flat_out.matches("group {").count(), 1);
    assert_eq!(nested_out.matches("group {").count(), 2);
    // The nested `group` is one level deeper: its children sit at 24 spaces
    // instead of 20.
    assert!(flat_out.contains("\n                    word-position \"initial\""));
    assert!(nested_out.contains("\n                        word-position \"initial\""));
}

#[test]
fn or_and_group_with_the_same_children_stay_distinct() {
    let as_group = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                group {
                    word-position "initial"
                    previous-phoneme "s"
                }
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let as_or = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                or {
                    word-position "initial"
                    previous-phoneme "s"
                }
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_ne!(canonical(as_group), canonical(as_or));
}

#[test]
fn authored_ipa_reference_is_preserved_verbatim() {
    // The reference is emitted exactly as authored — it is never rewritten
    // to the owning root's identifier.
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                roman "th"
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    // `previous-phoneme "tʰ"` names the *allophone*, whose owning root is
    // `t`. The canonical output must keep the allophone spelling.
    let with_ref = source.replace(
        "            roman \"t\"\n",
        "            roman \"t\"\n            when {\n                previous-phoneme \"t\u{02B0}\"\n            }\n",
    );
    let out = canonical(&with_ref);
    assert!(out.contains("previous-phoneme \"t\u{02B0}\""));
    assert!(!out.contains("previous-phoneme \"t\""));
}

#[test]
fn tie_bar_variant_is_preserved() {
    // ADR 0009 E-2: the authored tie-bar variant survives in both the
    // declaration and the condition reference.
    let source = r#"phonemes {
    consonants {
        phoneme "t͜ʃ" {
            roman "TCH"
            when {
                previous-phoneme "t͡ʃ"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert!(out.contains("phoneme \"t͜ʃ\""));
    assert!(out.contains("previous-phoneme \"t͡ʃ\""));
}

#[test]
fn romanization_case_is_preserved() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "T"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert!(out.contains("roman \"T\""));
    assert!(!out.contains("roman \"t\""));
}

#[test]
fn permitted_unicode_is_emitted_directly_as_utf8() {
    let source = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "tʰ" {
                roman "th"
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(source);
    assert!(out.contains("allophone \"tʰ\""));
    // U+02B0 is present as its raw UTF-8 bytes (0xCA 0xB0), not as an escape.
    let needle: &[u8] = &[0xCA, 0xB0];
    assert!(out.as_bytes().windows(2).any(|w| w == needle));
    // No `\u{...}` escape form is used for the permitted modifier.
    assert!(!out.contains("\\u{"));
}

// ---------------------------------------------------------------------------
// Canonical formatting (T1114..T1117)
// ---------------------------------------------------------------------------

#[test]
fn indentation_is_exactly_four_spaces_per_level() {
    let out = canonical(CONDITIONS);
    for line in out.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        assert_eq!(indent % 4, 0, "indent of {indent} is not a multiple of 4");
    }
    assert!(out.contains("\n    consonants {"));
    assert!(out.contains("\n        phoneme \"k\" {"));
    assert!(out.contains("\n            roman \"k\""));
    assert!(!out.contains('\t'));
}

#[test]
fn layout_has_no_blank_lines_no_comments_and_one_final_lf() {
    let messy = r#"// A leading comment.
/* Another one. */
phonemes {
    consonants {
        // A comment inside.
        phoneme "t" {
            roman "t" // trailing comment
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let out = canonical(messy);
    assert!(!out.contains("//"));
    assert!(!out.contains("/*"));
    assert!(!out.contains("\n\n"), "no blank lines");
    assert!(!out.contains('\r'), "LF only");
    assert!(out.ends_with("}\n"));
    assert!(!out.ends_with("\n\n"), "exactly one final LF");
}

#[test]
fn one_declaration_per_line() {
    let out = canonical(CONDITIONS);
    for line in out.lines() {
        let trimmed = line.trim();
        // A line holds one node: at most one opening brace, and never a
        // closing brace followed by another declaration.
        assert!(
            trimmed.matches('{').count() <= 1,
            "more than one node on the line: {line}"
        );
        assert!(
            !trimmed.contains("} "),
            "two declarations on one line: {line}"
        );
    }
}

#[test]
fn strings_are_double_quoted() {
    let out = canonical(CONDITIONS);
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("roman ") || trimmed.starts_with("phoneme ") {
            assert!(trimmed.contains('"'), "expected a quoted value: {line}");
        }
    }
}

// ---------------------------------------------------------------------------
// Determinism, idempotence and equivalence (T1118..T1120)
// ---------------------------------------------------------------------------

#[test]
fn canonicalization_is_idempotent() {
    let out = canonical(CONDITIONS);
    let again = canonical(&out);
    assert_eq!(again, out, "canon(canon(x)) must equal canon(x)");
}

#[test]
fn canonicalization_is_idempotent_for_every_fixture_shape() {
    for source in [MINIMAL, CONDITIONS] {
        let once = canonical(source);
        let twice = canonical(&once);
        assert_eq!(twice, once);
        let thrice = canonical(&twice);
        assert_eq!(thrice, once);
    }
}

#[test]
fn canonical_output_reparses_to_the_same_canonical_bytes() {
    // The *model* keeps declaration order, so the rebuilt model is not
    // `==` the original — canonicalization reorders declarations. What must
    // hold is that re-reading the canonical output and re-canonicalizing
    // yields the same bytes (a fixpoint).
    let rep = rep();
    let doc = elang_eld::parse(CONDITIONS.as_bytes(), &rep).expect("valid ELD");
    let inventory = elang_eld::inventory::from_document(&doc, &rep).expect("model");
    let out = elang_eld::canon::to_string(&inventory);
    let reparsed = elang_eld::parse(out.as_bytes(), &rep).expect("canonical output re-parses");
    let rebuilt = elang_eld::inventory::from_document(&reparsed, &rep).expect("model");
    assert_eq!(
        elang_eld::canon::to_string(&rebuilt),
        out,
        "canonical output is a fixpoint"
    );
}

#[test]
fn permitted_declaration_reorderings_produce_identical_bytes() {
    let ordered = r#"phonemes {
    consonants {
        phoneme "k" {
            roman "k"
        }
        phoneme "t" {
            roman "t"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    nasals {
        phoneme "n" {
            roman "n"
        }
    }
}
"#;
    let shuffled = r#"phonemes {
    nasals {
        phoneme "n" {
            roman "n"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    consonants {
        phoneme "t" {
            roman "t"
        }
        phoneme "k" {
            roman "k"
        }
    }
}
"#;
    assert_eq!(canonical(ordered), canonical(shuffled));
}

#[test]
fn permitted_condition_reorderings_produce_identical_bytes() {
    let ordered = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
                previous-phoneme "s"
                next-phoneme "a"
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    let shuffled = r#"phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            when {
                next-phoneme "a"
                previous-phoneme "s"
                word-position "initial"
            }
        }
        phoneme "s" {
            roman "s"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
"#;
    assert_eq!(canonical(ordered), canonical(shuffled));
}

#[test]
fn repeated_serialization_is_byte_identical() {
    let inventory = model(CONDITIONS);
    let first = elang_eld::canon::to_string(&inventory);
    let second = elang_eld::canon::to_string(&inventory);
    assert_eq!(first, second);
    assert_eq!(
        elang_eld::canon::to_bytes(&inventory),
        first.as_bytes(),
        "to_bytes must agree with to_string"
    );
}
