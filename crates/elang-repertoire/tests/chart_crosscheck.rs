//! Independent cross-check of the generated IPA repertoire base symbols
//! against the official IPA chart grids.
//!
//! The list below is transcribed **independently** from the authoritative IPA
//! chart (UNITIPA revised-to-2020 / IPA 2005) consonant, non-pulmonic, "other
//! symbols" and vowel grids, as published by the International Phonetic
//! Association and mirrored by the UCLA phonetics archive. It is deliberately
//! organized by chart section (not by code point) so a reviewer can tick each
//! cell of the printed chart against this file. The test asserts the generated
//! `ipa-v1.json` base-symbol set equals this independent set exactly.
//!
//! This is the T0114 "independent review against the official chart" gate in
//! executable form: if the generator and this transcription disagree, the test
//! fails and neither can be marked reviewed.
#![allow(clippy::expect_used, clippy::indexing_slicing)]

use elang_repertoire::repertoire::load;

/// Official IPA base symbols, grouped by chart section (voiceless/voiced where
/// the chart pairs them). Script-g `ɡ` (U+0261) is the official symbol, not
/// ASCII `g`.
fn official_base_symbols() -> Vec<&'static str> {
    let mut v = Vec::new();

    // --- Pulmonic consonants ---
    // Plosive
    v.extend(["p", "b", "t", "d", "ʈ", "ɖ", "c", "ɟ", "k", "ɡ", "q", "ɢ", "ʔ"]);
    // Nasal
    v.extend(["m", "ɱ", "n", "ɳ", "ɲ", "ŋ", "ɴ"]);
    // Trill
    v.extend(["ʙ", "r", "ʀ"]);
    // Tap or flap (incl. labiodental flap ⱱ)
    v.extend(["ⱱ", "ɾ", "ɽ"]);
    // Fricative
    v.extend([
        "ɸ", "β", "f", "v", "θ", "ð", "s", "z", "ʃ", "ʒ", "ʂ", "ʐ", "ç", "ʝ", "x", "ɣ", "χ",
        "ʁ", "ħ", "ʕ", "h", "ɦ",
    ]);
    // Lateral fricative
    v.extend(["ɬ", "ɮ"]);
    // Approximant
    v.extend(["ʋ", "ɹ", "ɻ", "j", "ɰ"]);
    // Lateral approximant
    v.extend(["l", "ɭ", "ʎ", "ʟ"]);

    // --- Other symbols ---
    v.extend(["ʍ", "w", "ɥ", "ʜ", "ʢ", "ʡ", "ɕ", "ʑ", "ɺ", "ɧ"]);

    // --- Non-pulmonic: clicks ---
    v.extend(["ʘ", "ǀ", "ǃ", "ǂ", "ǁ"]);
    // --- Non-pulmonic: voiced implosives ---
    v.extend(["ɓ", "ɗ", "ʄ", "ɠ", "ʛ"]);

    // --- Vowels ---
    // close / near-close
    v.extend(["i", "y", "ɨ", "ʉ", "ɯ", "u", "ɪ", "ʏ", "ʊ"]);
    // close-mid / mid
    v.extend(["e", "ø", "ɘ", "ɵ", "ɤ", "o", "ə"]);
    // open-mid
    v.extend(["ɛ", "œ", "ɜ", "ɞ", "ʌ", "ɔ"]);
    // near-open / open
    v.extend(["æ", "ɐ", "a", "ɶ", "ɑ", "ɒ"]);

    v
}

#[test]
fn generated_base_symbols_match_official_chart() {
    let rep = load().expect("repertoire loads");

    let mut generated: Vec<String> = rep.base_symbols.iter().map(|s| s.symbol.clone()).collect();
    generated.sort();

    let mut official: Vec<String> = official_base_symbols()
        .into_iter()
        .map(str::to_string)
        .collect();

    // The independent list must itself be free of duplicates.
    let before = official.len();
    official.sort();
    official.dedup();
    assert_eq!(
        before,
        official.len(),
        "independent official list has a duplicate"
    );

    // Symmetric difference, reported precisely so a reviewer sees any drift.
    let missing: Vec<&String> = official.iter().filter(|s| !generated.contains(s)).collect();
    let extra: Vec<&String> = generated.iter().filter(|s| !official.contains(s)).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "base symbol mismatch vs official chart.\n  missing from generated: {missing:?}\n  extra in generated: {extra:?}"
    );

    // And the counts agree with the well-known 107 segmental letters.
    assert_eq!(generated.len(), 107);
}
