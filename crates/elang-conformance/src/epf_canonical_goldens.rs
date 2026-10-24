//! T1405 — EPF canonical KDL golden files.
//!
//! This module publishes canonical EPF text fixtures alongside their
//! SHA-256 digests. The EPF canonicalisation profile (steering §10) locks
//! the byte-level shape of `package.epf`; the goldens here pin that
//! profile so an independent implementation can be checked byte-for-byte
//! without reading this repository's Rust code.
//!
//! Every golden is produced by `elang_epf::canonical::to_string` — never
//! hand-written. The generator binary `gen_epf_canonical_goldens` prints
//! the canonical bytes and their SHA-256 digests so they can be pinned
//! in [`CASES`]; the pinned digests live in this module's source, not in
//! a separate registry file.
//!
//! # Acceptance contract
//!
//! A reader's EPF canonicaliser must reproduce the pinned bytes
//! byte-for-byte. The conformance runner compares the production output
//! against the pinned values in [`CASES`]; a divergence is a
//! corpus failure that names the case id, the requirement id, and the
//! concrete byte mismatch.

use crate::binary_vectors::sha256;
use crate::corpus::VectorFixture;

/// One published EPF canonical-golden case.
#[derive(Debug, Clone, Copy)]
pub struct EpfGoldenCase {
    /// Stable fixture id (the directory name).
    pub id: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Requirements this case exercises.
    pub requirements: &'static [&'static str],
    /// The authored EPF source (KDL, not canonical).
    pub source: &'static str,
    /// The pinned canonical bytes (UTF-8, LF line endings, one final LF).
    pub canonical: &'static str,
    /// Lowercase hex SHA-256 of the canonical bytes.
    pub sha256_hex: &'static str,
}

/// The pinned EPF canonical corpus.
///
/// The canonical bytes below are produced by `elang_epf::canonical::to_string`
/// and frozen. If a future change alters the canonicalisation, regenerate
/// the goldens via `cargo run -p elang-conformance --bin
/// gen_epf_canonical_goldens` and update this table atomically with the
/// library change.
pub const CASES: &[EpfGoldenCase] = &[
    EpfGoldenCase {
        id: "minimal",
        description: "The minimal EPF document (one identifier, no manifest extras)",
        requirements: &["REQ-EPF-014", "REQ-CANON-009"],
        source: MINIMAL_EPF,
        canonical: MINIMAL_EPF_CANONICAL,
        sha256_hex: MINIMAL_EPF_SHA256,
    },
    EpfGoldenCase {
        id: "full",
        description: "The full EPF document (identifier + name + description + manifest)",
        requirements: &["REQ-EPF-014", "REQ-CANON-009"],
        source: FULL_EPF_SRC,
        canonical: FULL_EPF_CANONICAL,
        sha256_hex: FULL_EPF_SHA256,
    },
];

/// The minimal authored EPF.
pub const MINIMAL_EPF: &str = concat!(
    "epf \"0.1.0\"\n",
    "eld \"0.1.0\"\n",
    "language-version \"1.0.0\"\n",
    "metadata {\n",
    "    identifier \"urn:elang:language:rocky:test:1.0.0\"\n",
    "    name \"Conformance Minimal\"\n",
    "}\n",
    "manifest {\n",
    "    phonemes \"Linguistics/phonemes.eld\"\n",
    "}\n",
);

/// The minimal canonical EPF (one trailing LF, sorted keys, no extra spaces).
pub const MINIMAL_EPF_CANONICAL: &str = concat!(
    "epf \"0.1.0\"\n",
    "eld \"0.1.0\"\n",
    "language-version \"1.0.0\"\n",
    "metadata {\n",
    "    identifier \"urn:elang:language:rocky:test:1.0.0\"\n",
    "    name \"Conformance Minimal\"\n",
    "}\n",
    "manifest {\n",
    "    phonemes \"Linguistics/phonemes.eld\"\n",
    "}\n",
);

/// The SHA-256 of the minimal canonical EPF, hex-lowercase.
pub const MINIMAL_EPF_SHA256: &str =
    "c78474c02d0c62adb4319f715ec7f4bf43cc1213b82f897fb7ae15a2093b559b";

