//! Per-stage pipeline functions for the integrated reader.
//!
//! Each function runs one (or one cluster) of validation stages and returns
//! the data the next stage needs. The functions are pure with respect to the
//! [`crate::Package`] state — they take the archive bytes, the validated
//! [`ArchiveView`], and any required inputs as
//! arguments, and return the next stage's outputs. This keeps the per-stage
//! work independently testable.
//!
//! | Function | Stages | Output |
//! |---|---|---|
//! | [`run_integrity_structure`] | 9 | parsed [`elang_integrity::IntegrityManifest`] |
//! | [`run_epf`] | 10 | parsed [`EpfPackage`] |
//! | [`run_resource_integrity`] | 11 | decoded resources + per-resource errors |
//! | [`run_eld_semantics`] | 12 | validated [`elang_core::model::eld::PhonemeInventory`] |
//!
//! Bootstrap (stage 8) lives in [`crate::bootstrap`] because its error model
//! (CONTAINER category with stage 8 and a resource path) differs from the
//! stage-9+ errors that flow through the upstream crates' diagnostics.

use std::collections::BTreeSet;

use elang_container::frame::{decode, DecodeBudget};
use elang_core::error::{Category, Diagnostic, Error};
use elang_core::limits::{
    MAX_ELD_BYTES, MAX_EPF_BYTES, MAX_GENERIC_RESOURCE_BYTES, MAX_INTEGRITY_BYTES,
    MAX_UNCOMPRESSED_AGGREGATE_BYTES,
};
use elang_core::ResourcePath;
use elang_eld::inventory::from_document as eld_inventory_from_document;
use elang_eld::parse as eld_parse;
use elang_eld::when::parse_all as eld_parse_all;
use elang_epf::manifest::resolve_phonemes;
use elang_epf::model::EpfPackage;
use elang_integrity::{parse_document as integrity_parse_document, IntegrityManifest};
use elang_kdl::parse as kdl_parse;
use elang_kdl::ParseError;
use elang_repertoire::repertoire::load as repertoire_load;

use crate::archive::{ArchiveView, Span};
use crate::error::bootstrap_err;

/// Path of the integrity manifest resource.
pub const INTEGRITY_PATH: &str = "META-INF/integrity.kdl";
/// Path of the `package.epf` resource.
pub const EPF_PATH: &str = "package.epf";

/// Run stage 9 — parse `META-INF/integrity.kdl`.
///
/// # Errors
///
/// * `INTEGRITY_STRUCTURE_INVALID` (or any stage-9 integrity code) when the
///   KDL document fails parsing.
/// * `INTEGRITY_MISSING_ENTRY` / `INTEGRITY_EXTRA_ENTRY` via
///   [`elang_integrity::verify_coverage`] when the manifest's entries
///   disagree with the resource index (T1206).
pub fn run_integrity_structure(
    view: &ArchiveView<'_>,
    archive: &[u8],
) -> Result<IntegrityManifest, Error> {
    let bytes = decode_indexed_resource(view, archive, INTEGRITY_PATH, MAX_INTEGRITY_BYTES, 9)?;
    let doc = kdl_parse(&bytes).map_err(integrity_kdl_to_err)?;
    let manifest = integrity_parse_document(&doc)?;
    let mut indexed_paths: BTreeSet<ResourcePath> = BTreeSet::new();
    for (path, _) in view.iter() {
        let rp = ResourcePath::new(path).map_err(|e| {
            // Index paths were already validated by the container layer at
            // stage 1; reaching here means the container returned a path the
            // core layer refuses, which is a programmer error.
            Error {
                diagnostic: Diagnostic::new(
                    "INDEX_PATH_INVALID",
                    Category::Index,
                    format!("index path {path:?} is not a valid ResourcePath: {e}"),
                )
                .with_stage(9),
            }
        })?;
        indexed_paths.insert(rp);
    }
    let self_path = ResourcePath::new(INTEGRITY_PATH).map_err(|e| Error {
        diagnostic: Diagnostic::new(
            "INDEX_PATH_INVALID",
            Category::Index,
            format!("integrity path {INTEGRITY_PATH:?} is not a valid ResourcePath: {e}"),
        )
        .with_stage(9),
    })?;
    elang_integrity::verify_coverage(&manifest, &indexed_paths, &self_path)
        .map_err(integrity_coverage_err)?;
    Ok(manifest)
}

