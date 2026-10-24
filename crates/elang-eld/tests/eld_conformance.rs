//! Integration tests for `elang-eld` (T0929, T0930).
//!
//! These exercise the public API exactly as the reader SDK will: parse with
//! the repertoire → validate → model. The unit tests in `src/parse.rs` cover
//! the grammar in isolation; this file covers composed scenarios, boundary
//! limits, and the acceptance criteria from Issue #19.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "integration tests assert exact codes/behaviour; a panic signals a regression"
)]

use elang_core::error::Category;
use elang_eld::{parse, EldDocument, Error};
use elang_repertoire::repertoire;

fn rep() -> elang_repertoire::repertoire::Repertoire {
    repertoire::load().expect("repertoire must load")
}

fn parse_ok(src: &str) -> EldDocument {
    let r = rep();
    parse(src.as_bytes(), &r).expect("parse should succeed")
}

fn parse_err(src: &str) -> Error {
    let r = rep();
    parse(src.as_bytes(), &r).expect_err("parse should fail")
}

// ---- Golden vector (steering §6 example, adapted) -----------------------

#[test]
fn steering_section_6_example() {
    let src = "\
phonemes {
    consonants {
        phoneme \"t\" {
            roman \"t\"
            allophone \"ɾ\"
            allophone \"tʰ\" {
                roman \"th\"
                when {
                    word-position \"initial\"
                }
            }
        }
    }
    vowels {
        phoneme \"a\" {
            roman \"a\"
        }
    }
    diphthongs {
        phoneme \"aɪ\" {
            roman \"ai\"
        }
    }
}
";
    let doc = parse_ok(src);
    assert_eq!(doc.groups().len(), 3);
    let cons = &doc.groups()[0];
    assert_eq!(cons.kind().name(), "consonants");
    assert_eq!(cons.roots().len(), 1);
    let root_t = &cons.roots()[0];
    assert_eq!(root_t.ipa().as_str(), "t");
    assert_eq!(root_t.roman().as_str(), "t");
    assert_eq!(root_t.allophones().len(), 2);
    assert!(root_t.allophones()[0].is_bare());
    assert_eq!(root_t.allophones()[0].ipa().as_str(), "ɾ");
    assert_eq!(root_t.allophones()[1].ipa().as_str(), "tʰ");
    assert_eq!(root_t.allophones()[1].effective_roman().as_str(), "th");
    assert!(root_t.allophones()[1].when().is_some());

    let vowels = &doc.groups()[1];
    assert_eq!(vowels.kind().name(), "vowels");
    assert_eq!(vowels.roots()[0].ipa().as_str(), "a");

    let diphthongs = &doc.groups()[2];
    assert_eq!(diphthongs.kind().name(), "diphthongs");
    assert_eq!(diphthongs.roots()[0].ipa().as_str(), "aɪ");
}

// ---- Romanization case-insensitive collision with inherited exempt -----

#[test]
fn inherited_romans_do_not_cause_duplicate() {
    // C-15/C-16: two allophones inheriting the same parent roman are fine.
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"th\"\n\
             allophone \"ɾ\"\n\
             allophone \"ʔ\"\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let root = &doc.groups()[0].roots()[0];
    assert_eq!(root.allophones().len(), 2);
    assert_eq!(root.allophones()[0].effective_roman().as_str(), "th");
    assert_eq!(root.allophones()[1].effective_roman().as_str(), "th");
}

// ---- Explicit only duplicates collide ----------------------------------

#[test]
fn explicit_roman_collision_root_to_allophone() {
    // Same case-insensitive key across a root and an allophone.
    let e = parse_err(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"th\"\n\
             allophone \"d\" { roman \"TH\" }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ROMAN_DUPLICATE");
    assert_eq!(e.category(), Category::Roman);
}

#[test]
fn ipa_duplicate_points_at_allophone() {
    // Cover collisions with the root and a preceding allophone's tie-bar alias.
    for (first, duplicate) in [("d", "t"), ("t͡ʃ", "t͜ʃ")] {
        let src = format!(
            r#"phonemes {{
    consonants {{
        phoneme "t" {{
            roman "t"
            allophone "{first}"
            allophone "{duplicate}"
        }}
    }}
    vowels {{ phoneme "a" {{ roman "a" }} }}
}}"#
        );
        let e = parse_err(&src);
        assert_eq!(e.code(), "ELD_IPA_DUPLICATE");
        let span = e.diagnostic.span.expect("duplicate must have a span");
        assert_eq!((span.line, span.column), (6, 13));
    }
}

