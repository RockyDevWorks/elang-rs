//! Focused tests for the public pinned-UCD scalar decomposition helper.
//!
//! Mapping expectations come from the committed Unicode 18.0.0
//! `UnicodeData.txt`; Hangul expectations follow UAX#15 §3.12.

use elang_unicode::nfc::canonical_decomposition;

#[test]
fn recursive_mappings_expand_fully_in_canonical_order() {
    for (scalar, expected) in [
        ('\u{01FA}', "A\u{030A}\u{0301}"),
        ('\u{1E69}', "s\u{0323}\u{0307}"),
        ('\u{1F82}', "\u{03B1}\u{0313}\u{0300}\u{0345}"),
    ] {
        let actual = canonical_decomposition(scalar);
        assert_eq!(actual, expected.chars().collect::<Vec<_>>(), "{scalar:?}");
        // A full decomposition must have no remaining decomposable scalars.
        let repeated: Vec<char> = actual
            .iter()
            .copied()
            .flat_map(canonical_decomposition)
            .collect();
        assert_eq!(repeated, actual, "decomposition must be idempotent");
    }
}

#[test]
fn canonical_singletons_and_composition_exclusions_are_decomposed() {
    for (scalar, expected) in [
        ('\u{2126}', "\u{03A9}"), // OHM SIGN
        ('\u{212A}', "K"),        // KELVIN SIGN
        ('\u{0340}', "\u{0300}"), // COMBINING GRAVE TONE MARK
        ('\u{0344}', "\u{0308}\u{0301}"),
        ('\u{0958}', "\u{0915}\u{093C}"),
        ('\u{0F73}', "\u{0F71}\u{0F72}"),
        // Despite its name, this ideograph has a canonical mapping, not a
        // tagged compatibility mapping.
        ('\u{F900}', "\u{8C48}"),
        ('\u{1D15E}', "\u{1D157}\u{1D165}"), // supplementary-plane scalars
    ] {
        assert_eq!(
            canonical_decomposition(scalar),
            expected.chars().collect::<Vec<_>>(),
            "U+{:04X}",
            scalar as u32
        );
    }
}

#[test]
fn hangul_decomposition_handles_lv_lvt_and_range_boundaries() {
    for (scalar, expected) in [
        ('\u{ABFF}', "\u{ABFF}"), // immediately before the syllable range
        ('\u{AC00}', "\u{1100}\u{1161}"),
        ('\u{AC01}', "\u{1100}\u{1161}\u{11A8}"),
        ('\u{AC1B}', "\u{1100}\u{1161}\u{11C2}"),
        ('\u{AC1C}', "\u{1100}\u{1162}"), // next vowel, no trailing jamo
        ('\u{AE4C}', "\u{1101}\u{1161}"), // next leading jamo
        ('\u{D7A3}', "\u{1112}\u{1175}\u{11C2}"), // last syllable
        ('\u{D7A4}', "\u{D7A4}"),         // immediately after the syllable range
    ] {
        assert_eq!(
            canonical_decomposition(scalar),
            expected.chars().collect::<Vec<_>>(),
            "U+{:04X}",
            scalar as u32
        );
    }
}

#[test]
fn compatibility_mappings_are_never_applied() {
    for scalar in [
        '\u{00A0}',  // no-break space
        '\u{02B0}',  // modifier letter small h
        '\u{2460}',  // circled digit one
        '\u{FB03}',  // ffi ligature
        '\u{FF43}',  // fullwidth c
        '\u{1D400}', // mathematical bold capital A
    ] {
        assert_eq!(canonical_decomposition(scalar), vec![scalar]);
    }
}

#[test]
fn unmapped_scalars_are_preserved_without_category_validation() {
    // Decomposition is a structural operation, not ELANG string validation.
    // Even controls, private-use scalars and noncharacters must survive it.
    for scalar in [
        '\0',
        't',
        'ə',
        '\u{0303}',
        '\u{1100}',
        '\u{1161}',
        '\u{11A8}',
        '\u{D7FF}',
        '\u{E000}',
        '\u{1F600}',
        '\u{10FFFF}',
    ] {
        assert_eq!(canonical_decomposition(scalar), vec![scalar]);
    }
}
