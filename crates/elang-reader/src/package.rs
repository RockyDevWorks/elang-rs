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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "unit tests for stage tracker + error helpers; a panic signals a regression"
)]
mod tests {
    use super::*;
    use crate::package::__test_support::build_canonical_archive;
    use crate::Package;

    #[test]
    fn stage1_internal_err_attaches_stage_1() {
        let err = stage1_internal_err("simulated internal failure");
        assert_eq!(err.diagnostic.code, "READER_INTERNAL");
        assert_eq!(err.diagnostic.category, Category::Container);
        assert_eq!(err.diagnostic.stage, Some(1));
        assert!(
            err.diagnostic
                .message
                .contains("simulated internal failure"),
            "message should include the caller-supplied detail"
        );
    }

    #[test]
    fn stage_from_u8_maps_all_twelve_stages() {
        // Run the full pipeline; this exercises every match arm of
        // `stage_from_u8` via `advance_range(1, 12)`.
        let archive = build_canonical_archive();
        let pkg = Package::open(&archive.bytes).expect("stages 1-7");
        let _validated = pkg.validate().expect("stages 8-12");

        // Spot-check the helper directly for every value 1..=12.
        for n in 1u8..=12 {
            assert!(stage_from_u8(n).is_some(), "stage {n} must map to Some");
        }
        assert!(stage_from_u8(0).is_none());
        assert!(stage_from_u8(13).is_none());
    }

    #[test]
    fn advance_range_early_returns_on_invalid_stage() {
        // Drive `advance_range(0, 0)` — `stage_from_u8(0)` returns `None`,
        // so the function returns early without advancing the tracker.
        // The branch is exercised; we don't assert on the tracker state
        // because the StageTracker's initial state is opaque to us here.
        let mut tracker = StageTracker::new();
        advance_range(&mut tracker, 0, 0);
        // Drive `advance_range(13, 13)` — `stage_from_u8(13)` returns
        // `None`, same early-return branch.
        advance_range(&mut tracker, 13, 13);
    }
}

/// Crate-private test support — exposes a minimal in-memory canonical
/// archive so `#[cfg(test)]` modules in sibling files (bootstrap, stages,
/// archive) can exercise defensive code paths that require a real listing
/// but cannot depend on the integration-test fixture under `tests/`.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test fixture builder; a panic signals a fixture bug, not untrusted-input failure"
)]
#[doc(hidden)]
pub(crate) mod __test_support {
    use elang_container::footer::FOOTER_LEN;
    use elang_container::frame::{encode, Profile};
    use elang_container::header::{Header, HEADER_LEN};
    use elang_container::index::{ENTRY_FORMAT, INDEX_HEADER_LEN, INDEX_MAGIC, INDEX_VERSION};
    use sha2::{Digest, Sha256};

    /// A built archive's bytes.
    pub struct TestArchive {
        pub bytes: Vec<u8>,
    }

    /// Canonical SHA-256 / zstd-encoded payloads, kept self-consistent.
    const MIMETYPE: &[u8] = b"application/vnd.elang";
    const FORMAT_VERSION: &[u8] = b"0.1.0\n";
    const EPF: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Reader Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";
    const ELD: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

    fn build_manifest_bytes(resources: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut out = String::from("integrity {\n    algorithm \"sha-256\"\n");
        for (path, bytes) in resources {
            if path == "META-INF/integrity.kdl" {
                continue;
            }
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            let hex: String = digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join("");
            out.push_str(&format!("    resource \"{path}\" sha256=\"{hex}\"\n"));
        }
        out.push_str("}\n");
        out.into_bytes()
    }

