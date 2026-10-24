//! Stage 8 — Bootstrap resource cross-checks (T1204).
//!
//! The binary header carries a `format.version` (e.g. `"0.1.0"`) but the
//! archive also stores it under the indexed resource `Header/format.version`.
//! Likewise, the locked mimetype `application/vnd.elang` is mirrored under
//! `Header/mimetype`. The bootstrap stage enforces that the two views agree
//! byte-for-byte; if either resource is missing or carries the wrong bytes,
//! the reader refuses to advance past stage 8 with the matching diagnostic.
//!
//! These resources are small (≤ 64 bytes each, per
//! [`MAX_BOOTSTRAP_RESOURCE_BYTES`](elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES)),
//! so the stage uses a tight per-resource decode budget and a fresh
//! [`DecodeBudget`] that does **not** share the aggregate counter with the
//! later resource-integrity pass (stage 11 owns the aggregate).

use elang_container::frame::{decode, DecodeBudget};
use elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES;
use elang_core::ResourcePath;

use crate::archive::{ArchiveView, Span};
use crate::error::{bootstrap_err, bootstrap_err_no_path};

/// Path of the in-archive `format.version` resource.
pub const FORMAT_VERSION_PATH: &str = "Header/format.version";
/// Path of the in-archive mimetype resource.
pub const MIMETYPE_PATH: &str = "Header/mimetype";
/// The only mimetype an ELANG v0.1.0 archive may carry (REQ-PKG-003).
pub const EXPECTED_MIMETYPE: &[u8] = b"application/vnd.elang";

/// Run stage 8 and return the decompressed `format.version` and `mimetype`
/// bytes for downstream callers (the values are small enough to keep).
///
/// # Errors
///
/// Returns a stage-8 `CONTAINER`-category diagnostic with the resource path
/// attached when:
/// * either resource is missing from the index (`RESOURCE_MISSING`),
/// * decompressing either resource fails or exceeds the per-resource budget
///   (`FRAME_TRUNCATED`),
/// * `format.version` is not valid UTF-8 or its major.minor prefix does
///   not match the binary header's `major.minor` (`FORMAT_VERSION_INVALID`),
/// * `mimetype` does not equal [`EXPECTED_MIMETYPE`] exactly
///   (`MIMETYPE_INVALID`).
pub fn run(
    view: &ArchiveView<'_>,
    archive: &[u8],
    header_format_version: &str,
) -> Result<(String, String), elang_core::Error> {
    let format_version = read_bootstrap_resource(view, archive, FORMAT_VERSION_PATH)?;
    let mimetype = read_bootstrap_resource(view, archive, MIMETYPE_PATH)?;

    let format_str = core::str::from_utf8(&format_version).map_err(|_| {
        bootstrap_err(
            "FORMAT_VERSION_INVALID",
            "Header/format.version is not valid UTF-8",
            FORMAT_VERSION_PATH,
        )
    })?;
    // The binary header encodes major.minor (e.g. "0.1"); the in-archive
    // `Header/format.version` is a semver-shaped string whose
    // major.minor prefix must match. This catches writer bugs that emit a
    // resource claiming a different format than the binary container.
    let expected_prefix = format!("{header_format_version}.");
    let stripped = format_str.trim_end_matches('\n').trim_end_matches('\r');
    if !stripped.starts_with(&expected_prefix) {
        return Err(bootstrap_err(
            "FORMAT_VERSION_INVALID",
            format!(
                "Header/format.version content {stripped:?} does not start with binary header major.minor prefix {expected_prefix:?}",
            ),
            FORMAT_VERSION_PATH,
        ));
    }
    // The patch component must be a non-empty ASCII integer; full semver
    // parsing belongs to the integrity pass, this stage only enforces the
    // shape. `split_at` is bounds-checked: `expected_prefix.len() <=
    // stripped.len()` because the prefix-match check above already proved
    // the prefix is present.
    let (_, patch) = stripped.split_at(expected_prefix.len());
    if patch.is_empty() || !patch.chars().all(|c| c.is_ascii_digit()) {
        return Err(bootstrap_err(
            "FORMAT_VERSION_INVALID",
            format!(
                "Header/format.version patch component {patch:?} is not a non-empty ASCII integer",
            ),
            FORMAT_VERSION_PATH,
        ));
    }

    if mimetype.as_slice() != EXPECTED_MIMETYPE {
        return Err(bootstrap_err(
            "MIMETYPE_INVALID",
            format!(
                "Header/mimetype content {:?} does not match expected mimetype {:?}",
                String::from_utf8_lossy(&mimetype),
                core::str::from_utf8(EXPECTED_MIMETYPE).unwrap_or("<binary>"),
            ),
            MIMETYPE_PATH,
        ));
    }

    // The mimetype bytes are validated ASCII (`application/vnd.elang`), so
    // the conversion is infallible; fall back to a documented sentinel
    // string if the constant ever changes shape.
    let mimetype_string = core::str::from_utf8(EXPECTED_MIMETYPE)
        .unwrap_or("application/vnd.elang")
        .to_string();
    Ok((stripped.to_string(), mimetype_string))
}

/// Decompress one bootstrap resource.
///
/// The per-resource budget is the standard bootstrap ceiling; no aggregate
/// counter is shared with later stages.
fn read_bootstrap_resource(
    view: &ArchiveView<'_>,
    archive: &[u8],
    path: &str,
) -> Result<Vec<u8>, elang_core::Error> {
    let span = view.lookup_str(path).ok_or_else(|| {
        bootstrap_err_no_path(
            "RESOURCE_MISSING",
            format!("archive index does not contain required bootstrap resource {path:?}"),
        )
    })?;
    decode_span(span, archive).ok_or_else(|| {
        bootstrap_err(
            "FRAME_TRUNCATED",
            format!("bootstrap resource {path:?} frame falls outside the archive"),
            path,
        )
    })
}

/// Decode a single resource's frame, enforcing the bootstrap per-resource
/// ceiling.
fn decode_span(span: Span, archive: &[u8]) -> Option<Vec<u8>> {
    let bytes = ArchiveView::read_span(span, archive)?;
    let mut budget = DecodeBudget::new(MAX_BOOTSTRAP_RESOURCE_BYTES);
    decode(bytes, &mut budget).ok()
}

/// Validate a `ResourcePath` is one of the locked bootstrap paths.
#[allow(
    dead_code,
    reason = "API helper for downstream callers that build their own bootstrap checks"
)]
pub(crate) fn is_bootstrap_path(path: &ResourcePath) -> bool {
    matches!(path.as_str(), FORMAT_VERSION_PATH | MIMETYPE_PATH)
}