/// Run stage 10 — parse `package.epf` and resolve its phoneme-path manifest
/// against the resource index (T1205).
///
/// # Errors
///
/// Any stage-10 `EPF_SCHEMA` / `UNICODE` / `LIMIT_*` diagnostic from
/// `elang_epf::parse`. Resolution failures surface as the upstream
/// `EPF_PHONEMES_NOT_INDEXED` (or its renamed equivalent) emitted from
/// `resolve_phonemes`.
pub fn run_epf(view: &ArchiveView<'_>, archive: &[u8]) -> Result<EpfPackage, Error> {
    let bytes = decode_indexed_resource(view, archive, EPF_PATH, MAX_EPF_BYTES, 10)?;
    let pkg = elang_epf::parse(&bytes)?;
    resolve_phonemes(pkg.manifest().phonemes().as_str(), view)
        .map_err(|e| phoneme_resolution_err(e, pkg.manifest().phonemes().as_str()))?;
    Ok(pkg)
}

/// Run stage 11 — decode every indexed resource and verify its SHA-256
/// against the manifest.
///
/// Self-exclusion: the integrity manifest itself (`META-INF/integrity.kdl`)
/// is decoded (so its bytes are available) but its hash is **not** verified
/// against an entry in the manifest (the manifest cannot list itself; see
/// T1206 and `IntegrityManifest::verify_coverage` which enforces this).
///
/// # Errors
///
/// Returns the first `INTEGRITY_HASH_MISMATCH` (stage 11) encountered while
/// iterating the index in order; the `resource_path` diagnostic field names
/// the offending resource.
pub fn run_resource_integrity(
    view: &ArchiveView<'_>,
    archive: &[u8],
    manifest: &IntegrityManifest,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut budget = DecodeBudget::new(MAX_GENERIC_RESOURCE_BYTES);
    budget.max_aggregate_bytes = MAX_UNCOMPRESSED_AGGREGATE_BYTES;

    let mut decoded = Vec::with_capacity(view.len());
    for (path, span) in view.iter() {
        let bytes = decode_one(archive, path, span, &mut budget)?;
        if path != INTEGRITY_PATH {
            let entry = manifest
                .resources
                .values()
                .find(|e| e.path.as_str() == path)
                .ok_or_else(|| missing_entry_err(path))?;
            let mut hasher = elang_integrity::Hasher::new();
            hasher.update(&bytes);
            let computed = hasher.finalize();
            elang_integrity::verify_one(entry, &computed)
                .map_err(|e| enrich_with_resource(e, path))?;
        }
        decoded.push((path.to_string(), bytes));
    }
    Ok(decoded)
}

/// Run stage 12 — full ELD structural + semantic validation.
///
/// Loads the pinned IPA repertoire, parses the declared phoneme `.eld`,
/// parses every `when` block (Phase 10 reference resolution), and builds the
/// validated [`elang_core::model::eld::PhonemeInventory`].
///
/// # Errors
///
/// * Repertoire load failure (`REPERTOIRE_LOAD_FAILED`) — propagated as
///   a stage-12 diagnostic. In practice this only fires if the pinned
///   `specs/schema/ipa-v1.json` shipped with the crate is missing.
/// * Any stage-12 `ELD_SCHEMA` / `IPA` / `ROMAN` / `WHEN` diagnostic from
///   `elang-eld`.
pub fn run_eld_semantics(
    view: &ArchiveView<'_>,
    archive: &[u8],
    epf: &EpfPackage,
) -> Result<elang_core::model::eld::PhonemeInventory, Error> {
    let phonemes_path = epf.manifest().phonemes().as_str();
    let bytes = decode_indexed_resource(view, archive, phonemes_path, MAX_ELD_BYTES, 12)?;
    let repertoire = repertoire_load().map_err(repertoire_load_err)?;
    let doc = eld_parse(&bytes, &repertoire)?;
    eld_parse_all(&doc, &repertoire)?;
    let inventory = eld_inventory_from_document(&doc, &repertoire)?;
    Ok(inventory)
}

