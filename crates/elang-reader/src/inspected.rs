//! `Inspected` — public read-only view over an archive after stages 1–7.
//!
//! Cheap to construct (the listing is held by [`crate::Package`]) and to
//! copy. Exposes only structural metadata: the binary header, the resource
//! index and the byte spans of each frame. No resource is decompressed, so
//! `Inspected` is the right handle for callers that just want to enumerate
//! what's in a package.
//!
//! [`crate::Package::inspect`] is the only way to obtain an `Inspected`. The
//! type is `Clone` so callers can hold one past the lifetime of the
//! originating [`crate::Package`] (which they typically `drop` after
//! inspecting, since they only wanted the listing).

use elang_container::ArchiveListing;

/// Public read-only view over an archive after stages 1–7 (T1201).
///
/// Construction is restricted to the [`crate::Package::inspect`] method, so a
/// value of this type proves the listing's invariants held at construction
/// (header, footer, index hash, sorted unique paths, contiguous frame layout,
/// per-frame header inspection).
#[derive(Debug, Clone)]
pub struct Inspected {
    listing: ArchiveListing,
}

impl Inspected {
    /// Build from an already-parsed listing. Restricted to the crate root.
    pub(crate) fn new(listing: ArchiveListing) -> Self {
        Self { listing }
    }

    /// Borrow the underlying [`ArchiveListing`]. Useful for callers that need
    /// frame offsets or the binary header directly.
    #[must_use]
    pub fn listing(&self) -> &ArchiveListing {
        &self.listing
    }

    /// Number of indexed resources.
    #[must_use]
    pub fn len(&self) -> usize {
        self.listing.len()
    }

    /// Whether the archive has no resources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.listing.is_empty()
    }

    /// All resource paths in index order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.listing
            .index()
            .entries()
            .iter()
            .map(|e| e.path().as_str())
            .collect()
    }

    /// Binary header version as `(major, minor)`.
    #[must_use]
    pub fn header_version(&self) -> (u16, u16) {
        (self.listing.header().major(), self.listing.header().minor())
    }

    /// Total declared uncompressed bytes across all resources.
    #[must_use]
    pub fn total_uncompressed(&self) -> u64 {
        self.listing
            .index()
            .entries()
            .iter()
            .map(|e| e.uncompressed_size())
            .fold(0u64, |acc, n| acc.saturating_add(n))
    }
}
