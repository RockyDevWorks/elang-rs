//! Precomposed IPA regression and boundary tests (REQ-IPA-008, steering §7).

#![allow(clippy::expect_used)]

use elang_repertoire::ipa::{validate, IpaError, Unit, TIE_LOWER, TIE_UPPER};
use elang_repertoire::repertoire::load;
use elang_repertoire::unicode::UnicodeViolation;

fn single_unit(base: char, modifier: char) -> Unit {
    Unit {
        bases: vec![base],
        tie: None,
        modifier: Some(modifier),
    }
}

#[test]
fn adjacent_precomposed_bases_count_as_separate_modified_units() {
    let rep = load().expect("pinned repertoire loads");
    let id = validate("çççç", &rep).expect("four logical units are permitted");
    assert_eq!(id.units, vec![single_unit('c', '\u{0327}'); 4]);
    assert_eq!(id.original, "çççç");
    assert_eq!(id.comparison_key(), "çççç");
    assert_eq!(validate("ççççç", &rep), Err(IpaError::TooManyUnits(5)));
}

#[test]
fn scalar_limit_uses_authored_length_even_with_twelve_decomposed_scalars() {
    let rep = load().expect("pinned repertoire loads");
    // Eight authored scalars expand to twelve, across four logical units.
    let input = "t\u{035C}çt\u{0361}ççç";
    assert_eq!(input.chars().count(), 8);
    let id = validate(input, &rep).expect("authored scalar count is within the limit");
    assert_eq!(id.original, input);
    assert_eq!(
        id.units,
        vec![
            Unit {
                bases: vec!['t', 'c'],
                tie: Some(TIE_LOWER),
                modifier: Some('\u{0327}'),
            },
            Unit {
                bases: vec!['t', 'c'],
                tie: Some(TIE_UPPER),
                modifier: Some('\u{0327}'),
            },
            single_unit('c', '\u{0327}'),
            single_unit('c', '\u{0327}'),
        ]
    );
    // Only three units, but nine authored scalars: the independent scalar
    // ceiling must still reject this and report nine, not twelve.
    assert_eq!(
        validate("t\u{035C}çt\u{0361}çt\u{035C}ç", &rep),
        Err(IpaError::TooManyScalars(9))
    );
}

#[test]
fn implicit_modifications_do_not_consume_an_adjacent_units_modifier() {
    let rep = load().expect("pinned repertoire loads");
    for (input, expected) in [
        (
            "çtʰ",
            vec![single_unit('c', '\u{0327}'), single_unit('t', 'ʰ')],
        ),
        (
            "tʰç",
            vec![single_unit('t', 'ʰ'), single_unit('c', '\u{0327}')],
        ),
    ] {
        let id = validate(input, &rep).expect("each unit may have its own modification");
        assert_eq!(id.units, expected, "{input:?}");
        assert_eq!(id.original, input);
    }
}

#[test]
fn every_approved_modifier_is_extra_after_a_precomposed_base() {
    let rep = load().expect("pinned repertoire loads");
    for entry in rep
        .combining_diacritics
        .iter()
        .chain(&rep.spacing_modifiers)
    {
        for stem in ["ç", "t\u{0361}ç", "t\u{035C}ç"] {
            let input = format!("{stem}{}", entry.symbol);
            assert_eq!(
                validate(&input, &rep),
                Err(IpaError::TooManyModifications),
                "implicit cedilla already uses the modification in {input:?}"
            );
        }
    }
}

#[test]
fn implicit_cedilla_never_approves_authored_cedillas() {
    let rep = load().expect("pinned repertoire loads");
    for input in [
        "\u{0327}",
        "\u{0327}ç",
        "p\u{0327}",
        "ç\u{0327}",
        "t\u{035C}çp\u{0327}",
    ] {
        let err = validate(input, &rep).expect_err("authored cedilla is not allowlisted");
        assert_eq!(err, IpaError::BaseNotApproved('\u{0327}'), "{input:?}");
        assert_eq!(err.code(), "IPA_NOT_IN_REPERTOIRE");
    }
}

#[test]
fn nfc_validation_and_original_scalar_allowlist_precede_decomposition() {
    let rep = load().expect("pinned repertoire loads");
    for input in ["c\u{0327}", "t\u{0361}c\u{0327}", "ç\u{0301}"] {
        assert_eq!(
            validate(input, &rep),
            Err(IpaError::Unicode(UnicodeViolation::NotNfc)),
            "non-NFC input must not be silently normalized: {input:?}"
        );
    }
    // These NFC forms decompose to IPA-like components, but the authored
    // scalar itself is not approved. Decomposition cannot grant approval.
    for scalar in ['ã', 'ñ', 'ä', '\u{1E09}', '\u{FB03}'] {
        assert_eq!(
            validate(&scalar.to_string(), &rep),
            Err(IpaError::BaseNotApproved(scalar)),
            "unapproved precomposed/compatibility scalar {scalar:?}"
        );
    }
}

#[test]
fn tied_precomposed_keys_preserve_nfc_bytes_and_untied_distinction() {
    let rep = load().expect("pinned repertoire loads");
    let upper = validate("t\u{0361}ç", &rep).expect("upper tie");
    let lower = validate("t\u{035C}ç", &rep).expect("lower tie");
    let untied = validate("tç", &rep).expect("untied bases");
    assert_eq!(upper.original, "t\u{0361}ç");
    assert_eq!(lower.original, "t\u{035C}ç");
    assert_eq!(upper.comparison_key(), "t\u{0361}ç");
    assert_eq!(lower.comparison_key(), upper.comparison_key());
    assert_ne!(untied.comparison_key(), upper.comparison_key());
    assert_eq!(untied.units.len(), 2);
}

#[test]
fn precomposed_ties_reject_missing_bases_intervening_modifiers_and_chaining() {
    let rep = load().expect("pinned repertoire loads");
    for tie in [TIE_UPPER, TIE_LOWER] {
        for input in [
            format!("ç{tie}"),
            format!("ç{tie}ç"),
            format!("t{tie}ç{tie}t"),
            format!("t{tie}ʰç"),
            format!("tʰ{tie}ç"),
            format!("{tie}ç"),
        ] {
            assert_eq!(
                validate(&input, &rep),
                Err(IpaError::TieBarInvalid),
                "{input:?}"
            );
        }
    }
}
