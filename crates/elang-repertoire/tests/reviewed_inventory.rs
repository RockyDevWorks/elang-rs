//! Regression checks for the IPA inventories ratified by T0114/T0825.
//!
//! These fixtures pin the reviewed `ipa-v1.json` contents and the two review
//! artifacts identified in `specs/sources/SOURCES.md` §§2–3, 6–7. They preserve
//! the review baseline; the independent base-chart transcription remains in
//! `chart_crosscheck.rs`, and human chart review is recorded in SOURCES.md.

#![allow(clippy::expect_used)]

use elang_repertoire::ipa::{self, IpaError, Unit, TIE_LOWER, TIE_UPPER};
use elang_repertoire::repertoire::{load, Symbol, REPERTOIRE_JSON};
use sha2::{Digest, Sha256};

const REVIEWED_COMBINING: [char; 27] = [
    '\u{0303}', // nasalized
    '\u{0308}', // centralized
    '\u{030A}', // voiceless, above
    '\u{0318}', // advanced tongue root
    '\u{0319}', // retracted tongue root
    '\u{031A}', // no audible release
    '\u{031C}', // less rounded
    '\u{031D}', // raised
    '\u{031E}', // lowered
    '\u{031F}', // advanced
    '\u{0320}', // retracted
    '\u{0324}', // breathy voiced
    '\u{0325}', // voiceless, below
    '\u{0329}', // syllabic
    '\u{032A}', // dental
    '\u{032B}', // labialized, below
    '\u{032C}', // voiced
    '\u{032F}', // non-syllabic
    '\u{0330}', // creaky voiced
    '\u{0334}', // velarized or pharyngealized
    '\u{0339}', // more rounded
    '\u{033A}', // apical
    '\u{033B}', // laminal
    '\u{033C}', // linguolabial
    '\u{033D}', // mid-centralized
    '\u{0346}', // dental/raised variant
    '\u{0348}', // strong articulation
];

const REVIEWED_SPACING: [char; 9] = [
    '\u{02B0}', // aspirated
    '\u{02B2}', // palatalized
    '\u{02B7}', // labialized
    '\u{02BC}', // ejective
    '\u{02DE}', // rhoticity
    '\u{02E0}', // velarized
    '\u{02E1}', // lateral release
    '\u{02E4}', // pharyngealized
    '\u{207F}', // nasal release
];

fn assert_reviewed_inventory(actual: &[Symbol], reviewed: &[char]) {
    let mut expected: Vec<String> = reviewed.iter().map(char::to_string).collect();
    expected.sort();
    assert!(
        expected.windows(2).all(|pair| pair.first() != pair.get(1)),
        "review fixture must have no duplicates"
    );
    let symbols: Vec<&str> = actual.iter().map(|entry| entry.symbol.as_str()).collect();
    assert_eq!(symbols, expected, "reviewed inventory must match exactly");
    for entry in actual {
        assert_eq!(
            entry.codepoints,
            entry.symbol.chars().collect::<Vec<_>>(),
            "codepoints for {:?}",
            entry.symbol
        );
    }
}

#[test]
fn combining_diacritics_match_all_27_reviewed_entries() {
    let rep = load().expect("ratified repertoire loads");
    assert_reviewed_inventory(&rep.combining_diacritics, &REVIEWED_COMBINING);
}

#[test]
fn spacing_modifiers_match_all_9_reviewed_entries() {
    let rep = load().expect("ratified repertoire loads");
    assert_reviewed_inventory(&rep.spacing_modifiers, &REVIEWED_SPACING);
}

