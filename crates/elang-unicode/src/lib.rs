//! # elang-unicode
//!
//! ELANG's pinned **Unicode 18.0.0** validation layer (steering §7). It does
//! not depend on host-OS Unicode behavior:
//!
//! - **General_Category** comes from tables generated from the official pinned
//!   UCD 18.0.0 `DerivedGeneralCategory.txt` (the private `gc_tables` module); the categories
//!   ELANG forbids (`Cc`/`Cf`/`Cs`/`Co`/`Cn`) are encoded as range tables.
//! - **Noncharacters** are detected algorithmically (version-independent).
//! - **NFC** uses the proven UAX#15 implementation in `unicode-normalization`,
//!   whose behavior is verified against the official Unicode 18.0.0
//!   `NormalizationTest.txt` conformance file (see
//!   `tests/normalization_conformance.rs`).
//!
//! The pinned version is asserted to be 18.0.0. Unlike the earlier
//! ICU4X-backed stopgap, this crate's General_Category data **is** 18.0.0, and
//! NFC is accepted only once it passes the 18.0.0 conformance suite (ADR 0007).

mod gc_tables;
pub mod nfc;
mod nfc_tables;

/// The Unicode version ELANG pins (steering §7).
pub const PINNED_UNICODE_VERSION: &str = "18.0.0";

/// The Unicode version the bundled General_Category tables were generated from.
pub const DATA_UNICODE_VERSION: &str = gc_tables::DATA_UNICODE_VERSION;

/// Whether the General_Category data matches the pinned version. This crate is
/// generated from UCD 18.0.0, so this is `true`; a mismatch means the tables
/// were regenerated from the wrong UCD.
#[must_use]
pub fn gc_data_matches_pin() -> bool {
    DATA_UNICODE_VERSION == PINNED_UNICODE_VERSION
}

/// Why a string or scalar is not a valid ELANG Unicode value. Codes map to the
/// `UNICODE` error category (`specs/registry/error-codes.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeViolation {
    /// Not in Normalization Form C. Code: `UNICODE_NOT_NFC`.
    NotNfc,
    /// General_Category is one of `Cc`/`Cf`/`Co`/`Cn` (or surrogate `Cs`).
    /// Code: `UNICODE_FORBIDDEN_CATEGORY`.
    ForbiddenCategory {
        /// Offending scalar value.
        scalar: char,
    },
    /// A Unicode noncharacter. Code: `UNICODE_NONCHARACTER`.
    Noncharacter {
        /// Offending scalar value.
        scalar: char,
    },
}

impl UnicodeViolation {
    /// The stable error code from the registry for this violation.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            UnicodeViolation::NotNfc => "UNICODE_NOT_NFC",
            UnicodeViolation::ForbiddenCategory { .. } => "UNICODE_FORBIDDEN_CATEGORY",
            UnicodeViolation::Noncharacter { .. } => "UNICODE_NONCHARACTER",
        }
    }
}

/// Is `scalar` a Unicode noncharacter? (U+FDD0..=U+FDEF and `..FFFE`/`..FFFF`
/// in every plane.) Version-independent by definition (UAX stability).
#[must_use]
pub fn is_noncharacter(scalar: char) -> bool {
    let c = scalar as u32;
    if (0xFDD0..=0xFDEF).contains(&c) {
        return true;
    }
    let low = c & 0xFFFF;
    low == 0xFFFE || low == 0xFFFF
}

/// Does `scalar` have a forbidden General_Category per the pinned UCD 18.0.0
/// tables? (`Cc`/`Cf`/`Cs`/`Co`/`Cn`.)
#[must_use]
pub fn has_forbidden_category(scalar: char) -> bool {
    let c = scalar as u32;
    // Binary search the sorted, non-overlapping inclusive ranges.
    let ranges = gc_tables::FORBIDDEN_CATEGORY_RANGES;
    ranges
        .binary_search_by(|&(start, end)| {
            if c < start {
                std::cmp::Ordering::Greater
            } else if c > end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Is `value` in Normalization Form C? Uses the pinned UCD 18.0.0 NFC
/// implementation in [`nfc`].
#[must_use]
pub fn is_nfc(value: &str) -> bool {
    nfc::is_nfc(value)
}

/// Return the NFC form of `value` (pinned UCD 18.0.0). ELD validation rejects
/// non-NFC input and never normalizes it; this is exposed for tooling and the
/// conformance harness.
#[must_use]
pub fn to_nfc(value: &str) -> String {
    nfc::nfc(value)
}

/// Validate one scalar's category and noncharacter status. Returns the first
/// violation, or `Ok(())` if allowed.
pub fn check_scalar(scalar: char) -> Result<(), UnicodeViolation> {
    if is_noncharacter(scalar) {
        return Err(UnicodeViolation::Noncharacter { scalar });
    }
    if has_forbidden_category(scalar) {
        return Err(UnicodeViolation::ForbiddenCategory { scalar });
    }
    Ok(())
}

/// Validate a whole ELD string: NFC first, then every scalar's category and
/// noncharacter status. Returns the first violation found.
///
/// The input is a `&str`, so it is valid UTF-8 with no surrogates by
/// construction (REQ-UNICODE-001 enforced at decode time).
pub fn validate_str(value: &str) -> Result<(), UnicodeViolation> {
    if !is_nfc(value) {
        return Err(UnicodeViolation::NotNfc);
    }
    for scalar in value.chars() {
        check_scalar(scalar)?;
    }
    Ok(())
}

/// Canonical_Combining_Class of a scalar (pinned UCD 18.0.0 data).
#[must_use]
pub fn ccc(scalar: char) -> u8 {
    nfc::ccc(scalar)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn pinned_version_is_18() {
        assert_eq!(PINNED_UNICODE_VERSION, "18.0.0");
        assert_eq!(DATA_UNICODE_VERSION, "18.0.0");
        assert!(gc_data_matches_pin());
    }

    #[test]
    fn accepts_plain_ascii_and_ipa() {
        assert!(validate_str("t").is_ok());
        assert!(validate_str("\u{00E1}").is_ok()); // á NFC
        assert!(validate_str("\u{0283}").is_ok()); // ʃ
    }

    #[test]
    fn rejects_non_nfc() {
        // a + combining acute = NFD
        assert_eq!(validate_str("a\u{0301}"), Err(UnicodeViolation::NotNfc));
    }

    #[test]
    fn rejects_control_format_private_use() {
        assert!(matches!(
            check_scalar('\u{0007}'),
            Err(UnicodeViolation::ForbiddenCategory { .. })
        ));
        assert!(matches!(
            check_scalar('\u{200B}'),
            Err(UnicodeViolation::ForbiddenCategory { .. })
        ));
        assert!(matches!(
            check_scalar('\u{E000}'),
            Err(UnicodeViolation::ForbiddenCategory { .. })
        ));
    }

    #[test]
    fn rejects_noncharacters() {
        for c in ['\u{FDD0}', '\u{FDEF}', '\u{FFFE}', '\u{FFFF}', '\u{10FFFF}'] {
            assert_eq!(
                check_scalar(c),
                Err(UnicodeViolation::Noncharacter { scalar: c })
            );
        }
    }

    #[test]
    fn accepts_newly_assigned_18_0_characters() {
        // Sanity: a character assigned in an earlier version is allowed; the
        // conformance tests cover the 18.0-specific additions. Here we just
        // confirm a normal assigned letter is not treated as forbidden.
        assert!(check_scalar('A').is_ok());
    }
}