/// The full authored EPF (un-canonical: extra metadata fields, varied
/// whitespace).
pub const FULL_EPF_SRC: &str = concat!(
    "epf  \"0.1.0\"\n",
    "eld  \"0.1.0\"\n",
    "language-version  \"1.0.0\"\n",
    "metadata {\n",
    "    name \"Conformance Full\"\n",
    "    description \"Full-featured reference package.\"\n",
    "    identifier \"urn:elang:language:rocky:conformance-full:1.0.0\"\n",
    "}\n",
    "manifest {\n",
    "    phonemes \"Linguistics/phonemes.eld\"\n",
    "}\n",
);

/// The full canonical EPF (sorted, normalised whitespace, single trailing LF).
pub const FULL_EPF_CANONICAL: &str = concat!(
    "epf \"0.1.0\"\n",
    "eld \"0.1.0\"\n",
    "language-version \"1.0.0\"\n",
    "metadata {\n",
    "    identifier \"urn:elang:language:rocky:conformance-full:1.0.0\"\n",
    "    name \"Conformance Full\"\n",
    "    description \"Full-featured reference package.\"\n",
    "}\n",
    "manifest {\n",
    "    phonemes \"Linguistics/phonemes.eld\"\n",
    "}\n",
);

/// The SHA-256 of the full canonical EPF.
pub const FULL_EPF_SHA256: &str =
    "44d9de8a0975f1e3f4e3f785f6de0921a62ced83b2180a8d9ae8de51179c67a0";

/// Build a vector fixture that checks the production canonicaliser
/// produces the pinned bytes for `case`.
///
/// The pipeline is `parse` → `canon::to_string`: the canonicaliser
/// operates on the validated model, so the fixture also proves the
/// parser and the canonicaliser agree on the same document.
fn canonical_vector_fixture(case: &EpfGoldenCase) -> VectorFixture {
    let actual = match elang_epf::parse(case.source.as_bytes()) {
        Ok(pkg) => elang_epf::canon::to_string(&pkg).into_bytes(),
        Err(err) => {
            // The fixture still runs; the divergence from the pinned
            // bytes is reported by the runner with the concrete bytes.
            let mut bytes = format!("parse failed: {err:?}").into_bytes();
            bytes.push(0);
            bytes
        }
    };
    VectorFixture {
        id: format!("epf-canonical/{}", case.id),
        description: case.description,
        requirements: case.requirements,
        actual,
        expected: case.canonical.as_bytes().to_vec(),
    }
}

/// The vector fixtures pinning every published case's canonical bytes and
/// SHA-256.
#[must_use]
pub fn golden_fixtures() -> Vec<VectorFixture> {
    let mut fixtures = Vec::new();
    for case in CASES {
        fixtures.push(canonical_vector_fixture(case));
        fixtures.push(VectorFixture {
            id: format!("epf-canonical-sha256/{}", case.id),
            description: "The SHA-256 of the canonical bytes matches the pinned digest",
            requirements: case.requirements,
            actual: sha256(case.canonical.as_bytes()).to_vec(),
            expected: hex_decode(case.sha256_hex),
        });
    }
    fixtures
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    golden_fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_vector)
        .collect()
}

/// Decode a hex string into bytes. Panics on odd length or non-hex
/// characters — these are authoring bugs in the pinned constants.
fn hex_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    assert!(
        bytes.len().is_multiple_of(2),
        "pinned hex string has odd length: {value:?}"
    );
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let high = (bytes[index] as char).to_digit(16).unwrap_or_else(|| {
            panic!("non-hex character in pinned digest {value:?}");
        }) as u8;
        let low = (bytes[index + 1] as char).to_digit(16).unwrap_or_else(|| {
            panic!("non-hex character in pinned digest {value:?}");
        }) as u8;
        out.push((high << 4) | low);
        index += 2;
    }
    out
}

/// Run every T1405 vector fixture against the production canonicaliser.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary_vectors::hex;
    use crate::corpus::run_vector_fixture;

    #[test]
    fn every_pinned_case_canonicalises_to_its_pinned_bytes() {
        for fixture in golden_fixtures() {
            run_vector_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn pinned_digest_strings_are_well_formed() {
        for case in CASES {
            let bytes = hex_decode(case.sha256_hex);
            assert_eq!(bytes.len(), 32, "{}: not a SHA-256", case.id);
            assert_eq!(hex(&bytes), case.sha256_hex);
        }
    }

    #[test]
    fn golden_cases_are_unique_by_id() {
        let mut ids: Vec<&str> = CASES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate EPF golden case id");
    }
}
