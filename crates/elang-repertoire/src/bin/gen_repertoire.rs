//! Generator for `specs/schema/ipa-v1.json` — the ELANG IPA Repertoire v1
//! allowlist (base symbols, combining diacritics, spacing modifiers).
//!
//! # Why a generator
//!
//! The JSON file must be **reproducible and reviewable**. This source is the
//! human-reviewable artifact: every approved symbol is listed once, with its
//! Unicode scalar value(s) and Unicode character name, grouped by the official
//! IPA chart section it comes from. The generator sorts and deduplicates, then
//! emits deterministic JSON. Re-running it MUST reproduce the committed file
//! byte-for-byte.
//!
//! # Authoritative source
//!
//! The approved inventory is taken from the **UNITIPA International Phonetic
//! Alphabet chart (revised to 2020)** — the official IPA symbol set
//! (2005 revision, 2015/2020 typographic updates). ELANG pins this as its
//! "IPA 2026" issue label (steering §7: *"IPA 2026 chart issue (2015/2005
//! underlying revision)"*). Source files and their SHA-256 hashes are recorded
//! in `specs/sources/SOURCES.md`.
//!
//! Unicode scalar values follow the Unicode Standard (IPA Extensions U+0250..,
//! Spacing Modifier Letters U+02B0.., Combining Diacritical Marks U+0300..,
//! plus Basic Latin / Latin-1 letters used as IPA symbols). They are
//! independently verifiable against any Unicode Character Database.
//!
//! # ELANG exclusions (steering §7)
//!
//! Prosodic categories are **excluded** from identifiers and are NOT in this
//! allowlist: suprasegmentals (primary/secondary stress ˈ ˌ, length ː ˑ,
//! syllable break ., linking ‿), tone/word-accent letters and diacritics, and
//! intonation marks (↗ ↘ | ‖). These are reserved for future prosody features.
//!
//! Run with: `cargo run -p elang-repertoire --bin gen_repertoire`
//!
//! This is a build-time developer tool, not library code that runs on
//! untrusted input, so the workspace's untrusted-input lints are relaxed here:
//! a panic in the generator is the intended failure mode (fail closed at build
//! time), and the inputs are the hard-coded tables above.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::missing_docs_in_private_items
)]

use std::fmt::Write as _;

/// One approved symbol: its NFC string and the Unicode name, for review.
struct Entry {
    /// The symbol as authored (expected already-NFC, single or multi scalar).
    symbol: &'static str,
    /// Human-readable Unicode name(s), for independent review.
    name: &'static str,
}

const fn e(symbol: &'static str, name: &'static str) -> Entry {
    Entry { symbol, name }
}

