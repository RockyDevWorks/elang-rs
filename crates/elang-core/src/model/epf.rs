//! Immutable validated EPF (`package.epf`) model (REQ-CORE-MODEL-003).
//!
//! These types hold **already-validated** EPF data: the KDL parser and EPF
//! schema validator (Phase 6, `elang-epf`) construct them after enforcing the
//! full grammar (steering §5 / REQ-EPF-*). This module owns only the shape and
//! the intrinsic structural invariants that a model instance can enforce on its
//! own — chiefly that the language identifier URN is internally consistent and
//! that the manifest's `language-version` matches the URN's version suffix.
//!
//! Immutability contract: fields are private and every type is obtained only
//! through a fallible constructor, so holding an instance means the invariants
//! held at construction and cannot be mutated afterwards. There are no setters.

use crate::error::{Category, Error};
use crate::ids::ResourcePath;
use crate::version::SemVer;

/// Maximum `metadata.identifier` length, in UTF-8 bytes (steering §5).
const MAX_IDENTIFIER_BYTES: usize = 512;
/// Maximum `metadata.name` length, in UTF-8 bytes (steering §5).
const MAX_NAME_BYTES: usize = 256;
/// Maximum `metadata.description` length, in UTF-8 bytes (steering §5).
const MAX_DESCRIPTION_BYTES: usize = 4096;

/// A validated ELANG language identifier URN
/// (`urn:elang:language:<namespace>:<language-name>:<version>`, steering §5).
///
/// Guarantees: the full string is ≤ 512 UTF-8 bytes; the fixed prefix is
/// present; namespace and language-name are ASCII lowercase slugs
/// (`[a-z0-9]` with interior hyphens, alphanumeric ends); and the version is a
/// strict `MAJOR.MINOR.PATCH`. The parsed parts are retained so callers do not
/// re-split the string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageIdentifier {
    urn: String,
    namespace: String,
    language_name: String,
    version: SemVer,
}

impl LanguageIdentifier {
    /// Parse and validate a language-identifier URN.
    pub fn parse(s: impl Into<String>) -> Result<Self, Error> {
        let urn = s.into();
        if urn.is_empty() {
            return Err(err("EPF_IDENTIFIER_EMPTY", "language identifier is empty"));
        }
        if urn.len() > MAX_IDENTIFIER_BYTES {
            return Err(err(
                "EPF_IDENTIFIER_TOO_LONG",
                "language identifier exceeds 512 UTF-8 bytes",
            ));
        }
        // urn:elang:language:<namespace>:<language-name>:<version>
        // Split into exactly 6 colon-separated fields.
        let parts: Vec<&str> = urn.split(':').collect();
        let [p_urn, p_elang, p_language, namespace, language_name, version] = parts.as_slice()
        else {
            return Err(err(
                "EPF_IDENTIFIER_BAD_URN",
                "identifier must be urn:elang:language:<ns>:<name>:<version>",
            ));
        };
        if (*p_urn, *p_elang, *p_language) != ("urn", "elang", "language") {
            return Err(err(
                "EPF_IDENTIFIER_BAD_URN",
                "identifier must begin with urn:elang:language:",
            ));
        }
        if !valid_slug(namespace) {
            return Err(err("EPF_IDENTIFIER_BAD_SLUG", "invalid URN namespace slug"));
        }
        if !valid_slug(language_name) {
            return Err(err(
                "EPF_IDENTIFIER_BAD_SLUG",
                "invalid URN language-name slug",
            ));
        }
        let version = SemVer::parse(version)?;
        let namespace = (*namespace).to_string();
        let language_name = (*language_name).to_string();
        Ok(Self {
            urn,
            namespace,
            language_name,
            version,
        })
    }

    /// The full URN string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.urn
    }

    /// The URN namespace slug.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The URN language-name slug.
    #[must_use]
    pub fn language_name(&self) -> &str {
        &self.language_name
    }

    /// The URN version suffix.
    #[must_use]
    pub fn version(&self) -> SemVer {
        self.version
    }
}

