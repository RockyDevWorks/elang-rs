//! T1408 + T1413 + T1414 — ELD structure/uniqueness, EPF negatives, ELD
//! negatives.
//!
//! # T1408 — group / roman / allophone structure and uniqueness
//!
//! The ELD schema layer owns four structural guarantees:
//!
//! 1. The mandatory groups `consonants` and `vowels` must both be present
//!    (`ELD_GROUP_INVALID`).
//! 2. A group name must not repeat (`ELD_GROUP_DUPLICATE`).
//! 3. A romanisation must be unique across the whole document, compared
//!    case-insensitively (`ROMAN_DUPLICATE`).
//! 4. An allophone's IPA must not collide with another IPA in the same
//!    scope (`ELD_IPA_DUPLICATE`).
//!
//! # T1413 — EPF negatives
//!
//! The EPF schema layer owns the version, identifier, and manifest
//! contracts:
//!
//! - `EPF_BAD_VERSION` — the `epf`/`eld` version strings are not the
//!   supported `0.1.0`.
//! - `EPF_IDENTIFIER_BAD_URN` — the metadata identifier is not a
//!   well-formed `urn:elang:language:...` URN.
//! - `EPF_BAD_METADATA_FIELD` — an unknown or malformed metadata field.
//! - `EPF_MANIFEST_PATH_NOT_INDEXED` — the manifest references a path
//!   that the package's index does not contain.
//!
//! # T1414 — ELD negatives
//!
//! - `ELD_GROUP_INVALID` — a mandatory group is missing, or a group
//!   appears twice.
//! - `ELD_IPA_DUPLICATE` — the same IPA is declared twice.
//! - `ELD_IPA_DUPLICATE` — an allophone reuses its root's IPA.
//! - `ROMAN_DUPLICATE` — two phonemes share a romanisation.
//! - `ELD_GROUP_EMPTY` — a declared group has no members.
//!
//! # Fixture construction rules
//!
//! Every fixture is built with `String::push_str` over a single-line
//! template — never a `format!` string containing KDL braces, and never
//! a Rust line-continuation string. Both patterns silently corrupt the
//! KDL shape (a brace-balance error changes which validator fires, and a
//! `\` continuation collapses sibling nodes into one node's argument
//! list). The builders below use explicit `push_str` calls so each
//! node occupies exactly one line.

use crate::corpus::{DocumentExpectation, DocumentFixture, Schema};
use elang_core::error::Category;

// =======================================================================
// Builders
// =======================================================================

/// A minimal, valid ELD document with the given consonants and vowels.
///
/// `consonants` and `vowels` are `(ipa, roman)` pairs; the romanisation
/// must be unique across the whole document.
fn eld_with(consonants: &[(&str, &str)], vowels: &[(&str, &str)]) -> Vec<u8> {
    let mut src = String::from("phonemes {\n");
    for (group, entries) in [("consonants", consonants), ("vowels", vowels)] {
        src.push_str(&format!("    {group} {{\n"));
        for (ipa, roman) in entries {
            src.push_str(&format!("        phoneme \"{ipa}\" {{\n"));
            src.push_str(&format!("            roman \"{roman}\"\n"));
            src.push_str("        }\n");
        }
        src.push_str("    }\n");
    }
    src.push_str("}\n");
    src.into_bytes()
}

/// The minimal valid ELD used as the base for structural mutations.
fn base_eld() -> Vec<u8> {
    eld_with(&[("p", "p")], &[("a", "a")])
}

/// A minimal, valid EPF document with the given `epf`/`eld` version and
/// metadata identifier.
fn epf_with(epf_version: &str, eld_version: &str, identifier: &str) -> Vec<u8> {
    let mut src = String::new();
    src.push_str(&format!("epf \"{epf_version}\"\n"));
    src.push_str(&format!("eld \"{eld_version}\"\n"));
    src.push_str("language-version \"1.0.0\"\n");
    src.push_str("metadata {\n");
    src.push_str(&format!("    identifier \"{identifier}\"\n"));
    src.push_str("    name \"Conformance Probe\"\n");
    src.push_str("}\n");
    src.push_str("manifest {\n");
    src.push_str("    phonemes \"Linguistics/phonemes.eld\"\n");
    src.push_str("}\n");
    src.into_bytes()
}

// =======================================================================
// T1408 — ELD structural and uniqueness fixtures
// =======================================================================

