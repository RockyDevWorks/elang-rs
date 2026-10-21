//! Unicode validation adapter over the pinned [`elang_unicode`] crate
//! (Unicode 18.0.0, UCD-generated tables; NFC verified against the official
//! 18.0.0 NormalizationTest.txt — ADR 0007).
//!
//! This module preserves the small `UnicodeValidator`/`UnicodeViolation` API
//! the repertoire and IPA validators were written against, but all Unicode
//! decisions now come from `elang-unicode`. There is no ICU4X dependency and
//! no host-OS Unicode behavior.

pub use elang_unicode::{is_noncharacter, PINNED_UNICODE_VERSION};

/// The Unicode version the bundled General_Category + NFC data provides.
pub const DATA_UNICODE_VERSION: &str = elang_unicode::DATA_UNICODE_VERSION;

/// Whether the bundled Unicode data matches the steering-pinned version.
/// Now `true`: the data is generated from UCD 18.0.0 and NFC passes the 18.0.0
/// conformance suite (ADR 0007).
#[must_use]
pub fn unicode_data_matches_pin() -> bool {
    elang_unicode::gc_data_matches_pin()
}

/// Why a string/scalar is not a valid ELANG Unicode value. Mirrors the earlier
/// variant set (so existing match arms keep working); maps onto
/// [`elang_unicode::UnicodeViolation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeViolation {
    /// Not in Normalization Form C. Code: `UNICODE_NOT_NFC`.
    NotNfc,
    /// A control character (`Cc`). Code: `UNICODE_FORBIDDEN_CATEGORY`.
    ControlCharacter {
        /// Offending scalar.
        scalar: char,
    },
    /// A format character (`Cf`). Code: `UNICODE_FORBIDDEN_CATEGORY`.
    FormatCharacter {
        /// Offending scalar.
        scalar: char,
    },
    /// A private-use character (`Co`). Code: `UNICODE_FORBIDDEN_CATEGORY`.
    PrivateUse {
        /// Offending scalar.
        scalar: char,
    },
    /// An unassigned code point (`Cn`). Code: `UNICODE_FORBIDDEN_CATEGORY`.
    Unassigned {
        /// Offending scalar.
        scalar: char,
    },
    /// A Unicode noncharacter. Code: `UNICODE_NONCHARACTER`.
    Noncharacter {
        /// Offending scalar.
        scalar: char,
    },
}

impl UnicodeViolation {
    /// The stable error code from the registry for this violation.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            UnicodeViolation::NotNfc => "UNICODE_NOT_NFC",
            UnicodeViolation::ControlCharacter { .. }
            | UnicodeViolation::FormatCharacter { .. }
            | UnicodeViolation::PrivateUse { .. }
            | UnicodeViolation::Unassigned { .. } => "UNICODE_FORBIDDEN_CATEGORY",
            UnicodeViolation::Noncharacter { .. } => "UNICODE_NONCHARACTER",
        }
    }
}

fn map_violation(v: elang_unicode::UnicodeViolation) -> UnicodeViolation {
    match v {
        elang_unicode::UnicodeViolation::NotNfc => UnicodeViolation::NotNfc,
        elang_unicode::UnicodeViolation::Noncharacter { scalar } => {
            UnicodeViolation::Noncharacter { scalar }
        }
        // elang-unicode merges Cc/Cf/Co/Cn into ForbiddenCategory. The
        // repertoire/IPA callers only test for "forbidden"; report it as a
        // control-character variant whose code() is UNICODE_FORBIDDEN_CATEGORY.
        elang_unicode::UnicodeViolation::ForbiddenCategory { scalar } => {
            UnicodeViolation::ControlCharacter { scalar }
        }
    }
}

/// A validator over pinned Unicode 18.0.0 data. Zero-sized; construction is
/// kept for API compatibility.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnicodeValidator;

impl UnicodeValidator {
    /// Construct a validator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Is `value` in NFC? (steering §7).
    #[must_use]
    pub fn is_nfc(&self, value: &str) -> bool {
        elang_unicode::is_nfc(value)
    }

    /// Is `scalar` a Unicode noncharacter?
    #[must_use]
    pub fn is_noncharacter(scalar: char) -> bool {
        elang_unicode::is_noncharacter(scalar)
    }

    /// Validate a single scalar's category and noncharacter status.
    pub fn check_scalar(&self, scalar: char) -> Result<(), UnicodeViolation> {
        elang_unicode::check_scalar(scalar).map_err(map_violation)
    }

    /// Validate a whole ELD string (NFC + per-scalar checks).
    pub fn validate_str(&self, value: &str) -> Result<(), UnicodeViolation> {
        elang_unicode::validate_str(value).map_err(map_violation)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn pinned_version_is_18_and_matches() {
        assert_eq!(PINNED_UNICODE_VERSION, "18.0.0");
        assert_eq!(DATA_UNICODE_VERSION, "18.0.0");
        assert!(unicode_data_matches_pin());
    }

    #[test]
    fn accepts_plain_ascii_and_ipa() {
        let v = UnicodeValidator::new();
        assert!(v.validate_str("t").is_ok());
        assert!(v.validate_str("a").is_ok());
        assert!(v.validate_str("\u{00E1}").is_ok());
    }

    #[test]
    fn rejects_non_nfc() {
        let v = UnicodeValidator::new();
        assert_eq!(v.validate_str("a\u{0301}"), Err(UnicodeViolation::NotNfc));
    }

    #[test]
    fn rejects_forbidden_categories() {
        let v = UnicodeValidator::new();
        assert!(v.check_scalar('\u{0007}').is_err()); // Cc
        assert!(v.check_scalar('\u{200B}').is_err()); // Cf
        assert!(v.check_scalar('\u{E000}').is_err()); // Co
    }

    #[test]
    fn rejects_noncharacters() {
        let v = UnicodeValidator::new();
        for c in ['\u{FDD0}', '\u{FDEF}', '\u{FFFE}', '\u{FFFF}', '\u{10FFFF}'] {
            assert_eq!(
                v.check_scalar(c),
                Err(UnicodeViolation::Noncharacter { scalar: c })
            );
        }
    }

    #[test]
    fn violation_codes_match_registry() {
        assert_eq!(UnicodeViolation::NotNfc.code(), "UNICODE_NOT_NFC");
        assert_eq!(
            UnicodeViolation::Noncharacter { scalar: '\u{FFFF}' }.code(),
            "UNICODE_NONCHARACTER"
        );
        assert_eq!(
            UnicodeViolation::ControlCharacter { scalar: '\u{0007}' }.code(),
            "UNICODE_FORBIDDEN_CATEGORY"
        );
    }
}
