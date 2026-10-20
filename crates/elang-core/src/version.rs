//! Version types and the strict `MAJOR.MINOR.PATCH` parser
//! (REQ-CORE-VERSION-001, REQ-CORE-VERSION-002).

use crate::error::{Category, Error};

/// A strict semantic-looking version `MAJOR.MINOR.PATCH` with no leading zeros.
///
/// ELANG language releases use this exact form (steering §1): three decimal
/// components, each either a lone `0` or a nonzero digit followed by more
/// digits (no leading zeros), no prerelease or build metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemVer {
    /// Major component.
    pub major: u64,
    /// Minor component.
    pub minor: u64,
    /// Patch component.
    pub patch: u64,
}

impl SemVer {
    /// Parse a strict `MAJOR.MINOR.PATCH` string.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let mut it = s.split('.');
        let (Some(a), Some(b), Some(c), None) = (it.next(), it.next(), it.next(), it.next()) else {
            return Err(bad("expected exactly MAJOR.MINOR.PATCH"));
        };
        Ok(Self {
            major: parse_component(a)?,
            minor: parse_component(b)?,
            patch: parse_component(c)?,
        })
    }
}

impl core::fmt::Display for SemVer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_component(s: &str) -> Result<u64, Error> {
    if s.is_empty() {
        return Err(bad("empty version component"));
    }
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad("non-digit in version component"));
    }
    // No leading zeros: a lone "0" is allowed; "01", "007" are not.
    if s.len() > 1 && s.starts_with('0') {
        return Err(bad("leading zero in version component"));
    }
    s.parse::<u64>().map_err(|_| bad("version component out of range"))
}

fn bad(msg: &'static str) -> Error {
    Error::new("EPF_BAD_VERSION", Category::EpfSchema, msg)
}

/// Locked version-domain constants (steering §1), typed for reuse.
pub mod domains {
    /// Spec release this implementation targets.
    pub const SPEC_RELEASE: &str = "0.0.1-1";
    /// Binary container version.
    pub const CONTAINER_VERSION: &str = "0.1.0";
    /// Container header major.
    pub const CONTAINER_MAJOR: u16 = 0;
    /// Container header minor.
    pub const CONTAINER_MINOR: u16 = 1;
    /// EPF schema version.
    pub const EPF_VERSION: &str = "0.1.0";
    /// ELD schema version.
    pub const ELD_VERSION: &str = "0.1.0";
    /// Canonical compression profile identifier.
    pub const CANONICAL_PROFILE: &str = "elang-canonical-zstd-v1";
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_versions() {
        assert_eq!(SemVer::parse("1.0.0").unwrap(), SemVer { major: 1, minor: 0, patch: 0 });
        assert_eq!(SemVer::parse("0.1.0").unwrap(), SemVer { major: 0, minor: 1, patch: 0 });
        assert_eq!(SemVer::parse("12.34.56").unwrap().minor, 34);
    }

    #[test]
    fn rejects_leading_zeros() {
        assert!(SemVer::parse("01.0.0").is_err());
        assert!(SemVer::parse("1.00.0").is_err());
        assert!(SemVer::parse("1.0.007").is_err());
    }

    #[test]
    fn rejects_wrong_shape() {
        assert!(SemVer::parse("1.0").is_err());
        assert!(SemVer::parse("1.0.0.0").is_err());
        assert!(SemVer::parse("1.0.0-rc1").is_err());
        assert!(SemVer::parse("1.0.x").is_err());
        assert!(SemVer::parse("").is_err());
        assert!(SemVer::parse("v1.0.0").is_err());
    }

    #[test]
    fn roundtrips_display() {
        let v = SemVer::parse("3.5.9").unwrap();
        assert_eq!(v.to_string(), "3.5.9");
    }
}