/// The T1408 positive and negative ELD fixtures.
#[must_use]
pub fn eld_structure_fixtures() -> Vec<DocumentFixture> {
    let mut fixtures = Vec::new();

    // -- Positive: the minimal document is valid ----------------------
    fixtures.push(DocumentFixture {
        id: "eld/accept-minimal",
        description: "A minimal consonants+vowels document parses cleanly",
        requirements: &["REQ-ELD-001", "REQ-ELD-003", "REQ-ELD-006"],
        schema: Schema::Eld,
        source: base_eld(),
        expectation: DocumentExpectation::Accept,
    });

    // -- Positive: an extra custom group is allowed -------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n            roman \"p\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("    nasals {\n");
        src.push_str("        phoneme \"\u{014b}\" {\n            roman \"ng\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/accept-extra-group",
            description: "A custom group alongside the mandatory ones parses cleanly",
            requirements: &["REQ-ELD-002", "REQ-ELD-004"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Accept,
        });
    }

    // -- Negative: a group name repeats -------------------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n            roman \"p\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"t\" {\n            roman \"t\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/reject-duplicate-group",
            description: "A repeated group name is rejected as ELD_GROUP_DUPLICATE",
            requirements: &["REQ-ELD-003", "REQ-ELD-005"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "ELD_GROUP_INVALID",
                category: Category::EldSchema,
            },
        });
    }

    // -- Negative: two phonemes share a romanisation ------------------
    {
        let src = eld_with(&[("p", "p"), ("t", "p")], &[("a", "a")]);
        fixtures.push(DocumentFixture {
            id: "eld/reject-duplicate-roman",
            description: "Two phonemes sharing a romanisation are rejected",
            requirements: &["REQ-ELD-013"],
            schema: Schema::Eld,
            source: src,
            expectation: DocumentExpectation::Reject {
                code: "ROMAN_DUPLICATE",
                category: Category::Roman,
            },
        });
    }

    // -- Negative: two phonemes share an IPA --------------------------
    {
        let src = eld_with(&[("p", "p"), ("p", "q")], &[("a", "a")]);
        fixtures.push(DocumentFixture {
            id: "eld/reject-duplicate-ipa",
            description: "Two phonemes sharing an IPA are rejected",
            requirements: &["REQ-ELD-015"],
            schema: Schema::Eld,
            source: src,
            expectation: DocumentExpectation::Reject {
                code: "ELD_IPA_DUPLICATE",
                category: Category::EldSchema,
            },
        });
    }

    // -- Positive: an allophone with its own IPA and roman -----------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            allophone \"p\u{02b0}\" {\n");
        src.push_str("                roman \"ph\"\n");
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/accept-allophone",
            description: "An allophone with a distinct IPA and roman parses cleanly",
            requirements: &["REQ-ELD-009", "REQ-ELD-010", "REQ-ELD-011"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Accept,
        });
    }

    // -- Negative: an allophone reuses its root's IPA ------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            allophone \"p\" {\n");
        src.push_str("                roman \"ph\"\n");
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/reject-allophone-duplicate-ipa",
            description: "An allophone reusing its root's IPA is rejected",
            requirements: &["REQ-ELD-015"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "ELD_IPA_DUPLICATE",
                category: Category::EldSchema,
            },
        });
    }

    fixtures
}

// =======================================================================
// T1413 — EPF negatives
// =======================================================================

