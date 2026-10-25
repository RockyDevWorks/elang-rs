// Fuzz target for resource path validation (Phase 15, T1502).
//
// Drives arbitrary byte sequences through the same path-validation
// chain the container applies when an index entry is decoded:
//
//   1. UTF-8 decode (`std::str::from_utf8`).
//   2. NFC check (`elang_unicode::is_nfc`).
//   3. `ResourcePath::new` — enforces steering §3.4: non-empty,
//      non-absolute, ≤ 1024 UTF-8 bytes, no backslash, no NUL,
//      no `.` / `..` / empty segment.
//
// `ResourcePath` is documented as assuming the input is already
// NFC-normalised. Fuzzing the full chain catches defects introduced
// whenever one of the three stages misbehaves on adversarial input —
// especially NFC, which is the most expensive check on long inputs and
// the most likely place for an O(n²) regression or a panic.
//
// See `docs/security-testing.md` for budget and seed corpus policy.

#![no_main]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fuzz harness drives adversarial inputs through the path-validation chain; \
              panics are the failure signal that surfaces robustness defects in the \
              validator under test"
)]

use elang_core::ResourcePath;
use elang_unicode::is_nfc;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // The fuzzer feeds raw bytes. UTF-8 decode either succeeds (we run
    // the rest of the chain) or fails fast (a non-UTF-8 sequence is a
    // valid reject from the validator's perspective, so we let the
    // match-and-return shape discard it without counting as a bug).
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // NFC check is exercised even when the rest would reject — a panic
    // in `is_nfc` on a long malformed input is the kind of regression
    // this target is meant to catch.
    let _ = is_nfc(text);

    // ResourcePath::new enforces the S3.4 rules. We discard the result:
    // a structured error is the expected outcome for adversarial input.
    let _ = ResourcePath::new(text);
});