// ---------------------------------------------------------------------------
// BASE SYMBOLS
// Pulmonic consonants, non-pulmonic consonants, "other symbols", and vowels
// from the official IPA chart. Each is a standalone base letter.
// ---------------------------------------------------------------------------
fn base_symbols() -> Vec<Entry> {
    vec![
        // --- Pulmonic consonants: plosives ---
        e("p", "LATIN SMALL LETTER P"),
        e("b", "LATIN SMALL LETTER B"),
        e("t", "LATIN SMALL LETTER T"),
        e("d", "LATIN SMALL LETTER D"),
        e("ʈ", "LATIN SMALL LETTER T WITH RETROFLEX HOOK"),
        e("ɖ", "LATIN SMALL LETTER D WITH TAIL"),
        e("c", "LATIN SMALL LETTER C"),
        e("ɟ", "LATIN SMALL LETTER DOTLESS J WITH STROKE"),
        e("k", "LATIN SMALL LETTER K"),
        e("ɡ", "LATIN SMALL LETTER SCRIPT G"),
        e("q", "LATIN SMALL LETTER Q"),
        e("ɢ", "LATIN LETTER SMALL CAPITAL G"),
        e("ʔ", "LATIN LETTER GLOTTAL STOP"),
        // --- Nasals ---
        e("m", "LATIN SMALL LETTER M"),
        e("ɱ", "LATIN SMALL LETTER M WITH HOOK"),
        e("n", "LATIN SMALL LETTER N"),
        e("ɳ", "LATIN SMALL LETTER N WITH RETROFLEX HOOK"),
        e("ɲ", "LATIN SMALL LETTER N WITH LEFT HOOK"),
        e("ŋ", "LATIN SMALL LETTER ENG"),
        e("ɴ", "LATIN LETTER SMALL CAPITAL N"),
        // --- Trills ---
        e("ʙ", "LATIN LETTER SMALL CAPITAL B"),
        e("r", "LATIN SMALL LETTER R"),
        e("ʀ", "LATIN LETTER SMALL CAPITAL R"),
        // --- Taps/flaps ---
        e("ⱱ", "LATIN SMALL LETTER V WITH RIGHT HOOK"),
        e("ɾ", "LATIN SMALL LETTER R WITH FISHHOOK"),
        e("ɽ", "LATIN SMALL LETTER R WITH TAIL"),
        // --- Fricatives ---
        e("ɸ", "LATIN SMALL LETTER PHI"),
        e("β", "GREEK SMALL LETTER BETA"),
        e("f", "LATIN SMALL LETTER F"),
        e("v", "LATIN SMALL LETTER V"),
        e("θ", "GREEK SMALL LETTER THETA"),
        e("ð", "LATIN SMALL LETTER ETH"),
        e("s", "LATIN SMALL LETTER S"),
        e("z", "LATIN SMALL LETTER Z"),
        e("ʃ", "LATIN SMALL LETTER ESH"),
        e("ʒ", "LATIN SMALL LETTER EZH"),
        e("ʂ", "LATIN SMALL LETTER S WITH HOOK"),
        e("ʐ", "LATIN SMALL LETTER Z WITH RETROFLEX HOOK"),
        e("ç", "LATIN SMALL LETTER C WITH CEDILLA"),
        e("ʝ", "LATIN SMALL LETTER J WITH CROSSED-TAIL"),
        e("x", "LATIN SMALL LETTER X"),
        e("ɣ", "LATIN SMALL LETTER GAMMA"),
        e("χ", "GREEK SMALL LETTER CHI"),
        e("ʁ", "LATIN LETTER SMALL CAPITAL INVERTED R"),
        e("ħ", "LATIN SMALL LETTER H WITH STROKE"),
        e("ʕ", "LATIN LETTER PHARYNGEAL VOICED FRICATIVE"),
        e("h", "LATIN SMALL LETTER H"),
        e("ɦ", "LATIN SMALL LETTER H WITH HOOK"),
        // --- Lateral fricatives ---
        e("ɬ", "LATIN SMALL LETTER L WITH BELT"),
        e("ɮ", "LATIN SMALL LETTER LEZH"),
        // --- Approximants ---
        e("ʋ", "LATIN SMALL LETTER V WITH HOOK"),
        e("ɹ", "LATIN SMALL LETTER TURNED R"),
        e("ɻ", "LATIN SMALL LETTER TURNED R WITH HOOK"),
        e("j", "LATIN SMALL LETTER J"),
        e("ɰ", "LATIN SMALL LETTER TURNED M WITH LONG LEG"),
        // --- Lateral approximants ---
        e("l", "LATIN SMALL LETTER L"),
        e("ɭ", "LATIN SMALL LETTER L WITH RETROFLEX HOOK"),
        e("ʎ", "LATIN SMALL LETTER TURNED Y"),
        e("ʟ", "LATIN LETTER SMALL CAPITAL L"),
        // --- Other symbols ---
        e("ʍ", "LATIN SMALL LETTER TURNED W"),
        e("w", "LATIN SMALL LETTER W"),
        e("ɥ", "LATIN SMALL LETTER TURNED H"),
        e("ʜ", "LATIN LETTER SMALL CAPITAL H"),
        e("ʢ", "LATIN LETTER REVERSED GLOTTAL STOP WITH STROKE"),
        e("ʡ", "LATIN LETTER GLOTTAL STOP WITH STROKE"),
        e("ɕ", "LATIN SMALL LETTER C WITH CURL"),
        e("ʑ", "LATIN SMALL LETTER Z WITH CURL"),
        e("ɺ", "LATIN SMALL LETTER TURNED R WITH LONG LEG"),
        e("ɧ", "LATIN SMALL LETTER HENG WITH HOOK"),
        // --- Non-pulmonic: clicks ---
        e("ʘ", "LATIN LETTER BILABIAL CLICK"),
        e("ǀ", "LATIN LETTER DENTAL CLICK"),
        e("ǃ", "LATIN LETTER RETROFLEX CLICK"),
        e("ǂ", "LATIN LETTER ALVEOLAR CLICK"),
        e("ǁ", "LATIN LETTER LATERAL CLICK"),
        // --- Non-pulmonic: voiced implosives ---
        e("ɓ", "LATIN SMALL LETTER B WITH HOOK"),
        e("ɗ", "LATIN SMALL LETTER D WITH HOOK"),
        e("ʄ", "LATIN SMALL LETTER DOTLESS J WITH STROKE AND HOOK"),
        e("ɠ", "LATIN SMALL LETTER G WITH HOOK"),
        e("ʛ", "LATIN LETTER SMALL CAPITAL G WITH HOOK"),
        // --- Vowels ---
        e("i", "LATIN SMALL LETTER I"),
        e("y", "LATIN SMALL LETTER Y"),
        e("ɨ", "LATIN SMALL LETTER I WITH STROKE"),
        e("ʉ", "LATIN SMALL LETTER U BAR"),
        e("ɯ", "LATIN SMALL LETTER TURNED M"),
        e("u", "LATIN SMALL LETTER U"),
        e("ɪ", "LATIN LETTER SMALL CAPITAL I"),
        e("ʏ", "LATIN LETTER SMALL CAPITAL Y"),
        e("ʊ", "LATIN SMALL LETTER UPSILON"),
        e("e", "LATIN SMALL LETTER E"),
        e("ø", "LATIN SMALL LETTER O WITH STROKE"),
        e("ɘ", "LATIN SMALL LETTER REVERSED E"),
        e("ɵ", "LATIN SMALL LETTER BARRED O"),
        e("ɤ", "LATIN SMALL LETTER RAMS HORN"),
        e("o", "LATIN SMALL LETTER O"),
        e("ə", "LATIN SMALL LETTER SCHWA"),
        e("ɛ", "LATIN SMALL LETTER OPEN E"),
        e("œ", "LATIN SMALL LIGATURE OE"),
        e("ɜ", "LATIN SMALL LETTER REVERSED OPEN E"),
        e("ɞ", "LATIN SMALL LETTER CLOSED REVERSED OPEN E"),
        e("ʌ", "LATIN SMALL LETTER TURNED V"),
        e("ɔ", "LATIN SMALL LETTER OPEN O"),
        e("æ", "LATIN SMALL LETTER AE"),
        e("ɐ", "LATIN SMALL LETTER TURNED A"),
        e("a", "LATIN SMALL LETTER A"),
        e("ɶ", "LATIN LETTER SMALL CAPITAL OE"),
        e("ɑ", "LATIN SMALL LETTER ALPHA"),
        e("ɒ", "LATIN SMALL LETTER TURNED ALPHA"),
    ]
}

