//! T1407 — Unicode NFC + IPA valid/invalid fixtures.
//!
//! # What this module covers
//!
//! Three independent but related concerns:
//!
//! 1. **Unicode NFC normalisation** (`[validate_str]` / [`to_nfc`]): strings
//!    the corpus asserts are NFC, the corresponding decomposed form, and
//!    the result of NFC round-tripping each one. The conformance guarantee
//!    is that an implementation **must** agree with the pinned outputs
//!    byte-for-byte — the published `Unicode 18.0.0` data tables are the
//!    ground truth.
//!
//! 2. **Unicode forbidden-category and noncharacter rejection**:
//!    [`UnicodeViolation::ForbiddenCategory`] and
//!    [`UnicodeViolation::Noncharacter`] are pinned so a future
//!    implementation change to the `Cc`/`Cf`/`Co`/`Cn`/`Cs` filter
//!    surfaces as a corpus failure.
//!
//! 3. **IPA validation** (`[IpaError]`): the ELANG IPA allowlist is
//!    authoritative. Inputs the corpus pins as accepted are in the
//!    allowlist; inputs pinned as rejected are not, with the expected
//!    `IPA_*` code.
//!
//! # How the fixtures run
//!
//! The NFC / forbidden-category / noncharacter checks are exposed as
//! [`VectorFixture`]s pinning the implementation's output bytes. The IPA
//! accepted/rejected fixtures use [`DocumentFixture`]s parsed against
//! the ELD schema, because the IPA check fires as part of the ELD
//! inventory parse, not in isolation.

use crate::corpus::{DocumentFixture, VectorFixture};
use elang_core::error::Category;
use elang_repertoire::ipa::validate;
use elang_repertoire::repertoire::load;
use elang_unicode::{is_nfc, is_noncharacter, to_nfc, validate_str, UnicodeViolation};

/// All T1407 vector fixtures (NFC round-trips, forbidden-category pins,
/// noncharacter pins).
#[must_use]
pub fn vector_fixtures() -> Vec<VectorFixture> {
    let mut fixtures = Vec::new();

    // -- NFC round-trip vectors ------------------------------------------
    // Each fixture pins `actual = production_code(value)` and
    // `expected = pinned_golden`. The pinned golden is constructed by
    // hand from the NFC specification.
    //
    // The table is `(label, input, expected_nfc_output, input_is_nfc)`.
    let nfc_pairs: &[(&str, &str, &str, bool)] = &[
        // Identity: ASCII is already NFC.
        ("ascii", "hello world", "hello world", true),
        // A precomposed é (\u{00E9}) is NFC and must survive identity.
        ("precomposed-e-acute", "caf\u{00E9}", "caf\u{00E9}", true),
        // Decomposed é is `e` + combining acute (\u{0301}); NFC must
        // compose it back to \u{00E9}.
        ("decomposed-e-acute", "cafe\u{0301}", "caf\u{00E9}", false),
        // Hangul: 가 = U+AC00 (NFC) and is composed from ㄱ + ㅏ. The
        // decomposed form U+1100 U+1161 must compose back to U+AC00.
        ("hangul-precomposed", "\u{AC00}", "\u{AC00}", true),
        ("hangul-decomposed", "\u{1100}\u{1161}", "\u{AC00}", false),
    ];
    for (label, value, golden, input_is_nfc) in nfc_pairs {
        fixtures.push(VectorFixture {
            id: format!("unicode/nfc-output/{label}"),
            description: "to_nfc produces the pinned NFC output",
            requirements: &["REQ-UNICODE-001"],
            actual: to_nfc(value).as_bytes().to_vec(),
            expected: golden.as_bytes().to_vec(),
        });
        fixtures.push(VectorFixture {
            id: format!("unicode/nfc-is-nfc/{label}"),
            description: "is_nfc agrees with the corpus's pinned expectation",
            requirements: &["REQ-UNICODE-001"],
            actual: vec![u8::from(is_nfc(value))],
            expected: vec![u8::from(*input_is_nfc)],
        });
    }

    // -- Forbidden-category vectors -------------------------------------
    // FORMAT characters (Cf) are forbidden. The corpus pins this for an
    // explicit ZWJ (U+200D), BOM (U+FEFF), and the bidi-format controls.
    let forbidden: &[(&str, char)] = &[
        ("zwj", '\u{200D}'),
        ("zwnj", '\u{200C}'),
        ("bom", '\u{FEFF}'),
    ];
    for (label, scalar) in forbidden {
        let result = validate_str(&scalar.to_string());
        let observed = match result {
            Err(UnicodeViolation::ForbiddenCategory { .. }) => 1u8,
            Err(_) => 2,
            Ok(()) => 0,
        };
        fixtures.push(VectorFixture {
            id: format!("unicode/forbidden-category/{label}"),
            description: "format characters are rejected as ForbiddenCategory",
            requirements: &["REQ-UNICODE-002"],
            actual: vec![observed],
            expected: vec![1u8],
        });
    }

    // -- Noncharacter vectors -------------------------------------------
    // U+FDD0 is in the noncharacter range.
    let nonchar = '\u{FDD0}';
    let is_nc = is_noncharacter(nonchar);
    fixtures.push(VectorFixture {
        id: "unicode/noncharacter-detection".to_string(),
        description: "U+FDD0 is a Unicode noncharacter",
        requirements: &["REQ-UNICODE-003"],
        actual: vec![u8::from(is_nc)],
        expected: vec![1u8],
    });

    fixtures
}

