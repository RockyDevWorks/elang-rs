//! T1416 — the explicit inspect / verify / validate level matrix.
//!
//! # The three levels
//!
//! | Level | Stages | Guarantee |
//! |---|---|---|
//! | `Inspect` | 1–7 | structure only; no decompression, no integrity |
//! | `Verify` | 1–11 | every resource decompresses and hashes match |
//! | `Validate` | 1–12 | full semantic validity of EPF + ELD |
//!
//! # What this module asserts
//!
//! Every golden package must survive **all three** levels. A package
//! that only passes `inspect` is structurally sound but semantically
//! incomplete; a corpus that published such a package as a "valid"
//! fixture would be laundering a broken package into the conformance
//! set.
//!
//! The matrix therefore publishes one fixture per (package, level)
//! pair — six fixtures in total — and the runner executes each one
//! through the production reader.

use crate::corpus::{Expectation, Fixture, Level};
use crate::golden_packages;

/// The catalog id of one (package, level) cell of the matrix.
///
/// The matrix publishes six fixtures — two packages times three reader
/// levels — and the catalog requires every fixture id to be unique, so
/// the id carries both coordinates of the cell. The mapping is a
/// `match` rather than a `format!` because [`Fixture::id`] is a
/// `&'static str`: a runtime-built id would not be `'static`.
fn matrix_id(label: &str, level: Level) -> &'static str {
    match (label, level) {
        ("minimal", Level::Inspect) => "matrix/minimal/inspect",
        ("minimal", Level::Verify) => "matrix/minimal/verify",
        ("minimal", Level::Validate) => "matrix/minimal/validate",
        ("full", Level::Inspect) => "matrix/full/inspect",
        ("full", Level::Verify) => "matrix/full/verify",
        ("full", Level::Validate) => "matrix/full/validate",
        _ => panic!("unknown matrix cell: package {label:?}, level {level:?}"),
    }
}

/// Every valid golden package must pass `inspect`, `verify`, and
/// `validate` with no error. This is the central anti-laundering claim
/// of the corpus: a "valid" package that is only structurally intact
/// would fail at `validate` and be re-categorised.
///
/// This module exposes one [`Fixture`] per (golden package, level) so
/// the level-ladder is exercised against real archives, not against
/// hand-built bytes.
#[must_use]
pub fn fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();
    for (label, build) in [
        (
            "minimal",
            minimal_package as fn() -> Result<golden_packages::GoldenPackage, String>,
        ),
        (
            "full",
            full_package as fn() -> Result<golden_packages::GoldenPackage, String>,
        ),
    ] {
        let archive = match build() {
            Ok(p) => p.archive,
            Err(err) => panic!("golden package {label} must build: {err}"),
        };
        for level in [Level::Inspect, Level::Verify, Level::Validate] {
            fixtures.push(Fixture {
                id: matrix_id(label, level),
                description: "A golden package passes the declared reader level",
                requirements: &["REQ-CONF-003"],
                expectation: Expectation::Accept(level),
                archive: archive.clone(),
            });
        }
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
        .map(crate::catalog::CatalogEntry::from_fixture)
        .collect()
}

fn minimal_package() -> Result<golden_packages::GoldenPackage, String> {
    golden_packages::minimal().map_err(|e| format!("{e:?}"))
}

fn full_package() -> Result<golden_packages::GoldenPackage, String> {
    golden_packages::full().map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_fixture;

    /// The matrix declares every level for both packages; the runner
    /// executes each fixture and checks that no failure surfaces. A
    /// regression that drops one of these fixtures would otherwise
    /// hide a level-package mismatch.
    #[test]
    fn every_level_of_every_golden_package_passes() {
        let fixtures = fixtures();
        assert_eq!(fixtures.len(), 6);
        for fixture in &fixtures {
            run_fixture(fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }
}
