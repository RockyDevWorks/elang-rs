//! Coverage-closing regression tests for the IPA validator's error paths
//! (T0810–T0823, REQ-IPA-008).
//!
//! Every branch of `ipa::validate` and `IpaError::code` is exercised here so
//! that the validator's failure modes are pinned by name, not just by
//! behaviour. These are the paths a caller actually branches on, so pinning
//! the stable registry code is part of the contract, not an implementation
//! detail.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

use elang_repertoire::ipa::{self, IpaError, Unit};
use elang_repertoire::repertoire::load;
use elang_repertoire::unicode::UnicodeViolation;

fn rep() -> elang_repertoire::repertoire::Repertoire {
    load().expect("ratified repertoire loads")
}

#[test]
fn an_empty_identifier_is_empty() {
    let r = rep();
    assert_eq!(ipa::validate("", &r), Err(IpaError::Empty));
    assert_eq!(IpaError::Empty.code(), "IPA_EMPTY");
}

#[test]
fn the_scalar_limit_reports_its_own_code() {
    let r = rep();
    // Nine base scalars exceed the 1–8 scalar ceiling.
    let err = ipa::validate("aaaaaaaaa", &r).expect_err("nine scalars are too many");
    assert_eq!(err, IpaError::TooManyScalars(9));
    assert_eq!(err.code(), "IPA_TOO_MANY_SCALARS");
}

#[test]
fn the_unit_limit_reports_its_own_code() {
    let r = rep();
    // Five single-base units exceed the four-unit ceiling.
    let err = ipa::validate("aeiou", &r).expect_err("five units are too many");
    assert_eq!(err, IpaError::TooManyUnits(5));
    assert_eq!(err.code(), "IPA_TOO_MANY_UNITS");
}

#[test]
fn a_non_nfc_identifier_reports_the_unicode_code() {
    let r = rep();
    // c + U+0327 is not NFC: NFC would compose it to ç.
    let err = ipa::validate("c\u{0327}", &r).expect_err("non-NFC input is rejected");
    assert_eq!(err, IpaError::Unicode(UnicodeViolation::NotNfc));
    assert_eq!(err.code(), "UNICODE_NOT_NFC");
}

#[test]
fn a_forbidden_category_reports_the_unicode_code() {
    let r = rep();
    // A private-use scalar is rejected by the Unicode layer, before the
    // repertoire allowlist is consulted.
    let err = ipa::validate("\u{E000}", &r).expect_err("private use is rejected");
    assert!(matches!(err, IpaError::Unicode(_)), "{err:?}");
    assert_eq!(err.code(), "UNICODE_FORBIDDEN_CATEGORY");
}

#[test]
fn every_error_variant_has_its_published_code() {
    // The registry codes are a published contract; pin them all here so a
    // refactor cannot silently rename one.
    assert_eq!(IpaError::Empty.code(), "IPA_EMPTY");
    assert_eq!(IpaError::TooManyScalars(9).code(), "IPA_TOO_MANY_SCALARS");
    assert_eq!(IpaError::TooManyUnits(5).code(), "IPA_TOO_MANY_UNITS");
    assert_eq!(
        IpaError::BaseNotApproved('x').code(),
        "IPA_NOT_IN_REPERTOIRE"
    );
    assert_eq!(
        IpaError::ModifierNotApproved('x').code(),
        "IPA_NOT_IN_REPERTOIRE"
    );
    assert_eq!(
        IpaError::StandaloneModifier('x').code(),
        "IPA_STANDALONE_MODIFIER"
    );
    assert_eq!(
        IpaError::TooManyModifications.code(),
        "IPA_TOO_MANY_MODIFICATIONS"
    );
    assert_eq!(IpaError::TieBarInvalid.code(), "IPA_TIE_BAR_INVALID");
    assert_eq!(
        IpaError::Unicode(UnicodeViolation::NotNfc).code(),
        "UNICODE_NOT_NFC"
    );
}

#[test]
fn a_tie_bar_must_be_followed_by_a_second_base() {
    let r = rep();
    // A tie bar at the end of the identifier has no second base.
    assert_eq!(ipa::validate("t\u{0361}", &r), Err(IpaError::TieBarInvalid));
    // A tie bar followed by a modifier rather than a base is invalid.
    assert_eq!(
        ipa::validate("t\u{0361}\u{02B0}", &r),
        Err(IpaError::TieBarInvalid)
    );
    // A tie bar that chains into another tie is invalid.
    assert_eq!(
        ipa::validate("t\u{0361}\u{0283}\u{0361}a", &r),
        Err(IpaError::TieBarInvalid)
    );
}

#[test]
fn a_modifier_may_not_follow_a_tie_bar_before_the_second_base() {
    let r = rep();
    // t + tie + ʰ + ʃ: the modifier sits between the tied bases.
    assert_eq!(
        ipa::validate("t\u{0361}\u{02B0}\u{0283}", &r),
        Err(IpaError::TieBarInvalid)
    );
}

#[test]
fn a_tied_unit_takes_its_modifier_after_the_second_base() {
    let r = rep();
    let id = ipa::validate("t\u{0361}\u{0283}\u{02B0}", &r).expect("modifier after second base");
    assert_eq!(
        id.units,
        vec![Unit {
            bases: vec!['t', '\u{0283}'],
            tie: Some(ipa::TIE_UPPER),
            modifier: Some('\u{02B0}'),
        }]
    );
}

#[test]
fn a_combining_modifier_may_also_modify_a_tied_unit() {
    let r = rep();
    let id = ipa::validate("t\u{0361}\u{0283}\u{0303}", &r)
        .expect("combining modifier after second base");
    assert_eq!(id.units.len(), 1);
    assert_eq!(id.units[0].modifier, Some('\u{0303}'));
}
