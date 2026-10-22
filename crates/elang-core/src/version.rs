//! Version types and the strict `MAJOR.MINOR.PATCH` parser
//! (REQ-CORE-VERSION-001, REQ-CORE-VERSION-002).
//!
//! Each version domain has its own newtype so the compiler enforces that, for
//! example, an `EpfVersion` cannot be passed where a `ContainerVersion` is
//! expected. The inner `SemVer` is unchanged; the newtypes are zero-cost.

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

    /// Construct from individual components (for tests and constant tables).
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl core::fmt::Display for SemVer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Marker trait implemented by every version-domain newtype so generic code
/// can refer to "a version of some kind" without giving up domain separation.
pub trait Version: Copy + Eq + Ord + core::fmt::Display {
    /// Returns the underlying `SemVer`.
    fn semver(self) -> SemVer;
}

macro_rules! version_newtype {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(SemVer);

        impl $name {
            /// Parse the version string for this domain.
            pub fn parse(s: &str) -> Result<Self, Error> {
                Ok(Self(SemVer::parse(s)?))
            }

            /// Construct from a `SemVer` (caller must already have validated).
            pub const fn from_semver(v: SemVer) -> Self {
                Self(v)
            }

            /// Returns the underlying `SemVer`.
            pub const fn semver(self) -> SemVer {
                self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl Version for $name {
            fn semver(self) -> SemVer {
                self.0
            }
        }
    };
}

version_newtype!(
    /// Binary container (`file.elang`) format version.
    ContainerVersion
);

version_newtype!(
    /// EPF (`package.epf`) schema version.
    EpfVersion
);

version_newtype!(
    /// ELD phoneme-inventory schema version.
    EldVersion
);

version_newtype!(
    /// Language-definition version declared inside a package.
    LanguageVersion
);

/// Canonical compression profile identifier.
///
/// Not a `SemVer` — profiles are named and pinned by exact match. Use
/// [`CanonicalProfile::parse`] to validate user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalProfile(&'static str);

impl CanonicalProfile {
    /// The locked v0.1.0 profile name.
    pub const V1_NAME: &'static str = "elang-canonical-zstd-v1";

    /// The locked v0.1.0 profile, pre-validated.
    pub const V1: Self = Self(Self::V1_NAME);

    /// Parse and validate a canonical profile identifier.
    ///
    /// Accepts the locked v1 name verbatim. Other identifiers are rejected
    /// until a new profile is specified by the steering committee.
    pub fn parse(s: &str) -> Result<Self, Error> {
        if s == Self::V1_NAME {
            Ok(Self::V1)
        } else {
            Err(Error::new(
                "CANONICAL_PROFILE_UNKNOWN",
                Category::Container,
                "unknown canonical profile",
            ))
        }
    }

    /// Returns the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl core::fmt::Display for CanonicalProfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

impl Version for CanonicalProfile {
    fn semver(self) -> SemVer {
        // Profiles don't carry a SemVer; expose `1.0.0` so generic code has
        // something comparable.
        SemVer::new(1, 0, 0)
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
    s.parse::<u64>()
        .map_err(|_| bad("version component out of range"))
}

fn bad(msg: &'static str) -> Error {
    Error::new("EPF_BAD_VERSION", Category::EpfSchema, msg)
}

/// Locked version-domain constants (steering §1), typed for reuse.
pub mod domains {
    use super::{
        CanonicalProfile, ContainerVersion, EldVersion, EpfVersion, LanguageVersion, SemVer,
    };

    /// Spec release this implementation targets.
    pub const SPEC_RELEASE: &str = "0.0.1-1";

    /// Binary container version (typed).
    pub const CONTAINER: ContainerVersion = ContainerVersion::from_semver(SemVer::new(0, 1, 0));
    /// Binary container version (string form, for headers).
    pub const CONTAINER_VERSION: &str = "0.1.0";
    /// Container header major (`u16`).
    pub const CONTAINER_MAJOR: u16 = 0;
    /// Container header minor (`u16`).
    pub const CONTAINER_MINOR: u16 = 1;

    /// EPF schema version (typed).
    pub const EPF: EpfVersion = EpfVersion::from_semver(SemVer::new(0, 1, 0));
    /// EPF schema version (string form).
    pub const EPF_VERSION: &str = "0.1.0";

    /// ELD schema version (typed).
    pub const ELD: EldVersion = EldVersion::from_semver(SemVer::new(0, 1, 0));
    /// ELD schema version (string form).
    pub const ELD_VERSION: &str = "0.1.0";

    /// Language-definition version (typed, mirrors container for v0.1.0).
    pub const LANGUAGE: LanguageVersion = LanguageVersion::from_semver(SemVer::new(0, 1, 0));

    /// Canonical compression profile (typed).
    pub const CANONICAL: CanonicalProfile = CanonicalProfile::V1;
    /// Canonical compression profile identifier (string form).
    pub const CANONICAL_PROFILE: &str = CanonicalProfile::V1_NAME;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_versions() {
        assert_eq!(
            SemVer::parse("1.0.0").unwrap(),
            SemVer {
                major: 1,
                minor: 0,
                patch: 0
            }
        );
        assert_eq!(
            SemVer::parse("0.1.0").unwrap(),
            SemVer {
                major: 0,
                minor: 1,
                patch: 0
            }
        );
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

    #[test]
    fn rejects_overflowing_components() {
        // u64::MAX + 1 overflows; must be a parse error, not a panic.
        assert!(SemVer::parse("18446744073709551616.0.0").is_err());
        assert!(SemVer::parse("0.0.18446744073709551616").is_err());
        // u64::MAX itself is in range and must parse.
        assert!(SemVer::parse("18446744073709551615.0.0").is_ok());
    }

    #[test]
    fn domain_types_are_independent() {
        // Compiler enforces that you can't pass one where another is expected.
        fn takes_container(_: ContainerVersion) {}
        fn takes_epf(_: EpfVersion) {}
        fn takes_eld(_: EldVersion) {}
        fn takes_language(_: LanguageVersion) {}

        let cv = ContainerVersion::parse("0.1.0").unwrap();
        let ev = EpfVersion::parse("0.1.0").unwrap();
        let lv = EldVersion::parse("0.1.0").unwrap();
        let lang = LanguageVersion::parse("0.1.0").unwrap();

        takes_container(cv);
        takes_epf(ev);
        takes_eld(lv);
        takes_language(lang);

        // Equality is per-domain: the compiler refuses to compare across
        // domains (see the failed inference above), which is the whole
        // point of the newtypes. Inside a single domain, equal strings
        // yield equal values.
        assert_eq!(
            ContainerVersion::parse("0.1.0").unwrap(),
            ContainerVersion::parse("0.1.0").unwrap()
        );
        assert_eq!(
            EpfVersion::parse("0.1.0").unwrap(),
            EpfVersion::parse("0.1.0").unwrap()
        );
    }

    #[test]
    fn domain_types_reject_bad_input() {
        assert!(ContainerVersion::parse("1.0").is_err());
        assert!(EpfVersion::parse("01.0.0").is_err());
        assert!(EldVersion::parse("v1.0.0").is_err());
        assert!(LanguageVersion::parse("").is_err());
    }

    #[test]
    fn domain_constants_match_locked_values() {
        assert_eq!(domains::CONTAINER.to_string(), "0.1.0");
        assert_eq!(domains::EPF.to_string(), "0.1.0");
        assert_eq!(domains::ELD.to_string(), "0.1.0");
        assert_eq!(domains::LANGUAGE.to_string(), "0.1.0");
        assert_eq!(domains::CANONICAL_PROFILE, "elang-canonical-zstd-v1");
        assert_eq!(domains::CANONICAL.as_str(), "elang-canonical-zstd-v1");
        assert_eq!(
            domains::CANONICAL,
            CanonicalProfile::parse(domains::CANONICAL_PROFILE).unwrap()
        );
    }

    #[test]
    fn canonical_profile_parses_known_name() {
        let p = CanonicalProfile::parse(CanonicalProfile::V1_NAME).unwrap();
        assert_eq!(p.as_str(), "elang-canonical-zstd-v1");
        assert_eq!(p.to_string(), "elang-canonical-zstd-v1");
    }

    #[test]
    fn canonical_profile_rejects_unknown() {
        assert!(CanonicalProfile::parse("elang-canonical-zstd-v2").is_err());
        assert!(CanonicalProfile::parse("gzip").is_err());
        assert!(CanonicalProfile::parse("").is_err());
        // Case-sensitivity: the identifier is exact-match.
        assert!(CanonicalProfile::parse("ELANG-canonical-zstd-v1").is_err());
    }

    #[test]
    fn version_trait_round_trip() {
        let cv: ContainerVersion = ContainerVersion::parse("2.3.4").unwrap();
        let sm: SemVer = cv.semver();
        assert_eq!(sm, SemVer::new(2, 3, 4));
        assert_eq!(sm.to_string(), "2.3.4");

        // Generic use of the marker trait across domains.
        fn describe<V: Version>(v: V) -> String {
            format!("{v} (semver {})", v.semver())
        }
        assert_eq!(describe(cv), "2.3.4 (semver 2.3.4)");
        assert_eq!(describe(domains::EPF), "0.1.0 (semver 0.1.0)");
        assert_eq!(
            describe(domains::CANONICAL),
            "elang-canonical-zstd-v1 (semver 1.0.0)"
        );
    }
}
