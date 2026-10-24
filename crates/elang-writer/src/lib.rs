//! # elang-writer
//!
//! ELANG v0.1.0 package writer (Issue #23, Phase 13, T1301–T1309).
//!
//! This crate builds a `.elang` archive from already-validated inputs:
//!
//! - a parsed [`EpfPackage`](elang_epf::model::EpfPackage),
//! - a parsed [`PhonemeInventory`](elang_eld::PhonemeInventory),
//! - an optional list of additional permitted resources (paths plus their
//!   canonical bytes — see ADR 0006 / `specs/registry/resource-policy-v0.1.0.md`),
//! - a pinned [`format.version`] string (semver-shaped; e.g. `"0.1.0\n"`).
//!
//! [`format.version`]: ../struct.PackageConfig.html#structfield.format_version
//!
//! The writer is intentionally thin: it does not re-validate the EPF or ELD,
//! does not parse KDL, and does not run the 12-stage reader. Every input is
//! assumed to have already cleared the upstream validators. The writer's own
//! job is composition: serialize each resource with its ratified canonical
//! profile, compute the integrity manifest over the final uncompressed bytes,
//! encode each resource as an independent Zstandard frame, build the index,
//! compute the footer hash, and assemble the binary container.
//!
//! ## What the writer deliberately does not do
//!
//! - It does **not** check that `package.epf` references every required
//!   resource; that contract is owned by the reader's stage 8 (bootstrap).
//! - It does **not** verify the IPA repertoire against `specs/schema/ipa-v1.json`;
//!   the [`PhonemeInventory`] is assumed to be already validated.
//! - It does **not** run a second pass for KDL canonicalization edge cases
//!   beyond what the per-resource `canon` modules expose.
//!
//! ## Diagnostic model
//!
//! Every failure carries a registered error code from
//! `specs/registry/error-codes.md` and the [`Category`] the registry assigns.
//! Container-layer failures (`Category::Container`, `Category::Index`,
//! `Category::Frame`, `Category::ResourcePath`) propagate from
//! `elang-container` unchanged; canonical serialization and integrity failures
//! carry the upstream crate's category. The writer adds one new category
//! [`Category::Builder`] for its own pre-flight checks (resource-count ceiling,
//! size ceilings, profile/format-version sanity).

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod build;
mod config;
mod error;

pub use config::{ExtraResource, PackageConfig};
pub use error::{BuildError, SizeKind};

pub use elang_container::frame::Profile;
pub use elang_core::model::eld::PhonemeInventory;
pub use elang_epf::model::EpfPackage;
pub use elang_integrity::manifest::Algorithm;

/// Assemble a `.elang` archive from a [`PackageConfig`].
///
/// This is the single public entry point. It performs, in order:
///
/// 1. Pre-flight validation (resource count, path uniqueness, format-version
///    shape, size ceilings — see [`BuildError`] for the failure catalogue).
/// 2. Canonical serialization of EPF, ELD, and the integrity manifest over
///    the final uncompressed bytes (each via its ratified `canon` module).
/// 3. Canonical sort of every resource path (raw-UTF-8 ascending).
/// 4. Per-resource Zstandard frame encoding at the requested [`Profile`].
/// 5. Index construction via [`Index::with_entries`](elang_container::Index::with_entries).
/// 6. Footer SHA-256 over the exact index bytes.
/// 7. Final byte assembly: header → contiguous frames → index → footer.
///
/// The returned bytes are a complete, byte-correct archive that the reader's
/// 12-stage pipeline accepts; the integration tests roundtrip the output
/// through `elang-reader` to prove it.
///
/// # Errors
///
/// See [`BuildError`] for the full catalogue.
pub fn write_archive(config: &PackageConfig) -> Result<Vec<u8>, BuildError> {
    build::write_archive(config)
}