    /// Build the smallest fully-consistent archive: 5 indexed resources,
    /// real Zstd frames, a self-consistent integrity manifest.
    pub fn build_canonical_archive() -> TestArchive {
        // Step 1: build with placeholder manifest so offsets are computed.
        let placeholder = b"integrity {\n}\n";
        let specs: Vec<(&'static str, &[u8])> = vec![
            ("Header/mimetype", MIMETYPE),
            ("Header/format.version", FORMAT_VERSION),
            ("META-INF/integrity.kdl", placeholder),
            ("package.epf", EPF),
            ("Linguistics/phonemes.eld", ELD),
        ];
        let mut sorted: Vec<(&'static str, &[u8])> = specs;
        sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        let mut frame_bytes = Vec::new();
        let mut decoded: Vec<(String, Vec<u8>)> = Vec::new();
        let mut entries: Vec<(String, u64, u64, u64)> = Vec::new();
        let mut cursor: u64 = u64::try_from(HEADER_LEN).expect("HEADER_LEN fits u64");
        for (path, uncompressed) in &sorted {
            let compressed = encode(uncompressed, Profile::Fast).expect("zstd encode");
            entries.push((
                (*path).to_string(),
                cursor,
                u64::try_from(compressed.len()).expect("compressed fits u64"),
                u64::try_from(uncompressed.len()).expect("uncompressed fits u64"),
            ));
            decoded.push(((*path).to_string(), uncompressed.to_vec()));
            cursor = cursor
                .checked_add(u64::try_from(compressed.len()).expect("size fits u64"))
                .expect("cursor overflow");
            frame_bytes.extend_from_slice(&compressed);
        }
        let _index_offset = cursor;

        let mut index_bytes = Vec::with_capacity(INDEX_HEADER_LEN);
        index_bytes.extend_from_slice(&INDEX_MAGIC);
        index_bytes.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        index_bytes.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        let count = u32::try_from(entries.len()).expect("resource count fits u32");
        index_bytes.extend_from_slice(&count.to_le_bytes());
        for (path, offset, compressed_size, uncompressed_size) in &entries {
            let path_bytes = path.as_bytes();
            let path_len = u16::try_from(path_bytes.len()).expect("path fits u16");
            index_bytes.extend_from_slice(&path_len.to_le_bytes());
            index_bytes.extend_from_slice(path_bytes);
            index_bytes.extend_from_slice(&offset.to_le_bytes());
            index_bytes.extend_from_slice(&compressed_size.to_le_bytes());
            index_bytes.extend_from_slice(&uncompressed_size.to_le_bytes());
            index_bytes.push(1); // compression = zstd
        }
        let _index_size = u64::try_from(index_bytes.len()).expect("index size fits u64");
        let _index_sha256: [u8; 32] = Sha256::digest(&index_bytes).into();

        // Step 2: compute the manifest from the *other* resources, then
        // rebuild the archive with the real manifest as `META-INF/integrity.kdl`.
        let manifest = build_manifest_bytes(&decoded);
        let mut sorted2: Vec<(String, Vec<u8>)> = sorted
            .into_iter()
            .map(|(p, b)| (p.to_string(), b.to_vec()))
            .collect();
        if let Some(slot) = sorted2
            .iter_mut()
            .find(|(p, _)| p == "META-INF/integrity.kdl")
        {
            slot.1 = manifest;
        }

        let mut frame_bytes2 = Vec::new();
        let mut entries2: Vec<(String, u64, u64, u64)> = Vec::new();
        let mut cursor2: u64 = u64::try_from(HEADER_LEN).expect("HEADER_LEN fits u64");
        for (path, uncompressed) in &sorted2 {
            let compressed = encode(uncompressed, Profile::Fast).expect("zstd encode");
            entries2.push((
                path.clone(),
                cursor2,
                u64::try_from(compressed.len()).expect("compressed fits u64"),
                u64::try_from(uncompressed.len()).expect("uncompressed fits u64"),
            ));
            cursor2 = cursor2
                .checked_add(u64::try_from(compressed.len()).expect("size fits u64"))
                .expect("cursor overflow");
            frame_bytes2.extend_from_slice(&compressed);
        }
        let index_offset2 = cursor2;

        let mut index_bytes2 = Vec::with_capacity(INDEX_HEADER_LEN);
        index_bytes2.extend_from_slice(&INDEX_MAGIC);
        index_bytes2.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        index_bytes2.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        let count2 = u32::try_from(entries2.len()).expect("resource count fits u32");
        index_bytes2.extend_from_slice(&count2.to_le_bytes());
        for (path, offset, compressed_size, uncompressed_size) in &entries2 {
            let path_bytes = path.as_bytes();
            let path_len = u16::try_from(path_bytes.len()).expect("path fits u16");
            index_bytes2.extend_from_slice(&path_len.to_le_bytes());
            index_bytes2.extend_from_slice(path_bytes);
            index_bytes2.extend_from_slice(&offset.to_le_bytes());
            index_bytes2.extend_from_slice(&compressed_size.to_le_bytes());
            index_bytes2.extend_from_slice(&uncompressed_size.to_le_bytes());
            index_bytes2.push(1);
        }
        let index_size2 = u64::try_from(index_bytes2.len()).expect("index size fits u64");
        let index_sha256_2: [u8; 32] = Sha256::digest(&index_bytes2).into();

        let mut footer = Vec::with_capacity(FOOTER_LEN);
        footer.extend_from_slice(b"ELEND001");
        footer.extend_from_slice(&index_offset2.to_le_bytes());
        footer.extend_from_slice(&index_size2.to_le_bytes());
        footer.extend_from_slice(&index_sha256_2);
        footer.extend_from_slice(&[0u8; 8]);

        let header = Header::current();
        let mut header_buf = vec![0u8; HEADER_LEN];
        header.encode(&mut header_buf, 0).expect("header encode");

        let mut archive = Vec::new();
        archive.extend_from_slice(&header_buf);
        archive.extend_from_slice(&frame_bytes2);
        archive.extend_from_slice(&index_bytes2);
        archive.extend_from_slice(&footer);

        TestArchive { bytes: archive }
    }
}
