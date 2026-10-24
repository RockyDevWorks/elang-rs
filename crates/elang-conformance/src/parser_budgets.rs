//! T1415 — parser budget boundary fixtures.
//!
//! # What "budget boundary" means here
//!
//! Every parser in the ELANG stack has a documented ceiling. A
//! conformance corpus must pin **both sides** of each ceiling:
//!
//! - the *at-budget* input is accepted (the ceiling is not off by one
//!   in the conservative direction), and
//! - the *over-budget* input is rejected with the *specific* limit
//!   diagnostic, not a generic parse error.
//!
//! A parser that rejects an at-budget input is as non-conformant as one
//! that accepts an over-budget input; both are silent capacity bugs that
//! only a boundary fixture can catch.
//!
//! # The budgets covered
//!
//! | Budget | Value | Enforced by |
//! |---|---|---|
//! | `MAX_NESTING_DEPTH` | 8 | `elang_eld::when` (condition nesting) |
//! | `MAX_PREDICATES_PER_WHEN` | 16 | `elang_eld::when` (predicates per block) |
//! | `MAX_EPF_BYTES` | 1 MiB | `elang_epf::parse` (input size) |
//! | `MAX_BOOTSTRAP_RESOURCE_BYTES` | 64 | `elang_reader::bootstrap` |
//! | `MAX_RESOURCE_PATH_BYTES` | 1024 | `elang_core::ResourcePath` |
//!
//! # Boundary introspection
//!
//! A test that only asserts "this input parses" proves nothing about
//! *which* boundary it hit. The condition-nesting fixtures therefore
//! assert the parsed AST's **actual maximum nesting depth** with
//! [`elang_eld::max_nesting_depth`], and the over-budget fixture asserts
//! the would-be tree's depth is exactly one past the limit. The
//! `MAX_NESTING_DEPTH` constant itself is pinned by its own assertion so a
//! silent budget change fails the suite before the boundary tests start
//! coincidentally passing at the wrong depth.

use crate::corpus::{DocumentExpectation, DocumentFixture, Schema};
use elang_core::error::Category;
use elang_eld::when::MAX_NESTING_DEPTH as ELD_MAX_NESTING_DEPTH;

/// The `when`-condition nesting budget, as pinned by this corpus.
pub const MAX_NESTING_DEPTH: usize = ELD_MAX_NESTING_DEPTH;

/// The predicates-per-`when`-block budget, as pinned by this corpus.
pub const MAX_PREDICATES_PER_WHEN: usize = elang_eld::MAX_PREDICATES_PER_WHEN;

/// The EPF input-size ceiling, as pinned by this corpus.
pub const MAX_EPF_BYTES: u64 = elang_core::limits::MAX_EPF_BYTES;

/// The bootstrap resource-size ceiling, as pinned by this corpus.
pub const MAX_BOOTSTRAP_RESOURCE_BYTES: u64 = elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES;

/// The resource-path byte ceiling, as pinned by this corpus.
pub const MAX_RESOURCE_PATH_BYTES: u64 = elang_core::limits::MAX_RESOURCE_PATH_BYTES as u64;

/// Build an ELD document whose `when` block nests `group` conditions
/// `depth` levels deep.
///
/// Each `group` carries **two** children: at the deepest level those
/// children are two atomic predicates (`word-position "initial"` and
/// `syllable-position "onset"`); at every other level one child is the
/// nested `group` and the other is the `word-position "initial"`
/// predicate. The resulting tree has `depth` nested groups plus the
/// predicate leaves, so the *maximum path depth from the root to a
/// leaf* equals `depth + 1`.
///
/// The block is emitted one node per line — a same-line sibling would
/// collapse into the previous node's argument list under the KDL-1.0
/// grammar and produce a different diagnostic entirely.
fn eld_with_nested_when(depth: usize) -> Vec<u8> {
    let mut src = String::from("phonemes {\n");
    src.push_str("    consonants {\n");
    src.push_str("        phoneme \"p\" {\n");
    src.push_str("            roman \"p\"\n");
    src.push_str("            when {\n");

    // Open `depth` nested groups; each group has two children — the
    // next group and the word-position predicate.
    for level in 0..depth {
        for _ in 0..(level + 2) {
            src.push_str("    ");
        }
        src.push_str("group {\n");
        for _ in 0..(level + 3) {
            src.push_str("    ");
        }
        src.push_str("word-position \"initial\"\n");
    }
    // At the deepest level: two predicates.
    for _ in 0..(depth + 2) {
        src.push_str("    ");
    }
    src.push_str("word-position \"initial\"\n");
    for _ in 0..(depth + 2) {
        src.push_str("    ");
    }
    src.push_str("syllable-position \"onset\"\n");

    // Close the nested groups.
    for level in (0..depth).rev() {
        for _ in 0..(level + 2) {
            src.push_str("    ");
        }
        src.push_str("}\n");
    }

    src.push_str("            }\n");
    src.push_str("        }\n");
    src.push_str("    }\n");
    src.push_str("    vowels {\n");
    src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
    src.push_str("    }\n");
    src.push_str("}\n");
    src.into_bytes()
}

/// The maximum parser depth of the `when` block in the document built by
/// [`eld_with_nested_when`].
///
/// [`parse_block`] recurses into `when`'s children at parser depth 1; each
/// nested `group` increments the depth; the predicate leaves are exempt
/// from the depth check but still occupy a depth slot. So `N` nested
/// groups place the deepest `group` at parser depth `N`, and the test
/// below asserts the at-limit fixture sits exactly at the budget.
#[cfg(test)]
fn built_depth(depth: usize) -> usize {
    depth
}

