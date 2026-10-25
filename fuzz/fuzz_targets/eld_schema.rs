// Fuzz target for ELD structural validation (Phase 15, T1506).
//
// Drives arbitrary byte sequences through `elang_eld::parse`, the
// reader for the phoneme `.eld` resource — the document that
// declares every phoneme, allophone, romanization and phonological
// `when` block.
//
// The parser enforces, per constraint C-NN in
// `specs/grammar/eld-0.1.0.md`:
//
// - Document/group structure (C-01..C-05).
// - Root phoneme shape and cardinality (C-06..C-08).
// - Allophone shape and cardinality (C-10..C-12).
// - Romanization syntax and uniqueness (C-13..C-15).
// - Romanization inheritance (C-16).
// - Global IPA uniqueness including tie-bar aliases (C-09).
// - IPA validity against the pinned repertoire (`elang-repertoire`).
//
// T1506 covers the structural path end-to-end. The condition language
// (C-20..C-27) is exercised separately by `phon_condition`.
//
// The repertoire is embedded at compile time via `include_str!`, so
// `Repertoire::load()` is I/O-free and cheap to cache in a
// `LazyLock`. Loading it on every fuzz call would dominate runtime
// without changing what the fuzzer exercises.
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
    reason = "fuzz harness drives adversarial inputs through the ELD schema; \
              panics are the failure signal that surfaces robustness defects in the \
              KDL parser, IPA validator, schema checks and tie-bar normalisation"
)]

use std::sync::LazyLock;

use elang_eld;
use elang_repertoire::repertoire::{load_from, Repertoire, REPERTOIRE_JSON};

// `load_from(REPERTOIRE_JSON)` parses the embedded JSON
// once and reuses it for every fuzz iteration. `load()` would also
// work; the explicit `load_from` keeps the dependency on
// `REPERTOIRE_JSON` visible at the call site, which is what the
// supply-chain policy in `SECURITY.md` asks for.
static REPERTOIRE: LazyLock<Repertoire> =
    LazyLock::new(|| load_from(REPERTOIRE_JSON).expect("pinned repertoire parses"));

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // `parse` runs the full structural chain: KDL lex → KDL parse →
    // IPA validation → group/root/allophone shape → romanization
    // uniqueness → global IPA uniqueness (including tie-bar aliases).
    // Errors are expected and discarded; a panic is a defect.
    let _ = elang_eld::parse(data, &REPERTOIRE);
});
