// Fuzz target for EPF schema validation (Phase 15, T1505).
//
// Drives arbitrary byte sequences through `elang_epf::parse`, the
// reader for `package.epf` — the resource that declares the EPF/ELD
// version triple, language identifier and resource manifest.
//
// The parser enforces:
//
// - `MAX_EPF_BYTES` (1 MiB) as an unconditional input-size ceiling,
//   applied before any KDL allocation (T1003) — a hostile archive
//   cannot drive the lexer into O(n) work on attacker-controlled
//   bytes.
// - The KDL parser budgets inherited from `elang_kdl` (nesting depth,
//   token/string/comment bytes, node count).
// - Every C-NN / REQ-EPF-* schema rule: top-level `epf`/`eld`/
//   `language-version`/`metadata`/`manifest` nodes, the language URN
//   syntax, the `manifest.phonemes` path, and the kind-classification
//   of every node.
//
// The contract under fuzz is the same as the contract under attack:
// no panic, no read past the input slice, no unbounded allocation, no
// successful parse on a malformed document, and every rejectable
// condition produces a structured `Error` carrying a registered code.
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
    reason = "fuzz harness drives adversarial inputs through the EPF schema; \
              panics are the failure signal that surfaces robustness defects in the \
              parser, URN validator and schema checks under test"
)]

use elang_epf;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // `parse` runs the full chain: 1 MiB input-size cap → KDL lex →
    // KDL parse → EPF schema validation → `EpfPackage` model.
    // Errors are expected and discarded; a panic or OOM is a defect.
    let _ = elang_epf::parse(data);
});