/// An ASCII lowercase slug: `[a-z0-9]`, interior hyphens allowed, must begin and
/// end alphanumeric, nonempty (steering §5).
fn valid_slug(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let is_alnum = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    // Ends must be alphanumeric.
    match (bytes.first(), bytes.last()) {
        (Some(first), Some(last)) if is_alnum(first) && is_alnum(last) => {}
        _ => return false,
    }
    let mut prev_hyphen = false;
    for &b in bytes {
        match b {
            b'a'..=b'z' | b'0'..=b'9' => prev_hyphen = false,
            b'-' => {
                if prev_hyphen {
                    return false; // no double hyphen
                }
                prev_hyphen = true;
            }
            _ => return false,
        }
    }
    true
}

/// Validated EPF `metadata` block (steering §5 / REQ-EPF-006..011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    identifier: LanguageIdentifier,
    name: String,
    description: Option<String>,
}

impl Metadata {
    /// Construct validated metadata.
    ///
    /// Enforces the intrinsic length invariants on `name` (nonempty, ≤ 256
    /// bytes) and `description` (≤ 4096 bytes; empty is permitted). The
    /// `identifier` is already validated by its type.
    pub fn new(
        identifier: LanguageIdentifier,
        name: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, Error> {
        let name = name.into();
        if name.is_empty() {
            return Err(err("EPF_NAME_EMPTY", "metadata.name is empty"));
        }
        if name.len() > MAX_NAME_BYTES {
            return Err(err(
                "EPF_NAME_TOO_LONG",
                "metadata.name exceeds 256 UTF-8 bytes",
            ));
        }
        if let Some(d) = &description {
            if d.len() > MAX_DESCRIPTION_BYTES {
                return Err(err(
                    "EPF_DESCRIPTION_TOO_LONG",
                    "metadata.description exceeds 4096 UTF-8 bytes",
                ));
            }
        }
        Ok(Self {
            identifier,
            name,
            description,
        })
    }

    /// The language identifier.
    #[must_use]
    pub fn identifier(&self) -> &LanguageIdentifier {
        &self.identifier
    }

    /// The human-readable language name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The optional description (may be present and empty).
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Validated EPF `manifest` block (steering §5 / REQ-EPF-012).
///
/// v0.1.0 carries exactly one entry: the phoneme `.eld` resource path. The path
/// is a validated [`ResourcePath`]; whether it is actually present in the
/// archive index is checked by the container/EPF layer, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    phonemes: ResourcePath,
}

impl Manifest {
    /// Construct a manifest pointing at the phoneme resource.
    #[must_use]
    pub fn new(phonemes: ResourcePath) -> Self {
        Self { phonemes }
    }

    /// The phoneme `.eld` resource path.
    #[must_use]
    pub fn phonemes(&self) -> &ResourcePath {
        &self.phonemes
    }
}

/// An immutable, validated `package.epf` descriptor (steering §5).
///
/// Holds the five top-level nodes' validated values. The cross-field invariant
/// enforced here is REQ-EPF-008: `language-version` must equal the URN version
/// suffix exactly. EPF/ELD schema-version values being exactly `0.1.0` is
/// checked by the parser; this model stores them as [`SemVer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpfPackage {
    epf_version: SemVer,
    eld_version: SemVer,
    language_version: SemVer,
    metadata: Metadata,
    manifest: Manifest,
}

impl EpfPackage {
    /// Assemble a validated EPF descriptor from already-validated parts.
    ///
    /// Enforces REQ-EPF-008 (the `language-version` equals the identifier URN's
    /// version suffix). All other field-level grammar is enforced by the parser
    /// before these parts are built.
    pub fn new(
        epf_version: SemVer,
        eld_version: SemVer,
        language_version: SemVer,
        metadata: Metadata,
        manifest: Manifest,
    ) -> Result<Self, Error> {
        if language_version != metadata.identifier().version() {
            return Err(err(
                "EPF_VERSION_MISMATCH",
                "language-version must equal the URN version suffix",
            ));
        }
        Ok(Self {
            epf_version,
            eld_version,
            language_version,
            metadata,
            manifest,
        })
    }

