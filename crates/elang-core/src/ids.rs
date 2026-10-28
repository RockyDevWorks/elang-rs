//! Validated newtypes (REQ-CORE-MODEL-001, REQ-CORE-MODEL-002).
//!
//! Each type's only constructor is fallible, so a value of the type is
//! unrepresentable-if-invalid: holding a `ResourcePath` means the path passed
//! the steering §3.4 rules. Validation that needs external data (the IPA
//! allowlist) lives in `elang-repertoire`; here we provide the self-contained
//! types plus a wrapper for already-validated IPA identifier text.

use crate::error::{Category, Error};
use crate::limits::MAX_RESOURCE_PATH_BYTES;

/// A validated, package-relative resource path (steering §3.4).
///
/// Guarantees: valid UTF-8 (it is a `String`), ≤ 1024 UTF-8 bytes, no leading
/// `/`, no `\`, no NUL, and no empty / `.` / `..` segment. NFC is checked by
/// the caller using `elang-unicode` before construction here, since this crate
/// has no Unicode dependency; callers that hold untrusted bytes MUST NFC-check
/// first (the container layer does).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourcePath(String);

impl ResourcePath {
    /// Validate and construct. The input is assumed already NFC-checked.
    pub fn new(s: impl Into<String>) -> Result<Self, Error> {
        let s = s.into();
        if s.is_empty() {
            return Err(err(
                Category::ResourcePath,
                "PATH_EMPTY_SEGMENT",
                "empty path",
            ));
        }
        if s.len() > MAX_RESOURCE_PATH_BYTES {
            return Err(err(
                Category::ResourcePath,
                "PATH_TOO_LONG",
                "resource path exceeds 1024 UTF-8 bytes",
            ));
        }
        if s.starts_with('/') {
            return Err(err(
                Category::ResourcePath,
                "PATH_ABSOLUTE",
                "absolute path",
            ));
        }
        // Drive-letter / Windows absolute (e.g. "C:...") is also absolute.
        if looks_drive_absolute(&s) {
            return Err(err(
                Category::ResourcePath,
                "PATH_ABSOLUTE",
                "drive-absolute path",
            ));
        }
        if s.contains('\\') {
            return Err(err(
                Category::ResourcePath,
                "PATH_BACKSLASH",
                "backslash in path",
            ));
        }
        if s.contains('\0') {
            return Err(err(Category::ResourcePath, "PATH_NUL", "NUL in path"));
        }
        for segment in s.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(err(
                    Category::ResourcePath,
                    "PATH_EMPTY_SEGMENT",
                    "empty, '.' or '..' path segment",
                ));
            }
        }
        Ok(Self(s))
    }

    /// The validated path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Raw UTF-8 bytes (the sort/comparison key for the index, steering §3.2).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

fn looks_drive_absolute(s: &str) -> bool {
    matches!(s.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic())
}

/// A validated custom group name: ASCII `[a-z]+(?:-[a-z]+)*`, 1–32 chars
/// (steering §6.1). `consonants`/`vowels` are reserved and rejected here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupName(String);

impl GroupName {
    /// Validate and construct a custom group name.
    pub fn new_custom(s: impl Into<String>) -> Result<Self, Error> {
        let s = s.into();
        if s == "consonants" || s == "vowels" {
            return Err(err(
                Category::EldSchema,
                "ELD_GROUP_DUPLICATE",
                "reserved group name used as custom",
            ));
        }
        if !valid_group_slug(&s) {
            return Err(err(
                Category::EldSchema,
                "ELD_GROUP_INVALID",
                "custom group name must match [a-z]+(?:-[a-z]+)* and be 1-32 chars",
            ));
        }
        Ok(Self(s))
    }

    /// The group name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_group_slug(s: &str) -> bool {
    if s.is_empty() || s.len() > 32 {
        return false;
    }
    // [a-z]+(?:-[a-z]+)* : hyphen-separated nonempty lowercase runs.
    let mut prev_hyphen = true; // treat start as "after a boundary" => first must be a-z
    let mut last_was_hyphen = false;
    for (i, c) in s.char_indices() {
        match c {
            'a'..='z' => {
                prev_hyphen = false;
                last_was_hyphen = false;
            }
            '-' => {
                if prev_hyphen || i == 0 {
                    return false; // leading hyphen or empty run before hyphen
                }
                prev_hyphen = true;
                last_was_hyphen = true;
            }
            _ => return false,
        }
    }
    !last_was_hyphen // must not end with a hyphen
}

/// A validated romanization value: ASCII `[A-Za-z]{1,8}`, case preserved
/// (steering §6.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Romanization(String);

impl Romanization {
    /// Validate and construct.
    pub fn new(s: impl Into<String>) -> Result<Self, Error> {
        let s = s.into();
        let ok = (1..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic());
        if !ok {
            return Err(err(
                Category::Roman,
                "ROMAN_BAD_FORMAT",
                "romanization must match [A-Za-z]{1,8}",
            ));
        }
        Ok(Self(s))
    }

