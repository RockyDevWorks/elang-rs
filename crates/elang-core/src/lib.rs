//! # elang-core
//!
//! Core primitives shared across the ELANG reference implementation: stable
//! identifiers, error categories, resource/parser limits and validated model
//! types.
//!
//! This crate is intentionally dependency-light and contains no I/O. Everything
//! here must be safe to use on untrusted input: no panics, no unchecked
//! indexing, and checked integer arithmetic.
//!
//! See `.kiro/specs/elang-v0.1.0/design.md` for the crate's role in the overall
//! architecture and `requirements.md` for the requirements it implements.

/// Version-domain constants locked by the ELANG steering contract (§1).
///
/// These version domains are independent of one another; see the steering
/// document and `requirements.md` (REQ-CORE-VERSION-*) for the normative rules.
pub mod version_domains {
    /// ELANG specification release this implementation targets.
    pub const SPEC_RELEASE: &str = "0.0.1-1";

    /// Binary container format version (header major.minor).
    pub const CONTAINER_VERSION: &str = "0.1.0";
    /// Binary container header major version (`u16`).
    pub const CONTAINER_MAJOR: u16 = 0;
    /// Binary container header minor version (`u16`).
    pub const CONTAINER_MINOR: u16 = 1;

    /// EPF (`package.epf`) schema version.
    pub const EPF_VERSION: &str = "0.1.0";
    /// ELD phoneme-inventory schema version.
    pub const ELD_VERSION: &str = "0.1.0";

    /// Canonical compression profile identifier.
    pub const CANONICAL_PROFILE: &str = "elang-canonical-zstd-v1";
}

#[cfg(test)]
mod tests {
    use super::version_domains::*;

    #[test]
    fn version_domains_are_locked() {
        assert_eq!(SPEC_RELEASE, "0.0.1-1");
        assert_eq!(CONTAINER_VERSION, "0.1.0");
        assert_eq!(CONTAINER_MAJOR, 0);
        assert_eq!(CONTAINER_MINOR, 1);
        assert_eq!(EPF_VERSION, "0.1.0");
        assert_eq!(ELD_VERSION, "0.1.0");
        assert_eq!(CANONICAL_PROFILE, "elang-canonical-zstd-v1");
    }
}