/// Decode a single indexed resource under a per-resource byte ceiling and a
/// shared aggregate budget.
///
/// The `stage` is attached to every diagnostic so the failure names the
/// correct stage even when the underlying cause is a frame-level error.
fn decode_indexed_resource(
    view: &ArchiveView<'_>,
    archive: &[u8],
    path: &str,
    max_resource: u64,
    stage: u8,
) -> Result<Vec<u8>, Error> {
    let span = view.lookup_str(path).ok_or_else(|| {
        bootstrap_err(
            "RESOURCE_MISSING",
            format!("required resource {path:?} is not in the archive index"),
            path,
        )
    })?;
    let frame = ArchiveView::read_span(span, archive).ok_or_else(|| {
        bootstrap_err(
            "FRAME_TRUNCATED",
            format!("resource {path:?} frame falls outside the archive"),
            path,
        )
    })?;
    let mut budget = DecodeBudget::new(max_resource);
    decode(frame, &mut budget).map_err(|e| enrich_with_resource_and_stage(e, path, stage))
}

/// Decode one resource during the resource-integrity pass.
fn decode_one(
    archive: &[u8],
    path: &str,
    span: Span,
    budget: &mut DecodeBudget,
) -> Result<Vec<u8>, Error> {
    let frame = ArchiveView::read_span(span, archive).ok_or_else(|| {
        bootstrap_err(
            "FRAME_TRUNCATED",
            format!("resource {path:?} frame falls outside the archive"),
            path,
        )
    })?;
    decode(frame, budget).map_err(|e| enrich_with_resource_and_stage(e, path, 11))
}

/// `RESOURCE_PATH` diagnostic for an integrity-manifest entry that is
/// missing for an indexed resource.
fn missing_entry_err(path: &str) -> Error {
    Error {
        diagnostic: Diagnostic::new(
            "INTEGRITY_MISSING_ENTRY",
            Category::Integrity,
            format!("indexed resource {path:?} has no integrity-manifest entry"),
        )
        .with_stage(11)
        .with_resource(path),
    }
}

/// Adapter: re-tag a [`elang_integrity::verify_coverage`] error with a
/// stage-9 marker (the underlying code is already correct).
fn integrity_coverage_err(e: Error) -> Error {
    Error {
        diagnostic: e.diagnostic.with_stage(9),
    }
}

/// Adapter: convert a KDL parser error into an `INTEGRITY`-category,
/// stage-9 diagnostic.
fn integrity_kdl_to_err(e: ParseError) -> Error {
    let mut d = Diagnostic::new(
        "INTEGRITY_STRUCTURE_INVALID",
        Category::Integrity,
        format!("META-INF/integrity.kdl failed to parse: {e}"),
    )
    .with_stage(9);
    let span = e.span();
    if !span.is_invalid() {
        d = d.with_span(elang_core::error::KdlSpan {
            line: span.start.line,
            column: span.start.column,
            length: span.len(),
        });
    }
    Error { diagnostic: d }
}

/// Adapter: tag a manifest-resolution error from `resolve_phonemes` with
/// the declared phoneme path.
fn phoneme_resolution_err(e: Error, declared: &str) -> Error {
    Error {
        diagnostic: e.diagnostic.with_resource(declared),
    }
}

/// Adapter: convert a `RepertoireError` into a stage-12 diagnostic.
fn repertoire_load_err(e: elang_repertoire::repertoire::RepertoireError) -> Error {
    Error {
        diagnostic: Diagnostic::new(
            "REPERTOIRE_LOAD_FAILED",
            Category::Ipa,
            format!("failed to load the pinned IPA repertoire: {e:?}"),
        )
        .with_stage(12),
    }
}

/// Re-tag a [`crate::error::bootstrap_err`]'s diagnostic with the given
/// resource path (and leave the stage unchanged).
fn enrich_with_resource(e: Error, path: &str) -> Error {
    Error {
        diagnostic: e.diagnostic.with_resource(path),
    }
}

/// Re-tag an upstream error with a `resource_path` and an explicit stage.
fn enrich_with_resource_and_stage(e: Error, path: &str, stage: u8) -> Error {
    Error {
        diagnostic: e.diagnostic.with_resource(path).with_stage(stage),
    }
}

/// Re-export the [`elang_integrity::IntegrityEntry`] type used by the verify-pipeline outputs.
#[allow(dead_code, reason = "API stability for downstream callers")]
pub type IntegrityEntryRef = elang_integrity::IntegrityEntry;