// ---------------------------------------------------------------------------
// COMBINING DIACRITICS (IPA "Diacritics" block, combining forms).
// These attach to a base. Suprasegmental/tone diacritics are EXCLUDED.
// ---------------------------------------------------------------------------
fn combining_diacritics() -> Vec<Entry> {
    vec![
        e("\u{0325}", "COMBINING RING BELOW (voiceless)"),
        e("\u{030A}", "COMBINING RING ABOVE (voiceless, above)"),
        e("\u{032C}", "COMBINING CARON BELOW (voiced)"),
        e("\u{0339}", "COMBINING RIGHT HALF RING BELOW (more rounded)"),
        e("\u{031C}", "COMBINING LEFT HALF RING BELOW (less rounded)"),
        e("\u{031F}", "COMBINING PLUS SIGN BELOW (advanced)"),
        e("\u{0320}", "COMBINING MINUS SIGN BELOW (retracted)"),
        e("\u{0308}", "COMBINING DIAERESIS (centralized)"),
        e("\u{033D}", "COMBINING X ABOVE (mid-centralized)"),
        e("\u{0329}", "COMBINING VERTICAL LINE BELOW (syllabic)"),
        e("\u{032F}", "COMBINING INVERTED BREVE BELOW (non-syllabic)"),
        e("\u{0324}", "COMBINING DIAERESIS BELOW (breathy voiced)"),
        e("\u{0330}", "COMBINING TILDE BELOW (creaky voiced)"),
        e("\u{033C}", "COMBINING SEAGULL BELOW (linguolabial)"),
        e("\u{032A}", "COMBINING BRIDGE BELOW (dental)"),
        e("\u{033A}", "COMBINING INVERTED BRIDGE BELOW (apical)"),
        e("\u{033B}", "COMBINING SQUARE BELOW (laminal)"),
        e("\u{0303}", "COMBINING TILDE (nasalized)"),
        e("\u{031A}", "COMBINING LEFT ANGLE ABOVE (no audible release)"),
        e("\u{0334}", "COMBINING TILDE OVERLAY (velarized or pharyngealized)"),
        e("\u{031D}", "COMBINING UP TACK BELOW (raised)"),
        e("\u{031E}", "COMBINING DOWN TACK BELOW (lowered)"),
        e("\u{0318}", "COMBINING LEFT TACK BELOW (advanced tongue root)"),
        e("\u{0319}", "COMBINING RIGHT TACK BELOW (retracted tongue root)"),
        e("\u{032B}", "COMBINING INVERTED DOUBLE ARCH BELOW (labialized, w-variant below)"),
        e("\u{0346}", "COMBINING BRIDGE ABOVE (dental/raised variant)"),
        e("\u{0348}", "COMBINING DOUBLE VERTICAL LINE BELOW (strong articulation)"),
    ]
}

