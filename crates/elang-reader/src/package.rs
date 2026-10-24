//! The [`Package`] entry point — the integrated 12-stage reader pipeline.
//!
//! [`Package::open`] runs stages 1–7 (cheap, no decompression),
//! [`Package::inspect`] hands back an [`Inspected`] view, [`Package::verify`]
//! runs stages 8–11 and hands back a [`Verified`] view, and
//! [`Package::validate`] adds stage 12 and hands back a [`Validated`] view.
//!
//! # Stage ownership
//!
//! ```text
//!   open(archive)                       stages 1–7  →
//!   inspect(&self)                      Inspected   ← ceiling 7
//!   verify(self) → 8–11                 Verified    ← ceiling 11
//!   validate(self) → 12                 Validated   ← ceiling 12
//! ```
//!
//! `verify` and `validate` consume `self` because they advance the
//! [`StageTracker`] and store intermediate state on the [`Package`]. Callers
//! that only want the listing should call `inspect` (which borrows) and drop
//! the [`Package`] afterwards.
//!
//! # Diagnostics
//!
//! Every error carries a stable registry code, the validation stage where it
//! fired, and (where applicable) the resource path. The three-level "what does
//! each result type actually guarantee?" ladder is enforced by the type
//! system: [`Inspected`] ⇒ stages 1–7 done, [`Verified`] ⇒ 1–11 done (full
//! integrity), [`Validated`] ⇒ 1–12 done (full semantic). There is no public
//! constructor for any of the three result types, so a `Verified` cannot be
//! obtained without actually running stages 8–11, etc.

use elang_container::ArchiveListing;
use elang_core::error::{Category, Diagnostic, Error};
use elang_core::{Stage, StageTracker};

use crate::archive::ArchiveView;
use crate::bootstrap;
use crate::inspected::Inspected;
use crate::stages;
use crate::validated::Validated;
use crate::verified::Verified;

/// The integrated ELANG v0.1.0 package reader.
///
/// Construct via [`Package::open`], then call one of the three read methods
/// to advance the pipeline. See the [module-level docs](self) for the
/// stage-ownership diagram.
#[derive(Debug)]
pub struct Package {
    archive: Vec<u8>,
    listing: ArchiveListing,
    tracker: StageTracker,
}

impl Package {
    /// Parse an archive and run stages 1–7 (T1201).
    ///
    /// Stages 1–7 validate the binary header, terminal footer, index hash,
    /// sorted unique paths, per-resource size ceilings, the contiguous frame
    /// layout and the per-frame header inspection. No frame body is
    /// decompressed at this point. Opening also copies the entire archive
    /// into owned storage, costing `O(archive size)` time and memory in
    /// addition to parsing and retaining the index and frame metadata.
    ///
    /// # Errors
    ///
    /// Any stage-1–7 diagnostic from the container layer (`CONTAINER` /
    /// `INDEX` / `FRAME` / `LIMIT` category, stages 1–7). The error carries
    /// the stage number on its diagnostic.
    pub fn open(archive: &[u8]) -> Result<Self, Error> {
        let listing = ArchiveListing::parse(archive).map_err(|mut e| {
            if e.diagnostic.code == "HEADER_BAD_MAGIC" {
                e.diagnostic = e.diagnostic.with_stage(1);
            }
            e
        })?;
        let mut tracker = StageTracker::new();
        // Stages 1–7 are advanced in order. `ArchiveListing::parse` runs
        // them in the right order; the tracker records that fact for the
        // public SDK so callers see a single, consistent story.
        advance_range(&mut tracker, 1, 7);
        Ok(Self {
            archive: archive.to_vec(),
            listing,
            tracker,
        })
    }

    /// Hand back an [`Inspected`] view over the listing.
    ///
    /// Does not advance the pipeline further — `inspect` is by definition
    /// stages 1–7 only (REQ-READER-003). Borrows `&self` so the [`Package`]
    /// can be reused for [`Self::verify`] or [`Self::validate`] afterwards.
    #[must_use]
    pub fn inspect(&self) -> Inspected {
        Inspected::new(self.listing.clone())
    }

