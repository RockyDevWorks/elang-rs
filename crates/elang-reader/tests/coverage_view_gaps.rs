//! Coverage-closure tests for the `ArchiveView` constructor.
//!
//! `ArchiveView::new` pairs a parsed `Index` with a validated `FrameLayout`.
//! In the normal pipeline both come from the same
//! `elang_container::ArchiveListing::parse` call, so they always agree — the
//! constructor's two consistency guards are belt-and-braces. They are
//! nevertheless reachable from the public API, because an `Index` and a
//! `FrameLayout` are two independent values: a caller can pair an index with
//! a layout derived from a *different* index.
//!
//! Test code path: `.expect()` / `.expect_err()` / panics signal fixture
//! regressions, not untrusted-input failure. The workspace lint policy
//! denies these on production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use elang_container::{validate_layout, Index, IndexEntry, COMPRESSION_ZSTD};
use elang_reader::ArchiveView;

/// One index entry: an 8-byte compressed frame at `offset`.
fn entry(path: &str, offset: u64) -> IndexEntry {
    IndexEntry::new(path, offset, 8, 8, COMPRESSION_ZSTD).expect("index entry builds")
}

/// An index holding exactly one frame.
fn one_entry_index(path: &str, offset: u64) -> Index {
    Index::with_entries(vec![entry(path, offset)]).expect("index builds")
}

/// The frame layout of a two-frame index whose frames occupy bytes 32..48.
fn two_frame_layout() -> (Index, elang_container::FrameLayout) {
    let index =
        Index::with_entries(vec![entry("a.bin", 32), entry("b.bin", 40)]).expect("index builds");
    let layout = validate_layout(&index, 48).expect("layout validates");
    (index, layout)
}

/// The index and the layout disagree on how many resources there are.
#[test]
fn archive_view_rejects_an_index_and_layout_with_different_entry_counts() {
    let index = one_entry_index("a.bin", 32);
    let (_other, layout) = two_frame_layout();

    let err = ArchiveView::new(&index, &layout).expect_err("entry counts disagree");
    assert_eq!(
        err,
        "archive view: index and layout disagree on entry count"
    );
}

/// The index and the layout agree on the entry count but place the frame at
/// different offsets.
#[test]
fn archive_view_rejects_an_index_whose_frame_offset_disagrees_with_the_layout() {
    let index = one_entry_index("a.bin", 64);
    let other = one_entry_index("a.bin", 32);
    let layout = validate_layout(&other, 40).expect("layout validates");

    let err = ArchiveView::new(&index, &layout).expect_err("frame offsets disagree");
    assert_eq!(err, "archive view: index/frame-offset disagreement");
}