/// The T1413 EPF positive and negative fixtures.
#[must_use]
pub fn epf_fixtures() -> Vec<DocumentFixture> {
    let mut fixtures = Vec::with_capacity(8);

    // -- Positive: the minimal document is valid ----------------------
    fixtures.push(DocumentFixture {
        id: "epf/accept-minimal",
        description: "A minimal EPF document parses cleanly",
        requirements: &["REQ-EPF-001", "REQ-EPF-003", "REQ-EPF-005"],
        schema: Schema::Epf,
        source: epf_with(
            "0.1.0",
            "0.1.0",
            "urn:elang:language:rocky:conformance:1.0.0",
        ),
        expectation: DocumentExpectation::Accept,
    });

    // -- Negative: the `epf` version is not supported ------------------
    fixtures.push(DocumentFixture {
        id: "epf/reject-wrong-version",
        description: "An unsupported `epf` version is rejected as EPF_BAD_VERSION",
        requirements: &["REQ-EPF-004"],
        schema: Schema::Epf,
        source: epf_with(
            "0.2.0",
            "0.1.0",
            "urn:elang:language:rocky:conformance:1.0.0",
        ),
        expectation: DocumentExpectation::Reject {
            code: "EPF_BAD_VERSION",
            category: Category::EpfSchema,
        },
    });

    // -- Negative: the `eld` version is not supported ------------------
    fixtures.push(DocumentFixture {
        id: "epf/reject-wrong-eld-version",
        description: "An unsupported `eld` version is rejected as EPF_BAD_VERSION",
        requirements: &["REQ-EPF-004"],
        schema: Schema::Epf,
        source: epf_with(
            "0.1.0",
            "9.9.9",
            "urn:elang:language:rocky:conformance:1.0.0",
        ),
        expectation: DocumentExpectation::Reject {
            code: "EPF_BAD_VERSION",
            category: Category::EpfSchema,
        },
    });

    // -- Negative: the identifier is not an ELANG URN ------------------
    fixtures.push(DocumentFixture {
        id: "epf/reject-bad-urn",
        description: "A non-ELANG identifier URN is rejected",
        requirements: &["REQ-EPF-006"],
        schema: Schema::Epf,
        source: epf_with(
            "0.1.0",
            "0.1.0",
            "urn:example:language:rocky:conformance:1.0.0",
        ),
        expectation: DocumentExpectation::Reject {
            code: "EPF_BAD_URN",
            category: Category::EpfSchema,
        },
    });

    // -- Negative: an unknown metadata field --------------------------
    {
        let mut src = String::new();
        src.push_str("epf \"0.1.0\"\n");
        src.push_str("eld \"0.1.0\"\n");
        src.push_str("language-version \"1.0.0\"\n");
        src.push_str("metadata {\n");
        src.push_str("    identifier \"urn:elang:language:rocky:conformance:1.0.0\"\n");
        src.push_str("    name \"Conformance Probe\"\n");
        src.push_str("    unknown-field \"value\"\n");
        src.push_str("}\n");
        src.push_str("manifest {\n");
        src.push_str("    phonemes \"Linguistics/phonemes.eld\"\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "epf/reject-unknown-metadata-field",
            description: "An unknown metadata field is rejected as EPF_BAD_METADATA_FIELD",
            requirements: &["REQ-EPF-011"],
            schema: Schema::Epf,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "EPF_UNKNOWN_NODE",
                category: Category::EpfSchema,
            },
        });
    }

    // -- Negative: the metadata name is empty --------------------------
    // An empty `name` is rejected; the crate remaps its internal
    // `EPF_NAME_EMPTY` onto the registry-canonical
    // `EPF_BAD_METADATA_FIELD`.
    {
        let mut src = String::new();
        src.push_str("epf \"0.1.0\"\n");
        src.push_str("eld \"0.1.0\"\n");
        src.push_str("language-version \"1.0.0\"\n");
        src.push_str("metadata {\n");
        src.push_str("    identifier \"urn:elang:language:rocky:conformance:1.0.0\"\n");
        src.push_str("    name \"\"\n");
        src.push_str("}\n");
        src.push_str("manifest {\n");
        src.push_str("    phonemes \"Linguistics/phonemes.eld\"\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "epf/reject-empty-name",
            description: "An empty metadata name is rejected as EPF_BAD_METADATA_FIELD",
            requirements: &["REQ-EPF-009"],
            schema: Schema::Epf,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "EPF_BAD_METADATA_FIELD",
                category: Category::EpfSchema,
            },
        });
    }

    // -- Negative: the manifest path is not a `.eld` file -------------
    // `EPF_MANIFEST_PATH_NOT_INDEXED` fires for path shape problems (a
    // path that does not end with `.eld`); the "not in archive index"
    // half of C-12 is enforced by the reader's integrity stage, not by
    // the EPF schema.
    {
        let mut src = String::new();
        src.push_str("epf \"0.1.0\"\n");
        src.push_str("eld \"0.1.0\"\n");
        src.push_str("language-version \"1.0.0\"\n");
        src.push_str("metadata {\n");
        src.push_str("    identifier \"urn:elang:language:rocky:conformance:1.0.0\"\n");
        src.push_str("    name \"Conformance Probe\"\n");
        src.push_str("}\n");
        src.push_str("manifest {\n");
        src.push_str("    phonemes \"Linguistics/not-eld.txt\"\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "epf/reject-manifest-path-not-eld",
            description: "A manifest path without the `.eld` extension is rejected",
            requirements: &["REQ-EPF-012"],
            schema: Schema::Epf,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "EPF_MANIFEST_PATH_NOT_INDEXED",
                category: Category::EpfSchema,
            },
        });
    }

    fixtures
}

// =======================================================================
// T1414 — ELD negatives
// =======================================================================

/// The T1414 ELD negative fixtures.
#[must_use]
pub fn eld_negative_fixtures() -> Vec<DocumentFixture> {
    let mut fixtures = Vec::new();

    // -- Missing the `vowels` group ------------------------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n            roman \"p\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/reject-missing-vowels-group",
            description: "A document without the mandatory `vowels` group is rejected",
            requirements: &["REQ-ELD-003"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "ELD_GROUP_INVALID",
                category: Category::EldSchema,
            },
        });
    }

    // -- Missing the `consonants` group -------------------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/reject-missing-consonants-group",
            description: "A document without the mandatory `consonants` group is rejected",
            requirements: &["REQ-ELD-003"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "ELD_GROUP_INVALID",
                category: Category::EldSchema,
            },
        });
    }

    // -- An empty group ------------------------------------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n            roman \"p\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "eld/reject-empty-group",
            description: "A declared group with no members is rejected",
            requirements: &["REQ-ELD-006"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "ELD_GROUP_INVALID",
                category: Category::EldSchema,
            },
        });
    }

    fixtures
}

/// Every document fixture in this module, in corpus order.
#[must_use]
pub fn all_fixtures() -> Vec<DocumentFixture> {
    let mut all = eld_structure_fixtures();
    all.extend(epf_fixtures());
    all.extend(eld_negative_fixtures());
    all
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    all_fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_document)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_document_fixture;

    #[test]
    fn every_document_fixture_behaves_as_declared() {
        for fixture in all_fixtures() {
            run_document_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn document_fixture_ids_are_unique() {
        let mut ids: Vec<&str> = all_fixtures().iter().map(|f| f.id).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate document fixture id");
    }
}
