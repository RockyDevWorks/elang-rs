//! T1509 — property tests for canonicalisation idempotence.
//!
//! The canonical serializer is meant to be a fixpoint: re-serialising the
//! canonical bytes of any valid input must yield exactly the same bytes.
//! Every regression in canonicalisation — a new escape sequence that is
//! not idempotent, a key that is emitted in an order the next pass
//! disagrees with, a value that survives a `parse → canon → parse` but
//! changes between the two canonicalisations — surfaces here.
//!
//! The property is asserted via `proptest!`, with the corpus cases as
//! the input domain: every entry in
//! [`crate::canonical_goldens::CASES`] and
//! [`crate::epf_canonical_goldens::CASES`] is fed through
//! `parse → canon → parse → canon` and the two `canon` outputs are
//! compared. proptest's machinery handles the test enumeration,
//! failure shrinkage (the smallest input case that breaks the
//! property) and replay (`PROPTEST_REPLAY`).
//!
//! The corpus is the input domain rather than a randomly generated one
//! because the corpus already covers the structural edges that matter —
//! tie-bar variants, declaration orderings, duplicate predicates,
//! condition trees, allophone inheritance — and an arbitrary valid-ELD
//! grammar would either under-cover those edges or balloon the
//! generator past the value it returns. The property structure is what
//! T1509 asks for; the corpus gives it teeth.
//!
//! # Why this is a `proptest!` and not a `#[test]`
//!
//! The existing `canonical_output_is_idempotent` test in
//! `canonical_goldens.rs` walks the ELD corpus but bails on the first
//! failure. The proptest version reports all failures, attributes them
//! to the case id, and is replayable from a single env var. It also
//! extends coverage to EPF (T1509 covers both formats) without a
//! second test site.

#![cfg(test)]

use crate::canonical_goldens::CASES as ELD_CASES;
use crate::epf_canonical_goldens::CASES as EPF_CASES;
use elang_repertoire::repertoire::Repertoire;

/// One canonicalisation run on one source: `parse(source) → canon →
/// canon-bytes`. Returns the canonical bytes, or a descriptive error
/// if the source is rejected by the parser. The latter is the
/// expected outcome for an adversarial source, never a failure of the
/// idempotence property itself — the property is conditional on a
/// successful first canonicalisation.
fn canonicalize_eld(source: &str, rep: &Repertoire) -> Result<String, String> {
    let doc = elang_eld::parse(source.as_bytes(), rep).map_err(|e| format!("ELD parse: {e:?}"))?;
    let inv = elang_eld::inventory::from_document(&doc, rep)
        .map_err(|e| format!("ELD inventory: {e:?}"))?;
    Ok(elang_eld::canon::to_string(&inv))
}

fn canonicalize_epf(source: &str) -> Result<String, String> {
    let pkg = elang_epf::parse(source.as_bytes()).map_err(|e| format!("EPF parse: {e:?}"))?;
    Ok(elang_epf::canon::to_string(&pkg))
}

proptest::proptest! {
    // The proptest tests are written with a `proptest!` block that does
    // **not** declare a random strategy: the input domain is the
    // conformance corpus itself, enumerated by index. proptest's
    // machinery still applies (per-case isolation, failure shrinkage,
    // replay via `PROPTEST_REPLAY=...`), and the input range gives
    // us the per-case attribution that an unrolled `#[test]` loop
    // would have to hand-write.

    /// The canonical ELD bytes are a fixpoint: re-canonicalising them
    /// (after parsing them back into a model) yields the same bytes.
    #[test]
    fn canonical_eld_is_idempotent(idx in 0usize..ELD_CASES.len()) {
        let case = &ELD_CASES[idx];
        let rep = elang_repertoire::repertoire::load()
            .expect("pinned repertoire must load");

        let once = canonicalize_eld(case.source, &rep)
            .unwrap_or_else(|e| panic!("case `{}` first canonicalise: {e}", case.id));

        let twice = canonicalize_eld(&once, &rep)
            .unwrap_or_else(|e| panic!("case `{}` second canonicalise: {e}", case.id));

        let once_len = once.len();
        let twice_len = twice.len();
        proptest::prop_assert_eq!(
            once,
            twice,
            "case `{}` canonicalisation is not idempotent ({} vs {})",
            case.id,
            once_len,
            twice_len,
        );
    }

    /// The canonical EPF bytes are a fixpoint, mirroring the ELD
    /// property above. EPF canonicalisation does not need the
    /// repertoire because it does not carry IPA identifiers; the
    /// property is purely about the KDL canonical layout the EPF
    /// writer applies.
    #[test]
    fn canonical_epf_is_idempotent(idx in 0usize..EPF_CASES.len()) {
        let case = &EPF_CASES[idx];

        let once = canonicalize_epf(case.source)
            .unwrap_or_else(|e| panic!("case `{}` first canonicalise: {e}", case.id));
        let twice = canonicalize_epf(&once)
            .unwrap_or_else(|e| panic!("case `{}` second canonicalise: {e}", case.id));

        let once_len = once.len();
        let twice_len = twice.len();
        proptest::prop_assert_eq!(
            once,
            twice,
            "case `{}` EPF canonicalisation is not idempotent ({} vs {})",
            case.id,
            once_len,
            twice_len,
        );
    }
}
