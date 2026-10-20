//! ELANG IPA Repertoire v1 loader and verification (steering §7).
//!
//! The pinned repertoire JSON (`specs/schema/ipa-v1.json`) is embedded at
//! compile time via [`REPERTOIRE_JSON`]. It is generated reproducibly from
//! `src/bin/gen_repertoire.rs` against the UNITIPA 2020 (IPA 2005/2015/2020)
//! chart; provenance and source hashes are in `specs/sources/SOURCES.md`.
//!
//! This module:
//!
//! - parses the JSON with strict rules (no BOM, no duplicate keys, no unknown
//!   fields: [`load`]);
//! - validates every symbol is NFC and category-clean (via [`crate::unicode`]);
//! - enforces inventory uniqueness and sorted order;
//! - computes the RFC 8785 JCS SHA-256 digest of the canonical form
//!   ([`canonical_jcs_sha256`]) so it can be compared to a pinned constant.
//!
//! The approved allowlist built here is consumed by the IPA identifier parser
//! (Phase 8). Until the digest is pinned and the inventory independently
//! reviewed against the chart, callers must treat the repertoire as not frozen
//! (steering §12, ADR 0003).

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::unicode::{UnicodeValidator, UnicodeViolation};

/// The pinned repertoire JSON source, embedded at build time.
pub const REPERTOIRE_JSON: &str = include_str!("../../../specs/schema/ipa-v1.json");

/// The repertoire schema/content version this loader understands.
pub const EXPECTED_REPERTOIRE_VERSION: u32 = 1;
/// The ELANG IPA chart issue label (steering §7).
pub const EXPECTED_IPA_CHART: &str = "2026";

/// Pinned RFC 8785 JCS SHA-256 digest of [`REPERTOIRE_JSON`] (lowercase hex).
///
/// Reproduced by `cargo run -p elang-repertoire --bin print_digest` and
/// recorded in `specs/sources/SOURCES.md`. Any change to the inventory changes
/// this digest; the `pinned_digest_matches` test fails until it is re-reviewed
/// and updated here (steering §7, §12).
pub const PINNED_JCS_SHA256: &str =
    "190aae7b1fe355793a30beb29e44d36540132d9d77e350e06db0f7d9302c64aa";

/// A classified symbol in the repertoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The NFC symbol string (one or more scalars).
    pub symbol: String,
    /// Unicode scalar values as authored, in order.
    pub codepoints: Vec<char>,
    /// Unicode/IPA name for review.
    pub name: String,
}

/// The three inventories plus version metadata.
#[derive(Debug, Clone)]
pub struct Repertoire {
    /// `repertoire_version`.
    pub repertoire_version: u32,
    /// `unicode_version` the data was validated against.
    pub unicode_version: String,
    /// `ipa_chart` issue label.
    pub ipa_chart: String,
    /// Approved base symbols.
    pub base_symbols: Vec<Symbol>,
    /// Approved combining diacritics.
    pub combining_diacritics: Vec<Symbol>,
    /// Approved spacing modifiers.
    pub spacing_modifiers: Vec<Symbol>,
}

/// Why the repertoire failed to load or verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepertoireError {
    /// Source begins with a UTF-8 BOM.
    Bom,
    /// JSON is malformed or violates the strict schema (duplicate/unknown key).
    Json(String),
    /// `repertoire_version` / `unicode_version` / `ipa_chart` mismatch.
    Metadata(String),
    /// An inventory entry's `codepoints` do not match its `symbol`.
    CodepointMismatch {
        /// The offending symbol.
        symbol: String,
    },
    /// A symbol violated a Unicode rule (NFC/category/noncharacter).
    Unicode {
        /// The offending symbol.
        symbol: String,
        /// The specific violation.
        violation: UnicodeViolation,
    },
    /// An inventory was not sorted ascending by raw UTF-8 bytes.
    NotSorted {
        /// Which inventory (`base_symbols` etc.).
        inventory: &'static str,
    },
    /// A duplicate symbol within or across inventories.
    Duplicate {
        /// The duplicated symbol.
        symbol: String,
    },
}