// ---------------------------------------------------------------------------
// SPACING MODIFIERS (IPA spacing modifier letters that modify a base).
// Length marks and stress marks are suprasegmental and are EXCLUDED.
// ---------------------------------------------------------------------------
fn spacing_modifiers() -> Vec<Entry> {
    vec![
        e("ʰ", "MODIFIER LETTER SMALL H (aspirated)"),
        e("ʷ", "MODIFIER LETTER SMALL W (labialized)"),
        e("ʲ", "MODIFIER LETTER SMALL Y (palatalized)"),
        e("ˠ", "MODIFIER LETTER SMALL GAMMA (velarized)"),
        e("ˤ", "MODIFIER LETTER SMALL REVERSED GLOTTAL STOP (pharyngealized)"),
        e("ⁿ", "SUPERSCRIPT LATIN SMALL LETTER N (nasal release)"),
        e("ˡ", "MODIFIER LETTER SMALL L (lateral release)"),
        e("ʼ", "MODIFIER LETTER APOSTROPHE (ejective)"),
        e("˞", "MODIFIER LETTER RHOTIC HOOK (rhoticity)"),
    ]
}

/// Format a single JSON entry: `symbol`, `codepoints`, `name`.
fn format_entry(out: &mut String, entry: &Entry, indent: &str) {
    let cps: Vec<String> = entry
        .symbol
        .chars()
        .map(|c| format!("\"U+{:04X}\"", c as u32))
        .collect();
    let symbol_json = json_escape(entry.symbol);
    let name_json = json_escape(entry.name);
    let _ = writeln!(out, "{indent}{{");
    let _ = writeln!(out, "{indent}  \"symbol\": \"{symbol_json}\",");
    let _ = writeln!(out, "{indent}  \"codepoints\": [{}],", cps.join(", "));
    let _ = writeln!(out, "{indent}  \"name\": \"{name_json}\"");
    let _ = write!(out, "{indent}}}");
}

/// Minimal JSON string escaping for `"` and `\` (symbol/name have no controls).
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Sort by raw UTF-8 bytes of the symbol and reject duplicates (fail closed).
fn sort_dedup(mut v: Vec<Entry>, label: &str) -> Vec<Entry> {
    v.sort_by(|a, b| a.symbol.as_bytes().cmp(b.symbol.as_bytes()));
    for w in v.windows(2) {
        assert_ne!(
            w[0].symbol, w[1].symbol,
            "duplicate {label} entry: {:?}",
            w[0].symbol
        );
    }
    v
}

fn emit_array(out: &mut String, key: &str, entries: &[Entry], last: bool) {
    let _ = writeln!(out, "  \"{key}\": [");
    for (i, entry) in entries.iter().enumerate() {
        format_entry(out, entry, "    ");
        if i + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    if last {
        let _ = writeln!(out, "  ]");
    } else {
        let _ = writeln!(out, "  ],");
    }
}

fn main() {
    let base = sort_dedup(base_symbols(), "base_symbols");
    let comb = sort_dedup(combining_diacritics(), "combining_diacritics");
    let spacing = sort_dedup(spacing_modifiers(), "spacing_modifiers");

    let mut out = String::new();
    out.push_str("{\n");
    let _ = writeln!(out, "  \"repertoire_version\": 1,");
    let _ = writeln!(out, "  \"unicode_version\": \"18.0.0\",");
    let _ = writeln!(out, "  \"ipa_chart\": \"2026\",");
    emit_array(&mut out, "base_symbols", &base, false);
    emit_array(&mut out, "combining_diacritics", &comb, false);
    emit_array(&mut out, "spacing_modifiers", &spacing, true);
    out.push_str("}\n");

    // Write next to the schema it conforms to.
    let path = "specs/schema/ipa-v1.json";
    std::fs::write(path, &out).expect("write ipa-v1.json");
    eprintln!(
        "wrote {path}: {} base, {} combining, {} spacing",
        base.len(),
        comb.len(),
        spacing.len()
    );
}
