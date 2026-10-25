// Fuzz target for the IPA identifier tokenizer / tie-bar / modifier
// parser (Phase 15, T1507).
//
// Drives arbitrary bytes through `elang_repertoire::ipa::validate`,
// the parser that splits an IPA identifier into logical base units
// and enforces every structural rule from steering §7:
//
// - 1–8 Unicode scalars **and** at most 4 logical base units, both
//   enforced independently.
// - A tie bar (U+0361 / U+035C) joins exactly two bases into one
//   unit; no overlap, no chaining, and a modifier may appear only
//   after the second base of a tied unit.
// - At most one modification per unit — one combining diacritic
//   **or** one spacing modifier, never both.
// - Standalone/leading modifiers and combining marks are rejected.
// - Every base/diacritic/modifier must be in the approved repertoire
//   allowlist; anything else is `IPA_NOT_IN_REPERTOIRE`.
// - The whole string must be NFC and category-clean.
//
// The comparison key is exercised too: it normalises the lower tie
// bar to the upper one so `t͡ʃ` and `t͜ʃ` compare equal while the
// untied `tʃ` stays distinct. A regression in that normalisation
// would break global IPA uniqueness (C-09), which is a
// correctness bug, not just a fuzz finding.
//
// The repertoire is embedded at compile time via `include_str!`, so
// `Repertoire::load()` is I/O-free and cheap to cache in a
// `LazyLock`.
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
    reason = "fuzz harness drives adversarial inputs through the IPA tokenizer; \
              panics are the failure signal that surfaces robustness defects in the \
              unit splitter, tie-bar state machine and modifier classifier"
)]

use std::sync::LazyLock;

use elang_repertoire::ipa;
use elang_repertoire::repertoire::{load_from, Repertoire, REPERTOIRE_JSON};

// The pinned repertoire, parsed once from the embedded JSON and reused
// for every fuzz iteration.
static REPERTOIRE: LazyLock<Repertoire> =
    LazyLock::new(|| load_from(REPERTOIRE_JSON).expect("pinned repertoire parses"));

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // The IPA validator takes a `&str`, so the input must be valid
    // UTF-8 first. A non-UTF-8 sequence is not an IPA identifier at
    // all, so it is rejected before the tokenizer sees it.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // `validate` runs the whole chain: Unicode layer → scalar/unit
    // budgets → allowlist classification → tie-bar state machine →
    // modifier rules. A structured `IpaError` is the expected
    // outcome for adversarial input; a panic is a defect.
    let Ok(identifier) = ipa::validate(text, &REPERTOIRE) else {
        return;
    };

    // The identifier parsed. Its comparison key is what global
    // uniqueness (C-09) is decided on, so exercise it — and assert
    // the invariant that makes it sound: a key never contains the
    // lower tie bar, because it is normalised to the upper one.
    let key = identifier.comparison_key();
    debug_assert!(!key.contains(ipa::TIE_LOWER));
});