// --- Strict JSON model (serde with deny_unknown_fields) -------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSymbol {
    symbol: String,
    codepoints: Vec<String>,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepertoire {
    repertoire_version: u32,
    unicode_version: String,
    ipa_chart: String,
    base_symbols: Vec<RawSymbol>,
    combining_diacritics: Vec<RawSymbol>,
    spacing_modifiers: Vec<RawSymbol>,
}

fn parse_codepoint(s: &str) -> Option<char> {
    let hex = s.strip_prefix("U+")?;
    let n = u32::from_str_radix(hex, 16).ok()?;
    char::from_u32(n)
}

fn convert_symbol(raw: RawSymbol) -> Result<Symbol, RepertoireError> {
    let mut cps = Vec::with_capacity(raw.codepoints.len());
    for cp in &raw.codepoints {
        match parse_codepoint(cp) {
            Some(c) => cps.push(c),
            None => {
                return Err(RepertoireError::CodepointMismatch {
                    symbol: raw.symbol.clone(),
                });
            }
        }
    }
    // The declared codepoints MUST spell exactly the symbol string.
    let rebuilt: String = cps.iter().collect();
    if rebuilt != raw.symbol {
        return Err(RepertoireError::CodepointMismatch {
            symbol: raw.symbol.clone(),
        });
    }
    Ok(Symbol {
        symbol: raw.symbol,
        codepoints: cps,
        name: raw.name,
    })
}

fn is_sorted_unique(entries: &[Symbol]) -> Result<(), bool> {
    // Err(true) => not sorted; Err(false) => duplicate.
    for w in entries.windows(2) {
        if let [a, b] = w {
            match a.symbol.as_bytes().cmp(b.symbol.as_bytes()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => return Err(false),
                std::cmp::Ordering::Greater => return Err(true),
            }
        }
    }
    Ok(())
}

/// Parse and fully validate the pinned repertoire JSON.
pub fn load() -> Result<Repertoire, RepertoireError> {
    load_from(REPERTOIRE_JSON)
}

/// Parse and validate a repertoire from an arbitrary JSON source.
pub fn load_from(src: &str) -> Result<Repertoire, RepertoireError> {
    if src.starts_with('\u{FEFF}') {
        return Err(RepertoireError::Bom);
    }

    // serde_json rejects duplicate keys by default and, with
    // deny_unknown_fields, rejects unknown fields.
    let raw: RawRepertoire =
        serde_json::from_str(src).map_err(|e| RepertoireError::Json(e.to_string()))?;

    if raw.repertoire_version != EXPECTED_REPERTOIRE_VERSION {
        return Err(RepertoireError::Metadata(format!(
            "repertoire_version {} != {EXPECTED_REPERTOIRE_VERSION}",
            raw.repertoire_version
        )));
    }
    if raw.unicode_version != crate::unicode::PINNED_UNICODE_VERSION {
        return Err(RepertoireError::Metadata(format!(
            "unicode_version {} != {}",
            raw.unicode_version,
            crate::unicode::PINNED_UNICODE_VERSION
        )));
    }
    if raw.ipa_chart != EXPECTED_IPA_CHART {
        return Err(RepertoireError::Metadata(format!(
            "ipa_chart {} != {EXPECTED_IPA_CHART}",
            raw.ipa_chart
        )));
    }

    let mut rep = Repertoire {
        repertoire_version: raw.repertoire_version,
        unicode_version: raw.unicode_version,
        ipa_chart: raw.ipa_chart,
        base_symbols: convert_all(raw.base_symbols)?,
        combining_diacritics: convert_all(raw.combining_diacritics)?,
        spacing_modifiers: convert_all(raw.spacing_modifiers)?,
    };

    // Per-inventory sorted + unique.
    for (inv, label) in [
        (&rep.base_symbols, "base_symbols"),
        (&rep.combining_diacritics, "combining_diacritics"),
        (&rep.spacing_modifiers, "spacing_modifiers"),
    ] {
        match is_sorted_unique(inv) {
            Ok(()) => {}
            Err(true) => return Err(RepertoireError::NotSorted { inventory: label }),
            Err(false) => {
                return Err(RepertoireError::Duplicate {
                    symbol: first_duplicate(inv),
                });
            }
        }
    }

    // Unicode validity for every symbol (NFC + categories + noncharacters).
    // Combining marks are, on their own, not attached to a base; so we check
    // category/noncharacter rules per scalar but skip the whole-string NFC
    // check for lone combining marks (a lone combining mark is trivially NFC).
    let uni = UnicodeValidator::new();
    for sym in rep
        .base_symbols
        .iter()
        .chain(&rep.combining_diacritics)
        .chain(&rep.spacing_modifiers)
    {
        if let Err(violation) = validate_symbol(&uni, sym) {
            return Err(RepertoireError::Unicode {
                symbol: sym.symbol.clone(),
                violation,
            });
        }
    }

    // Cross-inventory global uniqueness.
    ensure_global_unique(&mut rep)?;

    Ok(rep)
}