    /// The EPF schema version (`epf` node value).
    #[must_use]
    pub fn epf_version(&self) -> SemVer {
        self.epf_version
    }

    /// The ELD schema version (`eld` node value).
    #[must_use]
    pub fn eld_version(&self) -> SemVer {
        self.eld_version
    }

    /// The language release version (`language-version` node value).
    #[must_use]
    pub fn language_version(&self) -> SemVer {
        self.language_version
    }

    /// The metadata block.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// The manifest block.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

fn err(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, Category::EpfSchema, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ver(s: &str) -> SemVer {
        SemVer::parse(s).unwrap()
    }

    #[test]
    fn language_identifier_parses_valid_urn() {
        let id = LanguageIdentifier::parse("urn:elang:language:rocky:aelith:1.0.0").unwrap();
        assert_eq!(id.namespace(), "rocky");
        assert_eq!(id.language_name(), "aelith");
        assert_eq!(id.version(), ver("1.0.0"));
        assert_eq!(id.as_str(), "urn:elang:language:rocky:aelith:1.0.0");
    }

    #[test]
    fn language_identifier_allows_hyphen_and_digit_slugs() {
        let id = LanguageIdentifier::parse("urn:elang:language:n1:old-high-aelith:2.3.4").unwrap();
        assert_eq!(id.namespace(), "n1");
        assert_eq!(id.language_name(), "old-high-aelith");
    }

    #[test]
    fn language_identifier_rejects_bad_forms() {
        // Wrong prefix.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:lang:rocky:aelith:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_URN"
        );
        // Wrong field count.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:language:rocky:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_URN"
        );
        // Uppercase slug.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:language:Rocky:aelith:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_SLUG"
        );
        // Leading hyphen.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:language:-x:aelith:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_SLUG"
        );
        // Trailing hyphen.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:language:x-:aelith:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_SLUG"
        );
        // Double hyphen.
        assert_eq!(
            LanguageIdentifier::parse("urn:elang:language:a--b:aelith:1.0.0")
                .unwrap_err()
                .code(),
            "EPF_IDENTIFIER_BAD_SLUG"
        );
        // Bad version (leading zero).
        assert!(LanguageIdentifier::parse("urn:elang:language:rocky:aelith:01.0.0").is_err());
        // Empty.
        assert_eq!(
            LanguageIdentifier::parse("").unwrap_err().code(),
            "EPF_IDENTIFIER_EMPTY"
        );
        // Too long.
        let long = format!("urn:elang:language:rocky:{}:1.0.0", "a".repeat(512));
        assert_eq!(
            LanguageIdentifier::parse(long).unwrap_err().code(),
            "EPF_IDENTIFIER_TOO_LONG"
        );
    }

    #[test]
    fn metadata_enforces_lengths() {
        let id = LanguageIdentifier::parse("urn:elang:language:rocky:aelith:1.0.0").unwrap();
        assert!(Metadata::new(id.clone(), "Aelith", None).is_ok());
        assert!(Metadata::new(id.clone(), "Aelith", Some(String::new())).is_ok());
        assert_eq!(
            Metadata::new(id.clone(), "", None).unwrap_err().code(),
            "EPF_NAME_EMPTY"
        );
        assert_eq!(
            Metadata::new(id.clone(), "n".repeat(257), None)
                .unwrap_err()
                .code(),
            "EPF_NAME_TOO_LONG"
        );
        assert_eq!(
            Metadata::new(id, "Aelith", Some("d".repeat(4097)))
                .unwrap_err()
                .code(),
            "EPF_DESCRIPTION_TOO_LONG"
        );
    }

    fn sample_metadata(version: &str) -> Metadata {
        let id = LanguageIdentifier::parse(format!("urn:elang:language:rocky:aelith:{version}"))
            .unwrap();
        Metadata::new(
            id,
            "Aelith",
            Some("A small constructed language.".to_string()),
        )
        .unwrap()
    }

    fn sample_manifest() -> Manifest {
        Manifest::new(ResourcePath::new("Linguistics/phonemes.eld").unwrap())
    }

    #[test]
    fn epf_package_accepts_consistent_versions() {
        let pkg = EpfPackage::new(
            ver("0.1.0"),
            ver("0.1.0"),
            ver("1.0.0"),
            sample_metadata("1.0.0"),
            sample_manifest(),
        )
        .unwrap();
        assert_eq!(pkg.epf_version(), ver("0.1.0"));
        assert_eq!(pkg.language_version(), ver("1.0.0"));
        assert_eq!(
            pkg.manifest().phonemes().as_str(),
            "Linguistics/phonemes.eld"
        );
        assert_eq!(pkg.metadata().name(), "Aelith");
    }

    #[test]
    fn epf_package_rejects_version_mismatch() {
        let e = EpfPackage::new(
            ver("0.1.0"),
            ver("0.1.0"),
            ver("2.0.0"),
            sample_metadata("1.0.0"),
            sample_manifest(),
        )
        .unwrap_err();
        assert_eq!(e.code(), "EPF_VERSION_MISMATCH");
    }

    /// Boundary case: a URN of exactly `MAX_IDENTIFIER_BYTES` (512) bytes
    /// must parse successfully. cargo-mutants mutates the length check
    /// `urn.len() > MAX_IDENTIFIER_BYTES` to `>=`; the boundary case
    /// (exactly 512 bytes) distinguishes the two forms. The previous
    /// "too long" test only covers lengths strictly greater than the cap,
    /// so it does not catch the off-by-one mutation.
    #[test]
    fn language_identifier_accepts_exactly_max_bytes() {
        // 19 ("urn:elang:language:") + 100 (namespace) + 1 (':') + 386
        // (language-name) + 1 (':') + 5 ("1.0.0") = 512 bytes — exactly
        // MAX_IDENTIFIER_BYTES.
        let urn = format!(
            "urn:elang:language:{}:{}:1.0.0",
            "a".repeat(100),
            "a".repeat(386),
        );
        assert_eq!(urn.len(), 512);
        let id = LanguageIdentifier::parse(urn).unwrap();
        assert_eq!(id.namespace().len(), 100);
        assert_eq!(id.language_name().len(), 386);
    }

    /// `Metadata::description` must round-trip the value set at
    /// construction. cargo-mutants mutates the getter to return `None`,
    /// `Some("")`, or `Some("xyzzy")`; only an assertion on the actual
    /// returned value catches all three substitutions.
    #[test]
    fn metadata_description_round_trips() {
        let id = LanguageIdentifier::parse("urn:elang:language:rocky:aelith:1.0.0").unwrap();
        let m = Metadata::new(
            id.clone(),
            "Aelith",
            Some("A small constructed language.".to_string()),
        )
        .unwrap();
        assert_eq!(m.description(), Some("A small constructed language."));

        // Absent description must surface as `None`, not as a substitute.
        let m_no_desc = Metadata::new(id, "Aelith", None).unwrap();
        assert_eq!(m_no_desc.description(), None);

        // Empty (but present) description must surface as `Some("")`,
        // not as `None` and not as some substituted literal.
        let id2 = LanguageIdentifier::parse("urn:elang:language:rocky:aelith:1.0.0").unwrap();
        let m_empty_desc = Metadata::new(id2, "Aelith", Some(String::new())).unwrap();
        assert_eq!(m_empty_desc.description(), Some(""));
    }
}
