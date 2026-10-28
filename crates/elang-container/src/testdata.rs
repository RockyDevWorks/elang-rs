//! Shared access to the committed golden fixtures.
//!
//! Every Phase 3 test reads its bytes from the committed fixture directory
#![allow(clippy::expect_used)]
//! rather than re-embedding byte arrays, so the tests assert against the
//! normative artifacts and a fixture change fails loudly everywhere at once.
//!
//! The fixture is `fixtures/valid/container/minimal-archive`, whose
//! `layout.json` is the normative frame map for this crate:
//!
//! | region  | offset | length |
//! |---------|--------|--------|
//! | header  | 0      | 32     |
//! | frames  | 32     | 240    |
//! | index   | 272    | 237    |
//! | footer  | 509    | 64     |
//!
//! Frame bodies in the fixture are labelled placeholders standing in for the
//! Phase 4 Zstd frames; the header/index/footer framing is fully normative.

/// Shared access to the committed golden fixtures. Every Phase 3
/// test reads its bytes from the committed fixture directory rather
/// than re-embedding byte arrays, so the tests assert against the
/// normative artifacts and a fixture change fails loudly everywhere
/// at once. Gated under `not(miri)` because every helper here uses
/// `std::fs::read`, which Miri cannot model (it would require
/// enabling the isolated FS emulation layer and pointing it at the
/// fixture directory); the production code paths these fixtures
/// exercise are already covered by the `not(miri)` gate on the
/// `frame::tests` module that round-trips the same byte streams
/// through libzstd.
#[cfg(not(miri))]
mod not_miri {
    use std::path::Path;

    /// Absolute path to the minimal-archive fixture directory.
    #[must_use]
    pub fn fixture_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/valid/container/minimal-archive")
    }

    /// The exact 237-byte reference index (`index.bin`).
    #[must_use]
    pub fn reference_index() -> Vec<u8> {
        std::fs::read(fixture_dir().join("index.bin")).expect("index.bin fixture present")
    }

    /// The exact 32-byte reference header (`header.bin`).
    #[must_use]
    pub fn reference_header() -> Vec<u8> {
        std::fs::read(fixture_dir().join("header.bin")).expect("header.bin fixture present")
    }

    /// The exact 64-byte reference footer (`footer.bin`).
    #[must_use]
    pub fn reference_footer() -> Vec<u8> {
        std::fs::read(fixture_dir().join("footer.bin")).expect("footer.bin fixture present")
    }

    /// The complete 573-byte reference archive (`archive.elang`).
    #[must_use]
    pub fn reference_archive() -> Vec<u8> {
        std::fs::read(fixture_dir().join("archive.elang")).expect("archive.elang fixture present")
    }
}

// Re-export so existing call sites (e.g. `crate::testdata::reference_header`)
// continue to work unchanged. The re-exports are empty under `miri`,
// so any caller that imports them is also unreachable under `miri` —
// every test that uses these helpers is gated under `not(miri)` to
// avoid a "function not found" error during Miri test discovery.
#[cfg(not(miri))]
pub use not_miri::*;

/// Normative layout constants from the fixture's `layout.json`.
/// `dead_code` is allowed because the constants are only read from
/// the `tests` modules, which are themselves gated under
/// `not(miri)`; under Miri nothing references these constants and
/// Rust would otherwise warn.
#[allow(dead_code)]
pub mod layout {
    /// Byte offset where the frame region starts.
    pub const FRAMES_OFFSET: u64 = 32;
    /// Byte offset where the index starts.
    pub const INDEX_OFFSET: u64 = 272;
    /// Index length in bytes.
    pub const INDEX_SIZE: u64 = 237;
    /// Byte offset where the footer starts.
    pub const FOOTER_OFFSET: u64 = 509;
    /// Length of the frame region.
    pub const FRAMES_LEN: u64 = 240;
    /// Number of resources.
    pub const RESOURCE_COUNT: usize = 5;
    /// Compressed size of every fixture frame (uniform placeholders).
    pub const FRAME_SIZE: u64 = 48;
}
