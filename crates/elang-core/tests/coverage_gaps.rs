//! Coverage-closure tests for `elang-core` boundary paths.
//!
//! Each test here drives a *reachable* branch that the crate's own unit
//! tests never take: an unchecked-conversion helper that no production
//! caller uses, the "no expected size" mode of the byte counter, the
//! runtime (non-const) instantiation of the version newtypes, and the
//! slug-shape rejections of the language-identifier URN.
//!
//! Test code path: `.expect()` / `.expect_err()` / panics signal fixture
//! regressions, not untrusted-input failure. The workspace lint policy
//! denies these on production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use elang_core::bounded::{BoundedCounter, ByteBudget};
use elang_core::checked::usize_to_u64;
use elang_core::model::epf::LanguageIdentifier;
use elang_core::version::{
    ContainerVersion, EldVersion, EpfVersion, LanguageVersion, SemVer, Version,
};

// ---------------------------------------------------------------------------
// checked::usize_to_u64
// ---------------------------------------------------------------------------

/// `usize_to_u64` is the uniform-shape `Option` wrapper around the
/// infallible `usize` → `u64` widening. It is public API, so it must be
/// exercised at runtime rather than only through its callers.
#[test]
fn usize_to_u64_widens_without_truncation() {
    assert_eq!(usize_to_u64(0), Some(0));
    assert_eq!(usize_to_u64(1), Some(1));
    assert_eq!(usize_to_u64(usize::MAX), u64::try_from(usize::MAX).ok());
}

// ---------------------------------------------------------------------------
// bounded::ByteBudget::cap / BoundedCounter without an expected size
// ---------------------------------------------------------------------------

/// The budget publishes the cap it was built with, alongside `used` and
/// `remaining`.
#[test]
fn byte_budget_publishes_its_cap() {
    let budget = ByteBudget::new(4096);
    assert_eq!(budget.cap(), 4096);
    assert_eq!(budget.used(), 0);
    assert_eq!(budget.remaining(), 4096);
}

/// A counter built *without* an expected size accepts any total that stays
/// under its cap: the "declared size" comparison is skipped entirely, and
/// `finish` reports the produced byte count.
#[test]
fn bounded_counter_without_expected_size_skips_the_declared_size_check() {
    let mut counter = BoundedCounter::new(1024, None);
    assert!(counter.record(700).is_ok());
    assert!(counter.record(324).is_ok());
    assert_eq!(
        counter.finish().expect("finish reports the produced count"),
        1024
    );
}

/// The same "no expected size" mode still fails closed when the cap itself
/// is exceeded.
#[test]
fn bounded_counter_without_expected_size_still_enforces_its_cap() {
    let mut counter = BoundedCounter::new(16, None);
    assert!(counter.record(16).is_ok());
    assert!(counter.record(1).is_err());
}

// ---------------------------------------------------------------------------
// version::from_semver
// ---------------------------------------------------------------------------

/// The version newtypes expose a `const fn from_semver`. The domain
/// constants evaluate it at compile time, which leaves the function body
/// uncovered by the runtime profile; calling it at runtime does cover it.
#[test]
fn version_newtypes_wrap_a_semver_at_runtime() {
    let semver = SemVer::new(0, 1, 0);

    let container = ContainerVersion::from_semver(semver);
    assert_eq!(container.semver(), semver);
    assert_eq!(Version::semver(container), semver);

    let epf = EpfVersion::from_semver(semver);
    assert_eq!(epf.semver(), semver);

    let eld = EldVersion::from_semver(semver);
    assert_eq!(eld.semver(), semver);

    let language = LanguageVersion::from_semver(semver);
    assert_eq!(language.semver(), semver);
}

// ---------------------------------------------------------------------------
// version::SemVer::parse — empty component
// ---------------------------------------------------------------------------

/// `MAJOR..PATCH` splits into three components, so the shape check passes;
/// the empty component itself must be rejected by the component parser.
#[test]
fn semver_rejects_an_empty_component() {
    let err = SemVer::parse("1..0").expect_err("an empty component is not a version");
    assert_eq!(err.code(), "EPF_BAD_VERSION");
    assert!(
        err.diagnostic.message.contains("empty version component"),
        "unexpected message: {}",
        err.diagnostic.message
    );
}

// ---------------------------------------------------------------------------
// model::epf::LanguageIdentifier — slug shape
// ---------------------------------------------------------------------------

/// The namespace slug is validated first; an empty namespace slug is
/// rejected before the language-name slug is even looked at.
#[test]
fn language_identifier_rejects_an_empty_namespace_slug() {
    let err = LanguageIdentifier::parse("urn:elang:language::aelith:1.0.0")
        .expect_err("an empty namespace slug is rejected");
    assert_eq!(err.code(), "EPF_IDENTIFIER_BAD_SLUG");
    assert!(
        err.diagnostic.message.contains("namespace"),
        "unexpected message: {}",
        err.diagnostic.message
    );
}

/// A slug whose *interior* byte is neither `[a-z0-9]` nor `-` is rejected by
/// the per-byte scan, even though its first and last bytes are fine.
#[test]
fn language_identifier_rejects_a_slug_with_an_interior_invalid_byte() {
    let err = LanguageIdentifier::parse("urn:elang:language:a_b:aelith:1.0.0")
        .expect_err("an interior `_` is not a slug byte");
    assert_eq!(err.code(), "EPF_IDENTIFIER_BAD_SLUG");
}

/// The language-name slug is validated independently of the namespace slug;
/// a bad language name is reported with its own diagnostic.
#[test]
fn language_identifier_rejects_a_bad_language_name_slug() {
    let err = LanguageIdentifier::parse("urn:elang:language:rocky:Bad-Name:1.0.0")
        .expect_err("an uppercase language-name slug is rejected");
    assert_eq!(err.code(), "EPF_IDENTIFIER_BAD_SLUG");
    assert!(
        err.diagnostic.message.contains("language-name"),
        "unexpected message: {}",
        err.diagnostic.message
    );
}

/// The happy path still parses, and the parts are retained.
#[test]
fn language_identifier_accepts_a_well_formed_urn() {
    let id = LanguageIdentifier::parse("urn:elang:language:rocky:test:1.2.3")
        .expect("a well-formed URN parses");
    assert_eq!(id.namespace(), "rocky");
    assert_eq!(id.language_name(), "test");
    assert_eq!(id.version(), SemVer::new(1, 2, 3));
}
