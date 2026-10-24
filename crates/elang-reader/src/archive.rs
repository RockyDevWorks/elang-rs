//! Bounded, index-based random access over a parsed archive's resources.
//!
//! After [`crate::Package::open`] has run stages 1–7, the [`ArchiveView`]
//! exposes:
//!
//! * Path-based lookup (powers [`crate::Package::verify`]'s stage-10 manifest
//!   resolution against the EPF layer's [`IndexLookup`] trait).
//! * Bounded random-access retrieval by index, used by the `Validated` public
//!   SDK API.
//! * Total resource count and aggregate uncompressed size for diagnostics
//!   and budget accounting.
//!
//! Bounds are enforced at the call site:
//!
//! * `path` lookups never decompress — they return either a [`Span`] (for
//!   on-disk access) or `None`.
//! * Index lookups return `None` on out-of-range indices, never panic.
//! * Span-to-bytes conversion (used by [`crate::Package::verify`] and the
//!   bootstrap stage) is the only place archive-byte slicing happens; it
//!   checks `offset + compressed_size <= archive.len()` with checked
//!   arithmetic and returns `None` otherwise.
//!
//! [`IndexLookup`]: elang_epf::manifest::IndexLookup

use elang_container::layout::FrameLayout;
use elang_container::Index;
use elang_core::ResourcePath;

/// An on-disk resource span inside the archive.
///
/// `compressed_size` is the on-disk Zstd frame size; decompressing yields at
/// most `uncompressed_size` bytes (verified by [`crate::Package::verify`] at
/// stage 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Absolute byte offset of the frame in the archive.
    pub offset: u64,
    /// Compressed frame size in bytes.
    pub compressed_size: u64,
    /// Declared uncompressed size in bytes.
    pub uncompressed_size: u64,
}

/// Read-only, index-backed view over an opened archive's resources.
///
/// Cheap to construct — it borrows the parsed [`Index`] from the container
/// layer. No bytes are read until `Span::read_span` or
/// [`crate::Package::verify`] asks for them.
#[derive(Debug, Clone)]
pub struct ArchiveView<'a> {
    /// Borrow marker — the [`Index`] this view was built from. Held so that
    /// the view's lifetime outlives the index entries it copied into
    /// `entries`.
    _index: &'a Index,
    /// Resolved layout: every resource's path, frame offset and sizes.
    /// Built once by [`ArchiveView::new`] from the listing's frame layout.
    entries: Vec<ArcEntry>,
}

/// One resource's path + frame-located metadata.
///
/// Paths are stored as `String` (not [`ResourcePath`]) because the container
/// layer validates them with NFC checks already; converting back would
/// re-run the same checks for no benefit and would lose the original byte
/// view the EPF layer's
/// [`elang_epf::manifest::IndexLookup::contains`] expects.
#[derive(Debug, Clone)]
struct ArcEntry {
    path: String,
    span: Span,
}

impl<'a> ArchiveView<'a> {
    /// Build a view from a parsed index and its validated frame layout.
    ///
    /// The two are required to be in the same resource order (both come from
    /// the same [`ArchiveListing`](elang_container::ArchiveListing) call).
    ///
    /// Returns `Err` if the layout and index disagree on entry count or per-entry
    /// frame offsets — both would mean the listing is internally inconsistent,
    /// which the container layer already rejects at stage 1, so this is a
    /// belt-and-braces guard rather than a normal failure mode.
    pub fn new(index: &'a Index, layout: &FrameLayout) -> Result<Self, &'static str> {
        if index.entries().len() != layout.len() {
            return Err("archive view: index and layout disagree on entry count");
        }
        let mut entries = Vec::with_capacity(layout.len());
        for (idx, frame_span) in layout.spans().iter().enumerate() {
            let idx_entry = index
                .entries()
                .get(idx)
                .ok_or("archive view: layout has more frames than index entries")?;
            if idx_entry.frame_offset() != frame_span.offset {
                return Err("archive view: index/frame-offset disagreement");
            }
            entries.push(ArcEntry {
                path: idx_entry.path().as_str().to_string(),
                span: Span {
                    offset: frame_span.offset,
                    compressed_size: frame_span.compressed_size,
                    uncompressed_size: idx_entry.uncompressed_size(),
                },
            });
        }
        Ok(Self {
            _index: index,
            entries,
        })
    }

    /// Number of indexed resources.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the archive is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a resource by its validated [`ResourcePath`].
    #[must_use]
    pub fn lookup(&self, path: &ResourcePath) -> Option<Span> {
        self.lookup_str(path.as_str())
    }

    /// Look up a resource by its raw UTF-8 path.
    #[must_use]
    pub fn lookup_str(&self, path: &str) -> Option<Span> {
        self.entries.iter().find(|e| e.path == path).map(|e| e.span)
    }

    /// Read the on-disk frame bytes for `span` from `archive`.
    ///
    /// Returns `None` if the slice would fall outside `archive` — that
    /// means the listing was inconsistent with the archive length, which
    /// the container layer already validated at stage 1 but a caller may
    /// still trip if the archive bytes were truncated after parsing.
    #[must_use]
    pub fn read_span(span: Span, archive: &[u8]) -> Option<&[u8]> {
        let offset = usize::try_from(span.offset).ok()?;
        let size = usize::try_from(span.compressed_size).ok()?;
        let end = offset.checked_add(size)?;
        if end > archive.len() {
            return None;
        }
        archive.get(offset..end)
    }

    /// All resource paths in index order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.path.as_str()).collect()
    }

    /// All (path, span) pairs in index order. Used by the resource-integrity
    /// stage to iterate every resource.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Span)> + '_ {
        self.entries.iter().map(|e| (e.path.as_str(), e.span))
    }
}

impl elang_epf::manifest::IndexLookup for ArchiveView<'_> {
    fn contains(&self, path: &str) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }
}