#[test]
fn roman_duplicate_points_at_allophone() {
    // Cover collisions with the root and a preceding explicit allophone roman.
    for duplicate in ["T", "D"] {
        let src = format!(
            r#"phonemes {{
    consonants {{
        phoneme "t" {{
            roman "t"
            allophone "d" {{ roman "d" }}
            allophone "b" {{ roman "{duplicate}" }}
        }}
    }}
    vowels {{ phoneme "a" {{ roman "a" }} }}
}}"#
        );
        let e = parse_err(&src);
        assert_eq!(e.code(), "ROMAN_DUPLICATE");
        assert_eq!(e.category(), Category::Roman);
        let span = e.diagnostic.span.expect("duplicate must have a span");
        assert_eq!((span.line, span.column), (6, 13));
    }
}

// ---- Allophone `when` independence test (REQ-ELD-017) --------------------

#[test]
fn allophone_when_independent_no_inheritance() {
    // Root has a `when`, allophone has no `when` → allophone is unconditional.
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"t\"\n\
             when { word-position \"initial\" }\n\
             allophone \"d\" { roman \"d\" }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let root = &doc.groups()[0].roots()[0];
    assert!(root.when().is_some());
    assert!(root.allophones()[0].when().is_none());
}

// ---- T0927: overlapping eligibility is valid, no priority ----------------

#[test]
fn overlapping_when_conditions_are_valid() {
    // Two allophones with the same `when` → both retained (no priority).
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"t\"\n\
             allophone \"d\" { when { word-position \"initial\" } }\n\
             allophone \"d̥\" { when { word-position \"initial\" } }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(doc.groups()[0].roots()[0].allophones().len(), 2);
}

// ---- T0928: none eligible does not invalidate ----------------------------

#[test]
fn condition_makes_no_eligible_but_definition_remains_valid() {
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"t\"\n\
             allophone \"d\" { when { word-position \"medial\" } }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert!(doc.groups()[0].roots()[0].allophones()[0].when().is_some());
}

// ---- WhenBlock node/children accessors ----------------------------------

#[test]
fn when_block_accessors() {
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"p\" {\n\
             roman \"p\"\n\
             when {\n\
                 word-position \"initial\"\n\
                 syllable-position \"onset\"\n\
             }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let wb = doc.groups()[0].roots()[0].when().unwrap();
    assert_eq!(wb.node().name(), "when");
    assert_eq!(wb.children().len(), 2);
    assert_eq!(wb.children()[0].name(), "word-position");
    assert_eq!(wb.children()[1].name(), "syllable-position");
}

// ---- Allophone effective_roman inheritance preservation -----------------

#[test]
fn roman_inheritance_preserves_root_case() {
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"Th\"\n\
             allophone \"d\"\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let root = &doc.groups()[0].roots()[0];
    assert_eq!(root.roman().as_str(), "Th");
    assert_eq!(root.allophones()[0].effective_roman().as_str(), "Th");
}

// ---- 256/257 roots boundary ----------------------------------------------

fn inventory_with_consonant_roots(count: usize) -> String {
    // Eight approved IPA bases give 512 unique three-unit identifiers.
    // They also serve as unique, valid ASCII romanizations.
    let bases = ['p', 'b', 't', 'd', 'k', 's', 'm', 'n'];
    let identifiers = bases.into_iter().flat_map(|a| {
        bases
            .into_iter()
            .flat_map(move |b| bases.into_iter().map(move |c| format!("{a}{b}{c}")))
    });
    let mut src = String::from("phonemes {\n    consonants {\n");
    for ipa in identifiers.take(count) {
        src.push_str(&format!(
            "        phoneme \"{ipa}\" {{ roman \"{ipa}\" }}\n"
        ));
    }
    src.push_str("    }\n    vowels { phoneme \"a\" { roman \"a\" } }\n}\n");
    src
}