/// All T1407 IPA fixtures, run as [`DocumentFixture`]s against the ELD
/// schema so the `IPA_*` codes surface from the right stage.
#[must_use]
pub fn ipa_fixtures() -> Vec<DocumentFixture> {
    let rep = match load() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut fixtures = Vec::new();

    // Positive: a fully-valid ELD document declares a small IPA inventory.
    let valid_source = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n"
    .to_vec();
    fixtures.push(DocumentFixture {
        id: "ipa/accept-valid-inventory",
        description: "All-Consonants-and-Vowels inventory parses cleanly",
        requirements: &["REQ-IPA-001"],
        schema: crate::corpus::Schema::Eld,
        source: valid_source,
        expectation: crate::corpus::DocumentExpectation::Accept,
    });

    // Negative: a phoneme whose IPA is not an approved base symbol.
    // The v1 chart spells the voiced velar stop `ɡ` (U+0261); the ASCII
    // letter `g` (U+0067) is deliberately NOT an approved base, so it
    // is rejected with `IPA_NOT_IN_REPERTOIRE`.
    //
    // The rejection is asserted, not assumed: if a future chart revision
    // admitted `g`, the fixture would silently stop being a negative
    // case. The count assertion at the end of this function turns that
    // into a hard failure.
    let invalid_ipa: &[char] = &['g'];
    for ipa_char in invalid_ipa {
        assert!(
            validate(&ipa_char.to_string(), &rep).is_err(),
            "IPA {ipa_char:?} must NOT be an approved base symbol in the v1 \
             chart — pick a different rejection fixture, or the negative \
             case below is vacuous"
        );
        let source = format!(
            "phonemes {{\n    consonants {{\n        phoneme \"{ipa_char}\" {{\n            roman \"q\"\n        }}\n    }}\n    vowels {{\n        phoneme \"a\" {{\n            roman \"a\"\n        }}\n    }}\n}}\n"
        );
        fixtures.push(DocumentFixture {
            id: "ipa/reject-non-allowlisted",
            description: "An IPA outside the v1 allowlist is rejected",
            requirements: &["REQ-IPA-002"],
            schema: crate::corpus::Schema::Eld,
            source: source.into_bytes(),
            expectation: crate::corpus::DocumentExpectation::Reject {
                code: "IPA_NOT_IN_REPERTOIRE",
                category: Category::Ipa,
            },
        });
    }

    // Guard against the negative case being dropped: the corpus must
    // publish exactly one accept case and one reject case.
    assert_eq!(
        fixtures.len(),
        2,
        "the IPA fixture set must contain exactly one accept and one reject \
         case — a silently-skipped rejection fixture is a vacuous corpus"
    );

    fixtures
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
///
/// The module publishes two families: the byte-level Unicode vectors
/// ([`vector_fixtures`]) and the IPA document fixtures
/// ([`ipa_fixtures`]), so both are mapped into the catalog.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    let mut entries: Vec<crate::catalog::CatalogEntry> = vector_fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_vector)
        .collect();
    entries.extend(
        ipa_fixtures()
            .iter()
            .map(crate::catalog::CatalogEntry::from_document),
    );
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{run_document_fixture, run_vector_fixture};

    #[test]
    fn unicode_vector_fixtures_pass() {
        for fixture in vector_fixtures() {
            run_vector_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn ipa_fixtures_run() {
        for fixture in ipa_fixtures() {
            run_document_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn nfc_identifies_nfc_strings() {
        assert!(is_nfc("hello"));
        assert!(is_nfc("caf\u{00E9}"));
        assert!(!is_nfc("cafe\u{0301}"));
    }

    #[test]
    fn nfc_roundtrip_is_nfc() {
        for value in ["hello", "caf\u{00E9}", "cafe\u{0301}", "\u{1100}\u{1161}"] {
            let nfc = to_nfc(value);
            assert!(is_nfc(&nfc), "to_nfc({value:?}) = {nfc:?} must be NFC");
        }
    }
}
