//! # elang-container
//!
//! Binary `.elang` container: 32-byte header, per-resource Zstandard frames,
//! index and 64-byte terminal footer, plus the bounded streaming reader and the
//! canonical/`Fast`/`Compact` writer.
//!
//! Implemented across Phases 3 (header/index/footer), 4 (Zstd frames) and 12/13
//! (reader integration and writer). This crate must treat all container bytes as
//! untrusted: no panics, no out-of-bounds reads, checked offset arithmetic.
#![allow(clippy::needless_doctest_main)]

pub mod footer;
pub mod frame;
pub mod header;
pub mod index;
pub mod layout;

#[cfg(test)]
pub(crate) mod testdata;

pub use footer::{Footer, FOOTER_LEN, FOOTER_MAGIC, MAX_INDEX_SIZE, MIN_INDEX_SIZE};
pub use header::{Header, HEADER_LEN, HEADER_MAGIC, SUPPORTED_MAJOR, SUPPORTED_MINOR};
pub use index::{
    Index, IndexEntry, COMPRESSION_ZSTD, ENTRY_FIXED_LEN, ENTRY_FORMAT, INDEX_HEADER_LEN,
    INDEX_MAGIC, INDEX_VERSION,
};
pub use layout::{validate_layout, ArchiveListing, FrameLayout, FrameSpan, ListingEntry};