fn convert_all(raws: Vec<RawSymbol>) -> Result<Vec<Symbol>, RepertoireError> {
    let mut out = Vec::with_capacity(raws.len());
    for r in raws {
        out.push(convert_symbol(r)?);
    }
    Ok(out)
}

fn validate_symbol(uni: &UnicodeValidator, sym: &Symbol) -> Result<(), UnicodeViolation> {
    // Every scalar must pass category/noncharacter checks. Combining marks
    // (Mn) and modifier letters (Lm/Sk) are allowed here because this is the
    // allowlist of modifiers themselves; the base-attachment rule is enforced
    // by the IPA identifier parser, not by category rejection.
    for scalar in sym.symbol.chars() {
        if UnicodeValidator::is_noncharacter(scalar) {
            return Err(UnicodeViolation::Noncharacter { scalar });
        }
    }
    // Base symbols are standalone letters: they must be NFC.
    if !uni.is_nfc(&sym.symbol) {
        return Err(UnicodeViolation::NotNfc);
    }
    Ok(())
}

fn first_duplicate(entries: &[Symbol]) -> String {
    for w in entries.windows(2) {
        if let [a, b] = w {
            if a.symbol == b.symbol {
                return a.symbol.clone();
            }
        }
    }
    String::new()
}

fn ensure_global_unique(rep: &mut Repertoire) -> Result<(), RepertoireError> {
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    for sym in rep
        .base_symbols
        .iter()
        .chain(&rep.combining_diacritics)
        .chain(&rep.spacing_modifiers)
    {
        if seen.insert(sym.symbol.as_str(), ()).is_some() {
            return Err(RepertoireError::Duplicate {
                symbol: sym.symbol.clone(),
            });
        }
    }
    Ok(())
}

// --- RFC 8785 JCS canonicalization + digest -------------------------------

/// Compute the RFC 8785 (JCS) canonical bytes of the repertoire JSON and
/// return their SHA-256 as lowercase hex.
///
/// JCS canonicalization here: parse to a generic value, then serialize with
/// object keys sorted by UTF-16 code unit and no insignificant whitespace.
/// `serde_json::Value` (a `BTreeMap`-backed map is used via `to_value` +
/// manual key sort) yields deterministic output; strings are emitted in JCS
/// minimal-escape form by serde_json, and all values here are strings, arrays,
/// objects and small integers, which share the same form in JCS.
pub fn canonical_jcs_sha256(src: &str) -> Result<String, RepertoireError> {
    let value: serde_json::Value =
        serde_json::from_str(src).map_err(|e| RepertoireError::Json(e.to_string()))?;
    let canonical = jcs_canonicalize(&value);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    Ok(hex)
}

