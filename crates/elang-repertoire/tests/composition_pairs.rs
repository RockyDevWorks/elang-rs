//! Conformance test pinning the canonical composition-pair closure
//! (ADR 0008 §7, T0814/T0825 follow-through).
//!
//! The composition-pair set is *derived* from the already-ratified IPA
//! inventories (`specs/schema/ipa-v1.json`) together with the pinned Unicode
//! 18.0.0 canonical-decomposition table (`crates/elang-unicode`). It is the
//! set of precomposed scalars whose full canonical decomposition is exactly
//! one approved base plus one approved combining mark, and which are not
//! themselves approved bases.
//!
//! Because the set is derived from ratified inputs, it does not need to be
//! stored as new normative data, and the repertoire digest is unaffected.
//! This test pins the exact derived closure as a golden list so that any
//! future change to the ratified inventories or to the Unicode pin is caught
//! and must be re-reviewed (T0114/T0825).
//!
//! The inline unit tests in `src/ipa.rs` cover individual behavior cases;
//! this file pins the closure in one place.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use elang_repertoire::ipa::{self, IpaError, Unit};
use elang_repertoire::repertoire::{self, Repertoire};

/// (precomposed scalar, decomposed base, decomposed combining mark).
/// Computed against the pinned Unicode 18.0.0 decomposition table and the
/// ratified IPA inventories; see the module-level note.
const GOLDEN_PAIRS: &[(char, char, char)] = &[
    ('\u{00E3}', '\u{0061}', '\u{0303}'), // ã = a + ̃
    ('\u{00E4}', '\u{0061}', '\u{0308}'), // ä = a + ̈
    ('\u{00E5}', '\u{0061}', '\u{030A}'), // å = a + ̊
    ('\u{00EB}', '\u{0065}', '\u{0308}'), // ë = e + ̈
    ('\u{00EF}', '\u{0069}', '\u{0308}'), // ï = i + ̈
    ('\u{00F1}', '\u{006E}', '\u{0303}'), // ñ = n + ̃
    ('\u{00F5}', '\u{006F}', '\u{0303}'), // õ = o + ̃
    ('\u{00F6}', '\u{006F}', '\u{0308}'), // ö = o + ̈
    ('\u{00FC}', '\u{0075}', '\u{0308}'), // ü = u + ̈
    ('\u{00FF}', '\u{0079}', '\u{0308}'), // ÿ = y + ̈
    ('\u{0129}', '\u{0069}', '\u{0303}'), // ĩ = i + ̃
    ('\u{0169}', '\u{0075}', '\u{0303}'), // ũ = u + ̃
    ('\u{016F}', '\u{0075}', '\u{030A}'), // ů = u + ̊
    ('\u{1E01}', '\u{0061}', '\u{0325}'), // ḁ = a + ̥
    ('\u{1E1B}', '\u{0065}', '\u{0330}'), // ḛ = e + ̰
    ('\u{1E27}', '\u{0068}', '\u{0308}'), // ḧ = h + ̈
    ('\u{1E2D}', '\u{0069}', '\u{0330}'), // ḭ = i + ̰
    ('\u{1E73}', '\u{0075}', '\u{0324}'), // ṳ = u + ̤
    ('\u{1E75}', '\u{0075}', '\u{0330}'), // ṵ = u + ̰
    ('\u{1E7D}', '\u{0076}', '\u{0303}'), // ṽ = v + ̃
    ('\u{1E85}', '\u{0077}', '\u{0308}'), // ẅ = w + ̈
    ('\u{1E8D}', '\u{0078}', '\u{0308}'), // ẍ = x + ̈
    ('\u{1E97}', '\u{0074}', '\u{0308}'), // ẗ = t + ̈
    ('\u{1E98}', '\u{0077}', '\u{030A}'), // ẘ = w + ̊
    ('\u{1E99}', '\u{0079}', '\u{030A}'), // ẙ = y + ̊
    ('\u{1EBD}', '\u{0065}', '\u{0303}'), // ẽ = e + ̃
    ('\u{1EF9}', '\u{0079}', '\u{0303}'), // ỹ = y + ̃
];

fn rep() -> Repertoire {
    repertoire::load().expect("ratified repertoire")
}

fn approved_set(symbols: &[repertoire::Symbol]) -> BTreeSet<char> {
    let mut out = BTreeSet::new();
    for s in symbols {
        if let Some(&cp) = s.codepoints.first() {
            out.insert(cp);
        }
    }
    out
}

