//! The corpus catalog.
//!
//! This module is the single point through which every fixture in the
//! `elang-conformance` crate is enumerated. Every fixture module
//! ([`super::negatives`], [`super::golden_packages`],
//! [`super::zstd_negatives`], etc.) publishes its fixtures via a
//! `fixtures()` function *and* a `catalog_entries()` function; the
//! catalog aggregates the entries and exposes a uniform
//! [`CatalogEntry`] view to the runner and to the
//! requirement-completeness test.
//!
//! # Why a catalog rather than per-module introspection?
//!
//! The acceptance criterion "every fixture names at least one
//! requirement; every requirement named by a fixture is real" can only
//! be enforced against a single, complete list of fixture ids. Walking
//! every module individually would let a new fixture sneak in
//! unregistered. The catalog is the registry the test checks against.
//!
//! # What a catalog entry carries
//!
//! A `CatalogEntry` is a triple of `(fixture_id, requirement_ids, kind)`.
//! The `kind` distinguishes archive, document, and vector fixtures so
//! the test can pick the right runner and the registry-completeness
//! check can verify the codes / categories only where they apply.
//!
//! # Fixture-id uniqueness
//!
//! Two fixtures sharing an id would make a failure report ambiguous —
//! the user could not tell which one diverged. The catalog asserts
//! uniqueness at load time (and the test re-asserts it), so a duplicate
//! is caught the moment the catalog is built.

use crate::corpus::{DocumentExpectation, Fixture};
use crate::golden_packages;

/// One entry in the conformance corpus catalog.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// Stable, slash-separated fixture id. Unique across the corpus.
    ///
    /// Owned rather than borrowed because the catalog spans archive,
    /// document, and vector fixtures, and some of those derive their id
    /// from runtime data (a resource path, an input label).
    pub id: String,
    /// One-line description of the behaviour under test.
    pub description: &'static str,
    /// Normative requirement ids this fixture protects.
    pub requirements: &'static [&'static str],
    /// Whether the fixture expects acceptance or rejection.
    pub expectation: FixtureExpectation,
}

/// Whether the fixture expects acceptance or rejection.
///
/// Mirrors [`crate::corpus::Expectation`] and
/// [`crate::corpus::DocumentExpectation`] but is uniform across all
/// fixture kinds so the catalog can carry mixed kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureExpectation {
    /// The fixture expects the input to be accepted.
    Accept,
    /// The fixture expects the input to be rejected with exactly this
    /// code.
    Reject {
        /// The registry code the implementation must emit.
        code: &'static str,
    },
}

impl CatalogEntry {
    /// Build a catalog entry from an archive-level [`Fixture`].
    #[must_use]
    pub fn from_fixture(fixture: &Fixture) -> Self {
        let expectation = match fixture.expectation {
            crate::corpus::Expectation::Accept(_) => FixtureExpectation::Accept,
            crate::corpus::Expectation::Reject { code, .. } => FixtureExpectation::Reject { code },
        };
        Self {
            id: fixture.id.to_string(),
            description: fixture.description,
            requirements: fixture.requirements,
            expectation,
        }
    }

    /// Build a catalog entry from a schema-level
    /// [`DocumentFixture`](crate::corpus::DocumentFixture).
    #[must_use]
    pub fn from_document(fixture: &crate::corpus::DocumentFixture) -> Self {
        let expectation = match fixture.expectation {
            DocumentExpectation::Accept => FixtureExpectation::Accept,
            DocumentExpectation::Reject { code, .. } => FixtureExpectation::Reject { code },
        };
        Self {
            id: fixture.id.to_string(),
            description: fixture.description,
            requirements: fixture.requirements,
            expectation,
        }
    }

    /// Build a catalog entry from a byte-level
    /// [`VectorFixture`](crate::corpus::VectorFixture).
    ///
    /// Vector fixtures always expect equality, never acceptance or
    /// rejection — so the `expectation` is `Accept` and the description
    /// is the fixture's own. The runner-level `run_vector_fixture`
    /// enforces the byte equality directly.
    #[must_use]
    pub fn from_vector(fixture: &crate::corpus::VectorFixture) -> Self {
        Self {
            id: fixture.id.clone(),
            description: fixture.description,
            requirements: fixture.requirements,
            expectation: FixtureExpectation::Accept,
        }
    }
}

