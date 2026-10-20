//! Official Unicode 18.0.0 NFC conformance test.
//!
//! Runs the pinned `NormalizationTest.txt` (UCD 18.0.0) against this crate's
//! NFC implementation. The file has five columns per row:
//!
//! ```text
//! source ; NFC ; NFD ; NFKC ; NFKD ; # comment
//! ```
//!
//! Per UAX#15, for every row: `toNFC(source) == toNFC(NFC) == toNFC(NFD) ==
//! <column NFC>`. We check that `nfc(col)` for `col in {source, NFC, NFD}`
//! equals the NFC column, and that the NFC column is reported as already-NFC by
//! `is_nfc`. Characters named in the file's `@Part1` section are the exhaustive
//! single-character cases; all parts are exercised.
//!
//! This is the gate for declaring Unicode 18.0.0 NFC compliance (ADR 0007): if
//! it fails, ELANG does not claim NFC 18.0 conformance.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::Path;

use elang_unicode::{is_nfc, to_nfc as nfc};

/// Parse a space-separated list of hex code points into a String.
fn parse_cps(field: &str) -> String {
    field
        .split_whitespace()
        .map(|h| {
            let n = u32::from_str_radix(h, 16).expect("hex code point");
            char::from_u32(n).expect("valid scalar")
        })
        .collect()
}

#[test]
fn nfc_conformance_unicode_18_0_0() {
    // Pinned UCD file, relative to the workspace root (two levels up from the
    // crate manifest dir).
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest)
        .join("..")
        .join("..")
        .join("specs/sources/ucd-18.0.0/NormalizationTest.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    // Confirm we are testing the 18.0.0 file (fail closed on version drift).
    assert!(
        text.lines()
            .next()
            .map(|l| l.contains("18.0.0"))
            .unwrap_or(false),
        "NormalizationTest.txt is not the pinned 18.0.0 file"
    );

    let mut rows = 0usize;
    let mut checked = 0usize;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        let cols: Vec<&str> = line.split(';').collect();
        if cols.len() < 5 {
            continue;
        }
        rows += 1;
        let source = parse_cps(cols[0]);
        let col_nfc = parse_cps(cols[1]);
        let col_nfd = parse_cps(cols[2]);

        // toNFC(source) == NFC column
        assert_eq!(nfc(&source), col_nfc, "toNFC(source) mismatch: {line}");
        // toNFC(NFC) == NFC column (idempotence)
        assert_eq!(nfc(&col_nfc), col_nfc, "toNFC(NFC) mismatch: {line}");
        // toNFC(NFD) == NFC column
        assert_eq!(nfc(&col_nfd), col_nfc, "toNFC(NFD) mismatch: {line}");

        // The NFC column must be reported as already-NFC.
        assert!(is_nfc(&col_nfc), "is_nfc(NFC) false: {line}");
        checked += 1;
    }

    assert!(rows > 10_000, "suspiciously few rows parsed: {rows}");
    assert_eq!(rows, checked, "every row must be checked");
    eprintln!("NFC conformance: {checked} rows passed (Unicode 18.0.0)");
}
