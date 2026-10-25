//! Coverage-closure tests for `elang-kdl` span length bookkeeping.
//!
//! `SourceSpan::len` has two arms: a valid span reports the byte distance
//! between its endpoints, and an invalid (sentinel) span reports zero.
//! The invalid arm is only reachable through the `INVALID` constant, which
//! no parser path queries for a length.
//!
//! Test code path: `.expect()` / panics signal fixture regressions, not
//! untrusted-input failure. The workspace lint policy denies these on
//! production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use elang_kdl::span::SourceSpan;

/// The sentinel span reports a zero length and is flagged as invalid.
#[test]
fn an_invalid_span_has_zero_length() {
    assert!(SourceSpan::INVALID.is_invalid());
    assert_eq!(SourceSpan::INVALID.len(), 0);
    assert!(SourceSpan::INVALID.is_empty());
}

/// A real span still reports the byte distance between its endpoints, so
/// the zero above comes from the invalid arm and not from a degenerate
/// range.
#[test]
fn a_valid_span_reports_its_byte_distance() {
    let input = b"abc\ndef";
    let span = SourceSpan::from_range(input, 0, 3);
    assert!(!span.is_invalid());
    assert_eq!(span.len(), 3);
    assert!(!span.is_empty());
}