    /// Run stages 8–11 and hand back a [`Verified`] view (T1202).
    ///
    /// Consumes `self` because the verify pass decodes every indexed resource
    /// and stores them on the [`Verified`] view; the [`Package`] state is
    /// released once that view is built.
    ///
    /// # Errors
    ///
    /// * Stage 8 — `BOOTSTRAP_*` (CONTAINER category) when the
    ///   `Header/format.version` or `Header/mimetype` resource is missing
    ///   or carries wrong bytes (T1204).
    /// * Stage 9 — `INTEGRITY_STRUCTURE_INVALID` / `_MISSING_ENTRY` /
    ///   `_EXTRA_ENTRY` when the manifest is malformed or does not cover
    ///   every indexed resource (T1206).
    /// * Stage 10 — `EPF_SCHEMA` / `EPF_PHONEMES_NOT_INDEXED` /
    ///   `PATH_NOT_ELD` when `package.epf` is malformed or its declared
    ///   phoneme resource is missing from the index (T1205).
    /// * Stage 11 — `INTEGRITY_HASH_MISMATCH` when any resource's SHA-256
    ///   disagrees with the manifest entry.
    pub fn verify(mut self) -> Result<Verified, Error> {
        let view = build_view(&self.listing)?;
        let header_version = format!(
            "{}.{}",
            self.listing.header().major(),
            self.listing.header().minor()
        );
        // Stage 8 — bootstrap. Stores nothing on Package; the bootstrap
        // result is only used to enforce the cross-check.
        bootstrap::run(&view, &self.archive, &header_version)?;
        // Stage 9 — integrity structure.
        let manifest = stages::run_integrity_structure(&view, &self.archive)?;
        // Stage 10 — EPF + phoneme-path resolution.
        let epf = stages::run_epf(&view, &self.archive)?;
        // Stage 11 — decode + SHA-256 verify.
        let decoded = stages::run_resource_integrity(&view, &self.archive, &manifest)?;
        // Advance the tracker from 7 to 11 in order.
        advance_range(&mut self.tracker, 8, 11);

        Ok(Verified::new(self.listing, manifest, epf, decoded))
    }

    /// Run stages 8–12 and hand back a [`Validated`] view (T1203).
    ///
    /// Consumes `self`. Runs the entire [`Self::verify`] pipeline first, then
    /// adds stage 12 (full ELD semantic validation) which builds the
    /// canonical `PhonemeInventory`.
    ///
    /// # Errors
    ///
    /// Every error documented on [`Self::verify`], plus:
    ///
    /// * Stage 12 — `ELD_SCHEMA` / `IPA` / `ROMAN` / `WHEN` when the phoneme
    ///   `.eld` is malformed, references unknown phonemes, or violates
    ///   romanization uniqueness.
    /// * `REPERTOIRE_LOAD_FAILED` if the pinned IPA repertoire cannot be
    ///   loaded from the workspace at compile time.
    pub fn validate(mut self) -> Result<Validated, Error> {
        let view = build_view(&self.listing)?;
        let header_version = format!(
            "{}.{}",
            self.listing.header().major(),
            self.listing.header().minor()
        );

        bootstrap::run(&view, &self.archive, &header_version)?;
        let manifest = stages::run_integrity_structure(&view, &self.archive)?;
        let epf = stages::run_epf(&view, &self.archive)?;
        stages::run_resource_integrity(&view, &self.archive, &manifest)?;
        let inventory = stages::run_eld_semantics(&view, &self.archive, &epf)?;
        advance_range(&mut self.tracker, 8, 12);

        Ok(Validated::new(self.listing, manifest, epf, inventory))
    }

    /// Borrow the raw archive bytes that were passed to [`Package::open`].
    /// Used by [`crate::Package::verify`]'s resource integrity stage; also
    /// useful for callers that need to inspect the original binary alongside
    /// the validated views.
    #[must_use]
    pub fn archive(&self) -> &[u8] {
        &self.archive
    }

    /// Borrow the underlying [`ArchiveListing`] (stages 1–7 view).
    #[must_use]
    pub fn listing(&self) -> &ArchiveListing {
        &self.listing
    }
}

/// Advance the tracker through a contiguous run of stages, both endpoints
/// inclusive. `lo..=hi` must lie within 1..=12.
fn advance_range(tracker: &mut StageTracker, lo: u8, hi: u8) {
    for n in lo..=hi {
        let stage = match stage_from_u8(n) {
            Some(s) => s,
            None => return,
        };
        // `advance` only returns `None` on a protocol violation; we just
        // ran stages 1..=hi in order so the call cannot fail.
        if tracker.advance(stage).is_none() {
            return;
        }
    }
}

/// Build an [`ArchiveView`] borrowing from a [`ArchiveListing`]. The lifetime
/// is tied to the listing, so the view is dropped when the listing is
/// consumed or dropped.
fn build_view<'a>(listing: &'a ArchiveListing) -> Result<ArchiveView<'a>, Error> {
    ArchiveView::new(listing.index(), listing.layout()).map_err(stage1_internal_err)
}

/// Map a `u8` (1..=12) to the corresponding [`Stage`].
fn stage_from_u8(n: u8) -> Option<Stage> {
    use Stage::*;
    match n {
        1 => Some(Preflight),
        2 => Some(Header),
        3 => Some(Footer),
        4 => Some(IndexHash),
        5 => Some(Index),
        6 => Some(FrameLayout),
        7 => Some(FrameInspect),
        8 => Some(Bootstrap),
        9 => Some(IntegrityStructure),
        10 => Some(Epf),
        11 => Some(ResourceIntegrity),
        12 => Some(EldSemantics),
        _ => None,
    }
}

fn stage1_internal_err(msg: &str) -> Error {
    Error {
        diagnostic: Diagnostic::new(
            "READER_INTERNAL",
            Category::Container,
            format!("internal: {msg}"),
        )
        .with_stage(1),
    }
}
