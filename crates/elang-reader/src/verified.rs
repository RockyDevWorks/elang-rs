//! `Verified` — public read-only view over an archive after stages 1–11.
//!
//! Holds the integrity manifest, the parsed `package.epf`, every resource
//! decoded under the verify pipeline, and the validated [`Inspected`] view.
//! Does **not** include the ELD phoneme inventory (that's the `Validated`
//! ceiling); use [`crate::Package::verify`] to obtain one.
//!
//! Bounded random-access retrieval (`T1208`) lives here because a verified
//! archive has its resources in memory: callers don't need to re-decompress
//! anything.

use elang_container::ArchiveListing;
use elang_core::error::Error;
use elang_epf::model::EpfPackage;
use elang_integrity::{IntegrityEntry, IntegrityManifest};

use crate::inspected::Inspected;

/// Public read-only view over an archive after stages 1–11 (T1202).
///
/// Construction is restricted to [`crate::Package::verify`]. A value of this
/// type proves the 11 stages completed in order: bootstrap resources agree
/// with the binary header, the integrity manifest is structurally valid and
/// covers every indexed resource, the EPF parses and resolves its phoneme
/// path against the index, and every resource's SHA-256 matches the manifest.
#[derive(Debug, Clone)]
pub struct Verified {
    inspected: Inspected,
    manifest: IntegrityManifest,
    epf: EpfPackage,
    /// Decoded resources keyed by their validated path. Paths are stored as
    /// `String` rather than `ResourcePath` to keep the lookup API trivial —
    /// `ResourcePath` is the canonical path type but readers here work with
    /// paths-as-strings end-to-end (the integrity manifest already validated
    /// them).
    decoded: Vec<(String, Vec<u8>)>,
}

impl Verified {
    /// Build from the verify-pipeline outputs. Restricted to the crate root.
    pub(crate) fn new(
        listing: ArchiveListing,
        manifest: IntegrityManifest,
        epf: EpfPackage,
        decoded: Vec<(String, Vec<u8>)>,
    ) -> Self {
        Self {
            inspected: Inspected::new(listing),
            manifest,
            epf,
            decoded,
        }
    }

    /// Downgrade to the [`Inspected`] view. Useful when a caller wants to
    /// release the decoded-resource cache but keep the listing.
    #[must_use]
    pub fn as_inspected(&self) -> Inspected {
        self.inspected.clone()
    }

    /// The parsed integrity manifest. Each [`IntegrityEntry`] carries the
    /// SHA-256 that the resource's bytes must match (stage 11 verified this).
    #[must_use]
    pub fn manifest(&self) -> &IntegrityManifest {
        &self.manifest
    }

    /// The parsed `package.epf`. Cross-field checks against the index (the
    /// phoneme path resolves to an indexed `.eld`) are already done at
    /// stage 10.
    #[must_use]
    pub fn epf(&self) -> &EpfPackage {
        &self.epf
    }

    /// Number of decoded resources cached in this view.
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.decoded.len()
    }

    /// Look up a decoded resource by its path.
    #[must_use]
    pub fn get_resource(&self, path: &str) -> Option<&[u8]> {
        self.decoded
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, b)| b.as_slice())
    }

    /// Iterate over `(path, decoded_bytes)` pairs in index order.
    ///
    /// Iteration is allocation-free and preserves the index ordering, which
    /// matches the manifest's and the listing's canonical orders.
    pub fn resources(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.decoded.iter().map(|(p, b)| (p.as_str(), b.as_slice()))
    }

    /// Look up the manifest entry for a given resource path.
    #[must_use]
    pub fn manifest_entry(&self, path: &str) -> Option<&IntegrityEntry> {
        self.manifest
            .resources
            .values()
            .find(|e| e.path.as_str() == path)
    }

    /// `Err` if `path` is missing from the index.
    pub fn require_resource(&self, path: &str) -> Result<&[u8], Error> {
        self.get_resource(path).ok_or_else(|| {
            use elang_core::error::{Category, Diagnostic};
            Error {
                diagnostic: Diagnostic::new(
                    "RESOURCE_MISSING",
                    Category::ResourcePath,
                    format!("archive index does not contain {path:?}"),
                ),
            }
        })
    }
}