    /// The romanization as a string slice (original case preserved).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// ASCII-lowercase folding, for case-insensitive uniqueness comparison.
    #[must_use]
    pub fn fold_key(&self) -> String {
        self.0.to_ascii_lowercase()
    }
}

/// Already-validated IPA identifier text. Construction is guarded: this crate
/// cannot validate IPA (no allowlist), so the only constructor is
/// [`IpaIdentifierText::from_validated`], called by `elang-repertoire` after a
/// successful repertoire check. This keeps "valid" unrepresentable-if-invalid
/// without pulling the allowlist into `elang-core`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IpaIdentifierText(String);

impl IpaIdentifierText {
    /// Wrap text that the IPA validator has already accepted.
    #[must_use]
    pub fn from_validated(s: String) -> Self {
        Self(s)
    }

    /// The identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn err(category: Category, code: &'static str, msg: &'static str) -> Error {
    Error::new(code, category, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resource_path_accepts_valid() {
        let p = ResourcePath::new("Linguistics/phonemes.eld").unwrap();
        assert_eq!(p.as_str(), "Linguistics/phonemes.eld");
    }

    #[test]
    fn resource_path_rejects_bad_forms() {
        assert_eq!(
            ResourcePath::new("/abs").unwrap_err().code(),
            "PATH_ABSOLUTE"
        );
        assert_eq!(
            ResourcePath::new("C:/x").unwrap_err().code(),
            "PATH_ABSOLUTE"
        );
        assert_eq!(
            ResourcePath::new("a\\b").unwrap_err().code(),
            "PATH_BACKSLASH"
        );
        assert_eq!(
            ResourcePath::new("a/../b").unwrap_err().code(),
            "PATH_EMPTY_SEGMENT"
        );
        assert_eq!(
            ResourcePath::new("a//b").unwrap_err().code(),
            "PATH_EMPTY_SEGMENT"
        );
        assert_eq!(
            ResourcePath::new("a/./b").unwrap_err().code(),
            "PATH_EMPTY_SEGMENT"
        );
        assert_eq!(
            ResourcePath::new("").unwrap_err().code(),
            "PATH_EMPTY_SEGMENT"
        );
        assert_eq!(ResourcePath::new("a\0b").unwrap_err().code(), "PATH_NUL");
        assert_eq!(
            ResourcePath::new("x".repeat(1025)).unwrap_err().code(),
            "PATH_TOO_LONG"
        );
    }

    #[test]
    fn group_name_rules() {
        assert!(GroupName::new_custom("diphthongs").is_ok());
        assert!(GroupName::new_custom("click-consonants").is_ok());
        assert_eq!(
            GroupName::new_custom("consonants").unwrap_err().code(),
            "ELD_GROUP_DUPLICATE"
        );
        assert_eq!(
            GroupName::new_custom("Vowels2").unwrap_err().code(),
            "ELD_GROUP_INVALID"
        );
        assert_eq!(
            GroupName::new_custom("-x").unwrap_err().code(),
            "ELD_GROUP_INVALID"
        );
        assert_eq!(
            GroupName::new_custom("x-").unwrap_err().code(),
            "ELD_GROUP_INVALID"
        );
        assert_eq!(
            GroupName::new_custom("a--b").unwrap_err().code(),
            "ELD_GROUP_INVALID"
        );
        assert_eq!(
            GroupName::new_custom("a".repeat(33)).unwrap_err().code(),
            "ELD_GROUP_INVALID"
        );
    }

    /// Boundary case: a slug of exactly the maximum 32 characters must
    /// be accepted. cargo-mutants mutates the length check
    /// `s.len() > 32` in `valid_group_slug` to `>=`; the boundary
    /// case (exactly 32 chars) distinguishes the two forms. The
    /// existing `group_name_rules` test only covers 33 chars, which
    /// does not catch the off-by-one mutation.
    #[test]
    fn group_name_accepts_exactly_max_length_slug() {
        let slug = "a".repeat(32);
        assert_eq!(slug.len(), 32);
        assert!(
            GroupName::new_custom(&slug).is_ok(),
            "32-char slug must be accepted (the cap is exclusive: len > 32 rejects)"
        );
    }

    #[test]
    fn romanization_rules_and_fold() {
        assert!(Romanization::new("th").is_ok());
        assert_eq!(Romanization::new("TH").unwrap().fold_key(), "th");
        assert_eq!(
            Romanization::new("").unwrap_err().code(),
            "ROMAN_BAD_FORMAT"
        );
        assert_eq!(
            Romanization::new("toolongvalue").unwrap_err().code(),
            "ROMAN_BAD_FORMAT"
        );
        assert_eq!(
            Romanization::new("a1").unwrap_err().code(),
            "ROMAN_BAD_FORMAT"
        );
    }
}