#[test]
fn review_artifacts_match_the_published_raw_byte_hashes() {
    // These are raw-byte hashes, distinct from the repertoire's JCS digest.
    let artifacts: [(&str, &[u8], &str); 2] = [
        (
            "UNITIPA 2020 chart",
            include_bytes!("../../../specs/sources/unitipa-ipa-chart-2020.pdf"),
            "882a0e4df2ad8c45c53752ecfecc506647e5e39b47dc83646fca925ce84f5fc9",
        ),
        (
            "reviewed ipa-v1.json",
            REPERTOIRE_JSON.as_bytes(),
            "5f5711c0ba33423e924f5869ef563851c725b218704262b80b90406e87304b54",
        ),
    ];
    for (name, bytes, expected) in artifacts {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn every_reviewed_modifier_attaches_to_single_and_tied_units() {
    let rep = load().expect("ratified repertoire loads");
    // Schwa and esh have no NFC compositions with the reviewed diacritics,
    // so these inputs exercise attachment without requiring normalization.
    for (stem, bases, tie) in [
        ("ə", vec!['ə'], None),
        ("t\u{0361}ʃ", vec!['t', 'ʃ'], Some(TIE_UPPER)),
        ("t\u{035C}ʃ", vec!['t', 'ʃ'], Some(TIE_LOWER)),
    ] {
        for modifier in REVIEWED_COMBINING.into_iter().chain(REVIEWED_SPACING) {
            let input = format!("{stem}{modifier}");
            let id = ipa::validate(&input, &rep).expect("reviewed modifier attaches to a base");
            assert_eq!(id.original, input, "authored bytes must be preserved");
            assert_eq!(
                id.units,
                vec![Unit {
                    bases: bases.clone(),
                    tie,
                    modifier: Some(modifier),
                }],
                "parsed unit for {input:?}"
            );
        }
    }
}

#[test]
fn every_reviewed_modifier_requires_a_preceding_base() {
    let rep = load().expect("ratified repertoire loads");
    for modifier in REVIEWED_COMBINING.into_iter().chain(REVIEWED_SPACING) {
        for input in [modifier.to_string(), format!("{modifier}ə")] {
            assert_eq!(
                ipa::validate(&input, &rep),
                Err(IpaError::StandaloneModifier(modifier)),
                "standalone or leading modifier in {input:?}"
            );
        }
    }
}

#[test]
fn excluded_prosody_is_absent_and_rejected_even_after_a_valid_base() {
    let rep = load().expect("ratified repertoire loads");
    // Every explicit example in SOURCES.md §6, including the combining tone
    // diacritics that can visually resemble approved segmental diacritics.
    let excluded = [
        '\u{02C8}', '\u{02CC}', // primary/secondary stress
        '\u{02D0}', '\u{02D1}', // length/half-length
        '.', '\u{203F}', // syllable break/linking
        '\u{02E5}', '\u{02E6}', '\u{02E7}', '\u{02E8}', '\u{02E9}', // tone letters
        '\u{030B}', '\u{0301}', '\u{0304}', '\u{0300}', '\u{030F}', // tone diacritics
        '|', '\u{2016}', // minor/major group
        '\u{2197}', '\u{2198}', // global rise/fall
    ];
    for scalar in excluded {
        let symbol = scalar.to_string();
        assert!(
            rep.base_symbols
                .iter()
                .chain(&rep.combining_diacritics)
                .chain(&rep.spacing_modifiers)
                .all(|entry| entry.symbol != symbol),
            "prosodic U+{:04X} must be absent from all inventories",
            scalar as u32
        );
        // Schwa avoids composing with tone accents, so rejection must come
        // from the allowlist rather than the earlier NFC validation step.
        for input in [symbol, format!("ə{scalar}")] {
            assert_eq!(
                ipa::validate(&input, &rep),
                Err(IpaError::BaseNotApproved(scalar)),
                "excluded prosody in {input:?}"
            );
        }
    }
}

#[test]
fn dotted_circle_review_carrier_is_not_an_approved_base() {
    let rep = load().expect("ratified repertoire loads");
    // SOURCES.md §7 uses the dotted circle only to display combining marks.
    // A rendered review cell must never become an accepted IPA identifier.
    for input in ["\u{25CC}", "\u{25CC}\u{0303}"] {
        assert_eq!(
            ipa::validate(input, &rep),
            Err(IpaError::BaseNotApproved('\u{25CC}')),
            "review carrier in {input:?}"
        );
    }
}
