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
pub mod header;

pub use footer::{Footer, FOOTER_LEN, FOOTER_MAGIC, MAX_INDEX_SIZE, MIN_INDEX_SIZE};
pub use header::{Header, HEADER_LEN, HEADER_MAGIC, SUPPORTED_MAJOR, SUPPORTED_MINOR};