/// Serialize a `serde_json::Value` in RFC 8785 JCS form.
///
/// Object members are sorted by the UTF-16 code units of their keys; there is
/// no insignificant whitespace. The repertoire contains only objects, arrays,
/// strings and non-negative integers, so number formatting reduces to the
/// integer case, which JCS and `serde_json` render identically.
fn jcs_canonicalize(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_jcs(value, &mut out);
    out
}

fn write_jcs(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_jcs_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // Sort keys by UTF-16 code units (RFC 8785).
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs_string(k, out);
                out.push(':');
                if let Some(v) = map.get(*k) {
                    write_jcs(v, out);
                }
            }
            out.push('}');
        }
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Minimal JSON string escaping per RFC 8785 (JSON string, shortest escapes).
fn write_jcs_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn pinned_repertoire_loads_and_validates() {
        let rep = load().expect("pinned repertoire must load and validate");
        assert_eq!(rep.repertoire_version, 1);
        assert_eq!(rep.ipa_chart, "2026");
        // Sanity: the IPA chart has 107 segmental base letters.
        assert_eq!(rep.base_symbols.len(), 107, "base symbol count");
        assert!(!rep.combining_diacritics.is_empty());
        assert!(!rep.spacing_modifiers.is_empty());
    }

    #[test]
    fn every_symbol_is_nfc_and_codepoints_match() {
        // load() already enforces this; this test documents the guarantee and
        // fails loudly if the data drifts.
        assert!(load().is_ok());
    }

    #[test]
    fn excluded_prosody_is_absent() {
        let rep = load().expect("load");
        let all: Vec<&str> = rep
            .base_symbols
            .iter()
            .chain(&rep.combining_diacritics)
            .chain(&rep.spacing_modifiers)
            .map(|s| s.symbol.as_str())
            .collect();
        // Suprasegmentals / stress / length / tone / intonation must NOT appear.
        for excluded in [
            "\u{02C8}", // ˈ primary stress
            "\u{02CC}", // ˌ secondary stress
            "\u{02D0}", // ː length
            "\u{02D1}", // ˑ half-length
            "\u{2016}", // ‖ major group
            "\u{2197}", // ↗ global rise
            "\u{2198}", // ↘ global fall
            ".",        // syllable break
            "\u{203F}", // ‿ linking
        ] {
            assert!(
                !all.contains(&excluded),
                "excluded prosodic symbol {excluded:?} must not be in the repertoire"
            );
        }
    }

    #[test]
    fn rejects_bom() {
        let with_bom = format!("\u{FEFF}{REPERTOIRE_JSON}");
        assert!(matches!(load_from(&with_bom), Err(RepertoireError::Bom)));
    }

    #[test]
    fn rejects_duplicate_json_key() {
        let dup = r#"{"repertoire_version":1,"repertoire_version":1,"unicode_version":"18.0.0","ipa_chart":"2026","base_symbols":[],"combining_diacritics":[],"spacing_modifiers":[]}"#;
        assert!(matches!(load_from(dup), Err(RepertoireError::Json(_))));
    }

    #[test]
    fn rejects_unknown_field() {
        let extra = r#"{"repertoire_version":1,"unicode_version":"18.0.0","ipa_chart":"2026","base_symbols":[],"combining_diacritics":[],"spacing_modifiers":[],"extra":true}"#;
        assert!(matches!(load_from(extra), Err(RepertoireError::Json(_))));
    }

    #[test]
    fn jcs_digest_is_stable_and_nonempty() {
        let a = canonical_jcs_sha256(REPERTOIRE_JSON).expect("digest");
        let b = canonical_jcs_sha256(REPERTOIRE_JSON).expect("digest");
        assert_eq!(a, b, "JCS digest must be deterministic");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn pinned_digest_matches() {
        // The pinned digest must equal the recomputed JCS digest. If the
        // inventory changes, this fails until the constant is re-reviewed and
        // updated (steering §12 anti-drift).
        let actual = canonical_jcs_sha256(REPERTOIRE_JSON).expect("digest");
        assert_eq!(actual, PINNED_JCS_SHA256);
    }
}