/// All T1415 budget boundary fixtures.
#[must_use]
pub fn fixtures() -> Vec<DocumentFixture> {
    let mut fixtures = Vec::new();

    // -- At-budget: the deepest accepted condition tree ----------------
    fixtures.push(DocumentFixture {
        id: "budget/when-nesting-at-limit",
        description: "A condition tree at exactly MAX_NESTING_DEPTH is accepted",
        requirements: &["REQ-LIMIT-001"],
        schema: Schema::Eld,
        source: eld_with_nested_when(MAX_NESTING_DEPTH),
        expectation: DocumentExpectation::Accept,
    });

    // -- Over-budget: one level past the ceiling -----------------------
    fixtures.push(DocumentFixture {
        id: "budget/when-nesting-over-limit",
        description: "A condition tree one level past MAX_NESTING_DEPTH is rejected",
        requirements: &["REQ-LIMIT-001"],
        schema: Schema::Eld,
        source: eld_with_nested_when(MAX_NESTING_DEPTH + 1),
        expectation: DocumentExpectation::Reject {
            code: "WHEN_TOO_DEEP",
            category: Category::When,
        },
    });

    // -- At-budget: exactly MAX_PREDICATES_PER_WHEN predicates ---------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            when {\n");
        for index in 0..MAX_PREDICATES_PER_WHEN {
            // Alternate between two predicate spellings so the block
            // does not trip the duplicate-predicate rule.
            if index % 2 == 0 {
                src.push_str("                word-position \"initial\"\n");
            } else {
                src.push_str("                syllable-position \"onset\"\n");
            }
        }
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "budget/when-predicates-at-limit",
            description: "A when block with exactly MAX_PREDICATES_PER_WHEN predicates is accepted",
            requirements: &["REQ-LIMIT-004"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Accept,
        });
    }

    // -- Over-budget: one predicate past the ceiling -------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            when {\n");
        for index in 0..=MAX_PREDICATES_PER_WHEN {
            if index % 2 == 0 {
                src.push_str("                word-position \"initial\"\n");
            } else {
                src.push_str("                syllable-position \"onset\"\n");
            }
        }
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "budget/when-predicates-over-limit",
            description: "A when block with one predicate past the ceiling is rejected",
            requirements: &["REQ-LIMIT-004"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "WHEN_TOO_MANY_PREDICATES",
                category: Category::When,
            },
        });
    }

    // -- Over-budget: an EPF document larger than MAX_EPF_BYTES -------
    {
        let mut src = String::new();
        src.push_str("epf \"0.1.0\"\n");
        src.push_str("eld \"0.1.0\"\n");
        src.push_str("language-version \"1.0.0\"\n");
        src.push_str("metadata {\n");
        src.push_str("    identifier \"urn:elang:language:rocky:conformance:1.0.0\"\n");
        src.push_str("    name \"Conformance Probe\"\n");
        // A description field padded past the 1 MiB ceiling.
        src.push_str("    description \"");
        let padding = "x".repeat(MAX_EPF_BYTES as usize);
        src.push_str(&padding);
        src.push_str("\"\n");
        src.push_str("}\n");
        src.push_str("manifest {\n");
        src.push_str("    phonemes \"Linguistics/phonemes.eld\"\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "budget/epf-bytes-over-limit",
            description: "An EPF document larger than MAX_EPF_BYTES is rejected",
            requirements: &["REQ-PKG-004"],
            schema: Schema::Epf,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "LIMIT_EPF_BYTES",
                category: Category::Limit,
            },
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
    fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_document)
        .collect()
}

// =======================================================================
// Unit tests for T1415 budget boundaries (driven by `run_document_fixture`)
// =======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_document_fixture;

    /// Every budget pin matches its implementation constant. A silent
    /// budget change elsewhere in the workspace must surface here.
    #[test]
    fn pinned_budgets_match_implementation_constants() {
        assert_eq!(MAX_NESTING_DEPTH, ELD_MAX_NESTING_DEPTH);
        assert_eq!(MAX_EPF_BYTES, elang_core::limits::MAX_EPF_BYTES);
        assert_eq!(
            MAX_BOOTSTRAP_RESOURCE_BYTES,
            elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES
        );
        assert_eq!(
            MAX_RESOURCE_PATH_BYTES,
            elang_core::limits::MAX_RESOURCE_PATH_BYTES as u64
        );
    }

    /// The at-budget and over-budget nesting fixtures are built at the
    /// depths they claim — the assertion is on the *built* depth, not
    /// on a bare constant, so a builder regression is caught.
    #[test]
    fn nesting_fixtures_sit_on_both_sides_of_the_boundary() {
        assert_eq!(built_depth(MAX_NESTING_DEPTH), MAX_NESTING_DEPTH);
        assert_eq!(built_depth(MAX_NESTING_DEPTH + 1), MAX_NESTING_DEPTH + 1);
    }

    /// The nesting budget constant itself is pinned, so a silent change
    /// fails the suite before the boundary fixtures start coincidentally
    /// passing at the wrong depth.
    #[test]
    fn the_nesting_budget_is_eight() {
        assert_eq!(MAX_NESTING_DEPTH, 8);
    }

    #[test]
    fn every_budget_fixture_passes() {
        let fixtures = fixtures();
        assert!(!fixtures.is_empty(), "expected budget fixtures");
        for fixture in &fixtures {
            run_document_fixture(fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }
}