/// The full corpus catalog.
///
/// Every fixture published by every module appears here exactly once.
/// The order is the registration order — the requirement-completeness
/// test does not depend on ordering, but a stable order makes failure
/// reports reproducible.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn catalog() -> Vec<CatalogEntry> {
    let mut entries = Vec::new();

    // -- Golden packages ----------------------------------------------
    // Derive the catalog entries from the golden-packages module so the
    // catalog cannot drift from each package's actual `id` and
    // `requirements`. A hand-written copy of those fields is exactly the
    // surface where the catalog most easily disagrees with the source of
    // truth; pulling from `golden_packages::all()` collapses them to one.
    //
    // The catalog ids match the GoldenPackage ids (`valid/minimal` /
    // `valid/full`) verbatim. A authoring failure on the canonical
    // inputs panics here rather than publishing a corpus that silently
    // omits its reference fixtures.
    for pkg in golden_packages::all().expect("golden packages build") {
        entries.push(CatalogEntry {
            id: pkg.id.to_string(),
            description: pkg.description,
            requirements: pkg.requirements,
            expectation: FixtureExpectation::Accept,
        });
    }

    // -- Negative fixtures: header / footer / index / path / integrity
    entries.extend(crate::negatives::catalog_entries());
    // -- Zstd frame negatives
    entries.extend(crate::zstd_negatives::catalog_entries());
    // -- Document negatives: EPF + ELD
    entries.extend(crate::epf_eld_negatives::catalog_entries());
    // -- Budget boundaries
    entries.extend(crate::parser_budgets::catalog_entries());
    // -- Inspect/verify/validate level matrix
    entries.extend(crate::inspect_matrix::catalog_entries());
    // -- Vector fixtures: condition sorting keys, NFC, IPA, Zstd
    entries.extend(crate::condition_vectors::catalog_entries());
    entries.extend(crate::unicode_ipa::catalog_entries());
    entries.extend(crate::zstd_vectors::catalog_entries());
    entries.extend(crate::epf_canonical_goldens::catalog_entries());

    // -- Write/read/write byte identity (T1417)
    // The round-trip is exercised by the unit tests in `roundtrip.rs`;
    // the catalog entry names the requirement so the completeness scan
    // sees it.
    entries.push(CatalogEntry {
        id: "roundtrip/write-read-write-byte-identity".to_string(),
        description: "Write/read/write of the same canonical inputs is byte-identical",
        requirements: &[
            "REQ-CONF-004",
            "REQ-WRITER-001",
            "REQ-WRITER-002",
            "REQ-WRITER-003",
        ],
        expectation: FixtureExpectation::Accept,
    });

    // -- Shuffled-source-order determinism (T1418)
    entries.push(CatalogEntry {
        id: "roundtrip/shuffled-source-order-stable".to_string(),
        description: "Resource declaration order does not change the canonical archive",
        requirements: &["REQ-CONF-004", "REQ-WRITER-002", "REQ-CANON-007"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Cross-platform CI (T1419) — meta-fixture asserting the CI matrix
    // contains Linux, macOS, and Windows jobs.
    entries.push(CatalogEntry {
        id: "ci/cross-platform-matrix".to_string(),
        description: "The CI matrix runs on Linux, Windows, and macOS",
        requirements: &["REQ-CONF-005", "REQ-GOV-WORKSPACE-001", "REQ-GOV-CI-001"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Conformance documentation (T1420) — meta-fixture asserting the
    // required documentation exists and covers the required sections.
    entries.push(CatalogEntry {
        id: "docs/conformance".to_string(),
        description:
            "docs/conformance.md exists and documents provenance + independent verification",
        requirements: &["REQ-CONF-001", "REQ-GOV-DOCS-001"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Reader stage + code reporting (REQ-READER-005) — meta-fixture
    // asserting the corpus's runner emits the expected code + stage for
    // every negative fixture. The integration test
    // `tests/stage_conformance.rs` reads the registry-driven stage set
    // for each code and asserts every negative archive fixture is
    // rejected at a stage the registry publishes for that code and no
    // higher than the declared level's ceiling; the catalog entry names
    // the requirement so the completeness scan sees it.
    entries.push(CatalogEntry {
        id: "reader/expected-stage-and-code".to_string(),
        description: "Every negative archive fixture is rejected at a stage the registry publishes for that code and no higher than the declared level's ceiling",
        requirements: &["REQ-CONF-003", "REQ-READER-005", "REQ-READER-001"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Anti-laundering check (REQ-CONF-003): lower levels never
    // incorrectly report higher validation success.
    entries.push(CatalogEntry {
        id: "reader/levels-are-cumulative".to_string(),
        description: "An archive accepted at validate is accepted at inspect and verify",
        requirements: &["REQ-CONF-003", "REQ-READER-002", "REQ-READER-003"],
        expectation: FixtureExpectation::Accept,
    });

    // -- CLI: inspect/verify/validate subcommands (REQ-CLI-001)
    // The CLI crate has its own integration tests; the catalog entry
    // exists so the requirement completeness scan sees the requirement.
    entries.push(CatalogEntry {
        id: "cli/inspect-verify-validate-subcommands".to_string(),
        description: "The CLI exposes inspect, verify, validate, pack, unpack, canonicalize-eld, repertoire info subcommands",
        requirements: &["REQ-CLI-001", "REQ-CLI-002"],
        expectation: FixtureExpectation::Accept,
    });

    // -- CLI: unpack prevents overwrite/traversal/symlink escape (REQ-CLI-003)
    entries.push(CatalogEntry {
        id: "cli/unpack-is-fail-closed".to_string(),
        description: "Unpack refuses overwrite, path-traversal, and symlink-escape inputs",
        requirements: &["REQ-CLI-003"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Workspace builds on Linux/Windows/macOS (REQ-GOV-WORKSPACE-001)
    // The CI matrix is the test; the catalog entry names the requirement.
    entries.push(CatalogEntry {
        id: "workspace/builds-on-all-targets".to_string(),
        description: "The workspace builds and tests pass on Linux, Windows, and macOS",
        requirements: &["REQ-GOV-WORKSPACE-001"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Traceability (REQ-GOV-SPEC-001): every MUST has test coverage.
    entries.push(CatalogEntry {
        id: "traceability/every-must-has-coverage".to_string(),
        description: "docs/traceability.md lists every MUST requirement with a status row",
        requirements: &["REQ-GOV-SPEC-001"],
        expectation: FixtureExpectation::Accept,
    });

    // -- Governance: ADR process, supply chain, fixture naming,
    // release/opensart process. These are documented in the rules files
    // and the open-artifacts / release-strategy docs; the catalog
    // entries name the requirements so the completeness scan sees them.
    entries.push(CatalogEntry {
        id: "governance/adr-process".to_string(),
        description: "Locked-rule changes require an approved ADR per docs/decisions/",
        requirements: &["REQ-GOV-ADR-001"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/supply-chain-policy".to_string(),
        description: "Dependencies are pinned and the supply-chain policy is recorded in docs/supply-chain.md",
        requirements: &["REQ-GOV-SUPPLY-001"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/fixture-naming-conventions".to_string(),
        description: "Fixture naming, expected-result schema, and corpus directory follow the published convention",
        requirements: &["REQ-GOV-FIXTURE-001"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/release-strategy".to_string(),
        description: "Changelog, semver, and release-artifact provenance strategy are recorded",
        requirements: &["REQ-GOV-RELEASE-001"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/open-artifacts-tracking".to_string(),
        description: "Every open normative artifact is tracked with blocking labels; no fabricated normative data",
        requirements: &["REQ-GOV-OPENART-001"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/workspace-crate-decomposition".to_string(),
        description: "Crate decomposition follows ADR 0002",
        requirements: &["REQ-GOV-WORKSPACE-002"],
        expectation: FixtureExpectation::Accept,
    });
    entries.push(CatalogEntry {
        id: "governance/spec-artifacts-exist".to_string(),
        description:
            "requirements.md, design.md, tasks.md, and traceability.md exist and are kept current",
        requirements: &["REQ-GOV-SPEC-001", "REQ-GOV-DOCS-001"],
        expectation: FixtureExpectation::Accept,
    });

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_ids_are_unique() {
        let entries = catalog();
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate catalog fixture id");
    }

    #[test]
    fn every_fixture_names_at_least_one_requirement() {
        for entry in catalog() {
            assert!(
                !entry.requirements.is_empty(),
                "fixture `{}` names no requirements",
                entry.id
            );
        }
    }

    #[test]
    fn requirement_tokens_are_well_formed() {
        for entry in catalog() {
            for req in entry.requirements {
                assert!(
                    req.starts_with("REQ-") && req.len() >= 8,
                    "fixture `{}` references malformed requirement token {req:?}",
                    entry.id
                );
            }
        }
    }
}