#[test]
fn two_hundred_fifty_six_roots_accepted() {
    let doc = parse_ok(&inventory_with_consonant_roots(255));
    assert_eq!(doc.groups()[0].roots().len(), 255);
    assert_eq!(doc.groups()[1].roots().len(), 1);
}

#[test]
fn two_hundred_fifty_seven_roots_rejected() {
    let e = parse_err(&inventory_with_consonant_roots(256));
    assert_eq!(e.code(), "ELD_PHONEME_INVALID");
    assert!(e.diagnostic.message.contains("too many root phonemes"));
}

// ---- 17 groups boundary --------------------------------------------------

#[test]
fn seventeen_groups() {
    let mut body = String::from(
        "consonants { phoneme \"p\" { roman \"p\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n",
    );
    for i in 0..15 {
        let name = format!("g{i}");
        let ipa = format!("b{i}");
        let roman = format!("x{i}");
        body.push_str(&format!(
            "{name} {{ phoneme \"{ipa}\" {{ roman \"{roman}\" }} }}\n"
        ));
    }
    let src = format!("phonemes {{ {} }}", body);
    let e = parse_err(&src);
    assert_eq!(e.code(), "ELD_GROUP_INVALID");
}

// ---- Allophone with only `when` (no roman) is not bare -------------------

#[test]
fn allophone_with_only_when_is_not_bare() {
    let doc = parse_ok(
        "phonemes {\n\
         consonants { phoneme \"t\" {\n\
             roman \"t\"\n\
             allophone \"d\" { when { word-position \"initial\" } }\n\
         } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let allo = &doc.groups()[0].roots()[0].allophones()[0];
    assert!(!allo.is_bare());
    assert!(allo.explicit_roman().is_none());
    assert!(allo.when().is_some());
}

// ---- Type annotation everywhere -----------------------------------------

#[test]
fn type_annotation_on_phonemes_rejected() {
    let e = parse_err(
        "(t) phonemes {\n\
         consonants { phoneme \"p\" { roman \"p\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ELD_TOPLEVEL_INVALID");
}

#[test]
fn type_annotation_on_group_rejected() {
    let e = parse_err(
        "phonemes {\n\
         (t) consonants { phoneme \"p\" { roman \"p\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ELD_GROUP_INVALID");
}

#[test]
fn type_annotation_on_phoneme_node_rejected() {
    let e = parse_err(
        "phonemes {\n\
         consonants { (t) phoneme \"p\" { roman \"p\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ELD_PHONEME_INVALID");
}

#[test]
fn type_annotation_on_allophone_rejected() {
    // The type annotation is caught by the phoneme-level child check
    // (phoneme child with type annotation) before the allophone validator runs.
    let e = parse_err(
        "phonemes {\n\
         consonants { phoneme \"p\" { roman \"p\"\n\
             (t) allophone \"b\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ELD_PHONEME_INVALID");
}

#[test]
fn type_annotation_on_roman_rejected() {
    // The type annotation is caught by the phoneme-level child check
    // before the roman validator runs.
    let e = parse_err(
        "phonemes {\n\
         consonants { phoneme \"p\" {\n\
             (t) roman \"p\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "ELD_PHONEME_INVALID");
}

// ---- Repertoire-gated IPA validation ------------------------------------

#[test]
fn ipa_long_string_rejected() {
    let e = parse_err(
        "phonemes {\n\
         consonants { phoneme \"aaaaaaaaa\" { roman \"x\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    assert_eq!(e.code(), "IPA_TOO_MANY_SCALARS");
    assert_eq!(e.category(), Category::Ipa);
}

#[test]
fn ipa_unicode_violation_propagates() {
    let e = parse_err(
        "phonemes {\n\
         consonants { phoneme \"\u{0301}\" { roman \"x\" } }\n\
         vowels { phoneme \"a\" { roman \"a\" } }\n\
         }",
    );
    let code = e.code();
    assert!(
        code == "IPA_STANDALONE_MODIFIER"
            || code == "UNICODE_NOT_NFC"
            || code == "IPA_NOT_IN_REPERTOIRE",
        "unexpected code: {code}"
    );
}
