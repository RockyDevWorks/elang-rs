// Fuzz target for the Zstd frame-header inspector and bounded streaming
// decoder (Phase 15, T1503).
//
// Drives arbitrary byte sequences through the two container-level frame
// entry points:
//
// - `frame::inspect` — parses the header only, returning a validated
//   `FrameHeader` (or a structured `ContainerError`). Performs no
//   allocation past the header, so it is cheap to call on every input.
// - `frame::decode`  — inspects, then streams the body through libzstd
//   against a `DecodeBudget` capped at the per-resource ceiling
//   (`MAX_GENERIC_RESOURCE_BYTES`). The per-resource and aggregate
//   budgets are what stop a decompression bomb: a frame declaring 1
//   KiB that expands to 1 GiB is killed at the cap rather than after
//   allocating its expansion.
//
// The contract under fuzz is the same as the contract under attack:
// `inspect` must not panic, must not read past `data`, must report a
// structured `ContainerError` for every rejectable header condition,
// and `decode` must additionally enforce the per-resource and
// aggregate budgets before allocation grows past them.
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
    reason = "fuzz harness drives adversarial inputs through the frame decoder; \
              panics are the failure signal that surfaces robustness defects in the \
              header parser and bounded decoder under test"
)]

use elang_container::frame::{self, DecodeBudget};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // Header inspection is allocation-cheap and total; a panic here
    // would be a real defect (out-of-bounds read, integer overflow,
    // unexpected allocation). Errors are expected and discarded.
    let _ = frame::inspect(data);

    // Bounded decode: charge against a fresh per-call budget so a
    // fuzz input cannot exhaust the aggregate of every other input.
    // The per-resource ceiling is the generic 1 GiB; the fuzzer
    // short-circuits on budget errors long before that matters.
    let mut budget = DecodeBudget::generic();
    let _ = frame::decode(data, &mut budget);
});