/// Independently recompute the derived closure from the pinned Unicode table
/// and the loaded repertoire. If this disagrees with `GOLDEN_PAIRS`, either
/// the inventory changed or the Unicode pin changed; both require review.
fn derive_closure(rep: &Repertoire) -> Vec<(char, char, char)> {
    let bases = approved_set(&rep.base_symbols);
    let comb = approved_set(&rep.combining_diacritics);
    let mut out = Vec::new();
    for cp in 0x80u32..0x110000 {
        let Some(scalar) = char::from_u32(cp) else {
            continue; // surrogate code point: not a Unicode scalar value
        };
        // A scalar with no canonical decomposition maps to itself, so its
        // decomposition has one element and cannot be a composition pair.
        let decomposition = elang_unicode::nfc::canonical_decomposition(scalar);
        if decomposition.len() < 2 {
            continue;
        }
        if bases.contains(&scalar) {
            // Direct membership wins: an approved base is interpreted through
            // its own canonical decomposition for modifier counting, so it is
            // not part of the composition-pair closure.
            continue;
        }
        let Some((&base, marks)) = decomposition.split_first() else {
            continue;
        };
        if !bases.contains(&base) {
            continue;
        }
        match marks {
            [m] if comb.contains(m) => out.push((scalar, base, *m)),
            _ => continue,
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn derived_closure_matches_golden_list() {
    let r = rep();
    let derived = derive_closure(&r);
    let golden: Vec<(char, char, char)> = GOLDEN_PAIRS.to_vec();
    assert_eq!(
        derived, golden,
        "composition-pair closure drifted from the pinned golden list"
    );
}

#[test]
fn every_golden_pair_validates_as_one_unit_with_modifier() {
    let r = rep();
    for &(c, base, mark) in GOLDEN_PAIRS {
        let s = c.to_string();
        let id = ipa::validate(&s, &r)
            .unwrap_or_else(|e| panic!("U+{:04X} rejected: {:?}", u32::from(c), e));
        assert_eq!(
            id.units.len(),
            1,
            "U+{:04X} should be one unit",
            u32::from(c)
        );
        assert_eq!(
            id.units[0].bases,
            vec![base],
            "U+{:04X} should have the decomposed base",
            u32::from(c)
        );
        assert_eq!(
            id.units[0].modifier,
            Some(mark),
            "U+{:04X} should carry the decomposed combining mark",
            u32::from(c)
        );
        assert_eq!(
            id.original,
            s,
            "U+{:04X} authored scalar must be preserved byte-for-byte",
            u32::from(c)
        );
    }
}

#[test]
fn a_composition_pair_uses_the_single_modification_slot() {
    let r = rep();
    // ã already carries U+0303, so a second modification is too many.
    assert_eq!(
        ipa::validate("ã\u{02B0}", &r),
        Err(IpaError::TooManyModifications)
    );
    assert_eq!(
        ipa::validate("ã\u{0303}", &r),
        Err(IpaError::TooManyModifications)
    );
}

#[test]
fn no_multi_mark_precomposed_silently_passes() {
    let r = rep();
    // ṏ = o + U+0303 + U+0308: two approved marks on one base is more than
    // one modification, not one composition pair.
    assert!(matches!(
        ipa::validate("\u{1E4F}", &r),
        Err(IpaError::TooManyModifications)
    ));
    // ḉ = c + U+0327 + U+0301: U+0327 is not an approved combining mark, so
    // the scalar is not in the repertoire at all.
    assert_eq!(
        ipa::validate("\u{1E09}", &r),
        Err(IpaError::BaseNotApproved('\u{1E09}'))
    );
}

#[test]
fn approved_precomposed_base_is_interpreted_through_its_decomposition() {
    let r = rep();
    // ç is an approved base; for base+modifier counting it is interpreted
    // through its canonical decomposition, and the inherited cedilla occupies
    // the single modification slot.
    let id = ipa::validate("ç", &r).expect("approved precomposed base");
    assert_eq!(
        id.units,
        vec![Unit {
            bases: vec!['c'],
            tie: None,
            modifier: Some('\u{0327}'),
        }]
    );
    assert_eq!(id.original, "ç", "authored bytes must be preserved");
    assert_eq!(
        ipa::validate("ç\u{02B0}", &r),
        Err(IpaError::TooManyModifications)
    );
}

#[test]
fn a_single_unapproved_inherited_mark_is_rejected() {
    let r = rep();
    // à = a + U+0300: the inherited grave is an excluded prosodic mark, so
    // the precomposed scalar is not approved.
    assert_eq!(ipa::validate("à", &r), Err(IpaError::BaseNotApproved('à')));
}
