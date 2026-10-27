//! Service memory admission checks, performed before any decompression.

use elang_container::frame;
use elang_reader::{ArchiveView, Package};

use crate::error::{internal, ServerError};

/// Maximum total decoded resource bytes per SDK job (8 MiB).
const MAX_DECODED_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum decoder history window per SDK job (8 MiB).
const MAX_WINDOW_BYTES: u64 = 8 * 1024 * 1024;

/// Open a package and admit it for decoding only if every frame fits the
/// service's memory limits. Inspect actual frame headers: index sizes alone
/// are insufficient because the reader cross-checks them after decoding.
pub(crate) fn open_for_decode(archive: &[u8]) -> Result<Package, ServerError> {
    let pkg = Package::open(archive).map_err(ServerError::from)?;
    let view = ArchiveView::new(pkg.listing().index(), pkg.listing().layout()).map_err(internal)?;
    let mut total = 0u64;
    for (_path, span) in view.iter() {
        let bytes = ArchiveView::read_span(span, archive)
            .ok_or_else(|| internal("opened frame falls outside the archive"))?;
        let header = frame::inspect(bytes).map_err(ServerError::from)?;
        total = total
            .checked_add(header.content_size)
            .ok_or(ServerError::PayloadTooLarge)?;
        if total > MAX_DECODED_BYTES || header.window_size > MAX_WINDOW_BYTES {
            return Err(ServerError::PayloadTooLarge);
        }
    }
    Ok(pkg)
}
