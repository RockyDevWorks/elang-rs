// Fuzz target for the binary container header/footer/index decoders
// (Phase 15, T1501).
//
// Drives arbitrary byte sequences through the three container-level
// decoders that consume the outermost bytes of a `.elang` archive:
//
// - `Header::decode` — the 32-byte fixed header.
// - `Footer::decode` — the 64-byte terminal footer, located at the end
//   of the input slice.
// - `Index::decode`  — the variable-length resource index that sits
//   between the frame area and the footer.
//
// The contract under fuzz is the same as the contract under attack:
// none of these decoders may panic, may read past the end of the input
// slice, may allocate without a documented bound, or may report a
// success on a malformed buffer. They must return a structured
// `ContainerError` for every rejectable condition.
//
// Inputs to this target are arbitrary `&[u8]` from libFuzzer. The target
// does not need to drive every decoder with the same slice — each
// decoder has its own shape and the fuzzer will discover whichever
// mutations reach each call site independently.
//
// See `docs/security-testing.md` for budget, CI integration and seed
// corpus policy.

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
    reason = "fuzz harness drives adversarial inputs through the container decoders; \
              panics are the failure signal that surfaces robustness defects in the \
              decoder under test"
)]

use elang_container::{Footer, Header, Index};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // Each call site is independent so the fuzzer can mutate one slice
    // along three different decode paths simultaneously. Errors are
    // expected and discarded; a panic or an out-of-bounds read is the
    // signal of a real defect.
    let _ = Header::decode(data);
    let _ = Footer::decode(data);
    let _ = Index::decode(data);
});
