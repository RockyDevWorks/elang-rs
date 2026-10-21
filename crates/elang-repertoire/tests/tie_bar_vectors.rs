//! Conformance test over the published IPA tie-bar equivalence and
//! comparison-key vectors (T0117).
//!
//! The authoritative, human-reviewable corpus lives at
//! `specs/registry/ipa-tie-bar-vectors.json` (a sibling of this crate, reached
//! via `CARGO_MANIFEST_DIR`). This test loads that file, drives
//! [`elang_repertoire::ipa::validate`] and
//! [`elang_repertoire::ipa::IpaIdentifier::comparison_key`] over every vector,
//! and asserts the published expectations. Keeping the vectors in a data file
//! (rather than only inline unit tests) makes them independently checkable and
//! reproducible; every row of steering §7.1 is exercised here.
//!
//! The inline unit tests in `src/ipa.rs` remain valid; this file is the
//! published, machine-checked corpus.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::PathBuf;

use elang_repertoire::ipa::{self, IpaError};
use elang_repertoire::repertoire::{self, Repertoire};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorFile {
    metadata: Metadata,
    positive: Vec<PositiveVector>,
    equivalence: Vec<EquivalenceVector>,
    negative: Vec<NegativeVector>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    vector_set: String,
    vector_set_version: u32,
    repertoire_version: u32,
    unicode_version: String,
    ipa_chart: String,
    #[allow(dead_code)]
    repertoire_source: String,
    #[allow(dead_code)]
    repertoire_pinned_jcs_sha256_constant: String,
    #[allow(dead_code)]
    provenance: String,
    #[allow(dead_code)]
    requirements: Vec<String>,
    #[allow(dead_code)]
    steering_reference: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PositiveVector {
    name: String,
    input: String,
    input_codepoints: Vec<String>,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    requirements: Vec<String>,
    expected_unit_count: usize,
    #[serde(default)]
    expected_tie_codepoint: Option<String>,
    #[serde(default)]
    expected_modifier_codepoint: Option<String>,
    expected_original_codepoints: Vec<String>,
    expected_comparison_key_codepoints: Vec<String>,
    #[allow(dead_code)]
    steering_row: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EquivalenceVector {
    name: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    requirements: Vec<String>,
    a: String,
    a_codepoints: Vec<String>,
    b: String,
    b_codepoints: Vec<String>,
    same_comparison_key: bool,
    #[allow(dead_code)]
    steering_row: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NegativeVector {
    name: String,
    input: String,
    input_codepoints: Vec<String>,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    requirements: Vec<String>,
    expected_error: String,
    expected_error_code: String,
    #[allow(dead_code)]
    steering_row: String,
}

/// `U+XXXX` -> `char`.
fn cp(s: &str) -> char {
    let hex = s.strip_prefix("U+").expect("codepoint must start with U+");
    let n = u32::from_str_radix(hex, 16).expect("valid hex codepoint");
    char::from_u32(n).expect("valid Unicode scalar")
}

/// Rebuild a string from a `U+XXXX` codepoint list.
fn cps_to_string(list: &[String]) -> String {
    list.iter().map(|s| cp(s)).collect()
}

fn load_vectors() -> VectorFile {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/elang-repertoire -> workspace root -> specs/registry/...
    path.push("../../specs/registry/ipa-tie-bar-vectors.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read vector file {}: {e}", path.display()));
    // Vectors are strict UTF-8 with no BOM.
    assert!(
        !src.starts_with('\u{FEFF}'),
        "vector file must not start with a BOM"
    );
    serde_json::from_str(&src).expect("vector file must parse")
}

fn rep() -> Repertoire {
    repertoire::load().expect("pinned repertoire must load")
}

/// Map an `IpaError` to the human-readable variant name used in the vectors.
fn error_variant_name(e: &IpaError) -> &'static str {
    match e {
        IpaError::Empty => "Empty",
        IpaError::TooManyScalars(_) => "TooManyScalars",
        IpaError::TooManyUnits(_) => "TooManyUnits",
        IpaError::BaseNotApproved(_) => "BaseNotApproved",
        IpaError::ModifierNotApproved(_) => "ModifierNotApproved",
        IpaError::StandaloneModifier(_) => "StandaloneModifier",
        IpaError::TooManyModifications => "TooManyModifications",
        IpaError::TieBarInvalid => "TieBarInvalid",
        IpaError::Unicode(_) => "Unicode",
    }
}

#[test]
fn vector_file_metadata_matches_pinned_repertoire() {
    let vf = load_vectors();
    let r = rep();
    assert_eq!(vf.metadata.vector_set, "ipa-tie-bar-vectors");
    assert_eq!(vf.metadata.vector_set_version, 1);
    assert_eq!(vf.metadata.repertoire_version, r.repertoire_version);
    assert_eq!(vf.metadata.unicode_version, r.unicode_version);
    assert_eq!(vf.metadata.ipa_chart, r.ipa_chart);
}

#[test]
fn positive_vectors_validate_as_published() {
    let vf = load_vectors();
    let r = rep();
    assert!(!vf.positive.is_empty(), "expected positive vectors");

    for v in &vf.positive {
        // The literal `input` and its `input_codepoints` must agree, so the
        // human-readable escapes are provably the same bytes under test.
        assert_eq!(
            v.input,
            cps_to_string(&v.input_codepoints),
            "[{}] input literal must match its codepoint list",
            v.name
        );

        let id = ipa::validate(&v.input, &r)
            .unwrap_or_else(|e| panic!("[{}] expected valid, got {e:?}", v.name));

        assert_eq!(
            id.units.len(),
            v.expected_unit_count,
            "[{}] logical unit count",
            v.name
        );

        // `original` preserves the authored NFC bytes, including the tie-bar
        // variant (REQ-IPA-013).
        assert_eq!(
            id.original,
            cps_to_string(&v.expected_original_codepoints),
            "[{}] original must be preserved byte-for-byte",
            v.name
        );

        // Comparison key normalizes the lower tie bar to the upper one
        // (REQ-IPA-012).
        assert_eq!(
            id.comparison_key(),
            cps_to_string(&v.expected_comparison_key_codepoints),
            "[{}] comparison key",
            v.name
        );

        if let Some(tie) = &v.expected_tie_codepoint {
            let tied_unit = id
                .units
                .iter()
                .find(|u| u.tie.is_some())
                .unwrap_or_else(|| panic!("[{}] expected a tied unit", v.name));
            assert_eq!(
                tied_unit.tie,
                Some(cp(tie)),
                "[{}] authored tie-bar variant preserved on the unit",
                v.name
            );
        }

        if let Some(m) = &v.expected_modifier_codepoint {
            let has_mod = id.units.iter().any(|u| u.modifier == Some(cp(m)));
            assert!(has_mod, "[{}] expected modifier {m} on a unit", v.name);
        }
    }
}

#[test]
fn equivalence_vectors_hold() {
    let vf = load_vectors();
    let r = rep();
    assert!(!vf.equivalence.is_empty(), "expected equivalence vectors");

    for v in &vf.equivalence {
        assert_eq!(
            v.a,
            cps_to_string(&v.a_codepoints),
            "[{}] operand a literal must match its codepoints",
            v.name
        );
        assert_eq!(
            v.b,
            cps_to_string(&v.b_codepoints),
            "[{}] operand b literal must match its codepoints",
            v.name
        );

        let a = ipa::validate(&v.a, &r)
            .unwrap_or_else(|e| panic!("[{}] operand a invalid: {e:?}", v.name));
        let b = ipa::validate(&v.b, &r)
            .unwrap_or_else(|e| panic!("[{}] operand b invalid: {e:?}", v.name));

        if v.same_comparison_key {
            assert_eq!(
                a.comparison_key(),
                b.comparison_key(),
                "[{}] operands must share a comparison key",
                v.name
            );
            // Distinct authored forms are still preserved verbatim.
            assert_ne!(
                a.original, b.original,
                "[{}] equivalent-but-distinct authored forms must be preserved",
                v.name
            );
        } else {
            assert_ne!(
                a.comparison_key(),
                b.comparison_key(),
                "[{}] operands must be distinct identities",
                v.name
            );
        }
    }
}

#[test]
fn negative_vectors_reject_as_published() {
    let vf = load_vectors();
    let r = rep();
    assert!(!vf.negative.is_empty(), "expected negative vectors");

    for v in &vf.negative {
        assert_eq!(
            v.input,
            cps_to_string(&v.input_codepoints),
            "[{}] input literal must match its codepoint list",
            v.name
        );

        let err = match ipa::validate(&v.input, &r) {
            Ok(id) => panic!("[{}] expected rejection, got valid {:?}", v.name, id.units),
            Err(e) => e,
        };

        assert_eq!(
            error_variant_name(&err),
            v.expected_error,
            "[{}] error variant",
            v.name
        );
        assert_eq!(
            err.code(),
            v.expected_error_code,
            "[{}] stable error code",
            v.name
        );
    }
}

#[test]
fn every_steering_7_1_row_is_covered() {
    // Guard: the published corpus must cover all ten §7.1 rows. Six are
    // positive (tʃ, t͡ʃ, t͜ʃ, t͡ʃa, t͡ʃd͡ʒ, t͡ʃʰ) and four are negative
    // (tʰ͡ʃ, tʰʷ, ʰ, isolated U+0303). The tie-equivalence of t͜ʃ to t͡ʃ is
    // additionally asserted as an equivalence vector.
    let vf = load_vectors();
    assert_eq!(vf.positive.len(), 6, "§7.1 positive rows");
    assert_eq!(vf.negative.len(), 4, "§7.1 negative rows");
    assert_eq!(
        vf.equivalence.len(),
        2,
        "tie equivalence + tied/untied distinct"
    );
}
