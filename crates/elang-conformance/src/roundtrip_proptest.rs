//! T1510 — property tests for parse/serialise roundtrips of validated
//! models.
//!
//! For every valid input, the pipeline
//!
//! ```text
//!     parse(source) → model → canonicalise → bytes1
//!     parse(bytes1)  → model' → canonicalise → bytes2
//! ```
//!
//! must yield `bytes1 == bytes2`. The first `canonicalise` proves
//! `bytes1` is the canonical form of `source`; the second proves the
//! canonical form is itself parseable and re-canonicalises to itself.
//!
//! Combined with the T1509 idempotence property, this gives the full
//! "parse, canonicalise, parse, canonicalise is a fixpoint" guarantee
//! every archive round-trip relies on: a writer that emits `bytes1`
//! can be read back, the reader produces `model'`, and `model'`
//! canonicalises to `bytes1` again.
//!
//! The property is asserted via `proptest!`, with the corpus cases
//! as the input domain. As with T1509, the corpus is the input
//! domain rather than randomly generated valid ELDs because the
//! corpus already covers the structural edges (group orderings,
//! allophone inheritance, condition trees, tie-bar variants).

#![cfg(test)]

use crate::canonical_goldens::CASES as ELD_CASES;
use crate::epf_canonical_goldens::CASES as EPF_CASES;
use elang_repertoire::repertoire::Repertoire;

/// `parse(source) → canon → parse(canon) → canon`.
///
/// Returns the two canonical-bytes outputs, or a descriptive error
/// if any step fails.
fn roundtrip_eld(source: &str, rep: &Repertoire) -> Result<(String, String), String> {
    let doc =
        elang_eld::parse(source.as_bytes(), rep).map_err(|e| format!("first parse: {e:?}"))?;
    let inv = elang_eld::inventory::from_document(&doc, rep)
        .map_err(|e| format!("first inventory: {e:?}"))?;
    let once = elang_eld::canon::to_string(&inv);

    let doc2 =
        elang_eld::parse(once.as_bytes(), rep).map_err(|e| format!("second parse: {e:?}"))?;
    let inv2 = elang_eld::inventory::from_document(&doc2, rep)
        .map_err(|e| format!("second inventory: {e:?}"))?;
    let twice = elang_eld::canon::to_string(&inv2);

    Ok((once, twice))
}

fn roundtrip_epf(source: &str) -> Result<(String, String), String> {
    let pkg = elang_epf::parse(source.as_bytes()).map_err(|e| format!("first parse: {e:?}"))?;
    let once = elang_epf::canon::to_string(&pkg);

    let pkg2 = elang_epf::parse(once.as_bytes()).map_err(|e| format!("second parse: {e:?}"))?;
    let twice = elang_epf::canon::to_string(&pkg2);

    Ok((once, twice))
}

proptest::proptest! {
    /// `parse(source) → canon → parse(canon) → canon` is a fixpoint
    /// for every golden ELD source. The second canonicalisation
    /// must equal the first; a divergence means the canonical output
    /// is not parseable back to a model whose canonical form is
    /// itself — a class of bug that has historically caught
    /// regressions in romanisation inheritance and tie-bar
    /// normalisation.
    #[test]
    fn eld_roundtrip_through_canonical_form_is_a_fixpoint(idx in 0usize..ELD_CASES.len()) {
        let case = &ELD_CASES[idx];
        let rep = elang_repertoire::repertoire::load()
            .expect("pinned repertoire must load");

        let (once, twice) = roundtrip_eld(case.source, &rep)
            .unwrap_or_else(|e| panic!("case `{}` roundtrip: {e}", case.id));

        let once_len = once.len();
        let twice_len = twice.len();
        proptest::prop_assert_eq!(
            once,
            twice,
            "case `{}` round-trip diverges ({} vs {})",
            case.id,
            once_len,
            twice_len,
        );
    }

    /// Mirror of the ELD property for EPF: `parse → canon →
    /// parse(canon) → canon` is a fixpoint for every golden EPF
    /// source. The EPF parser does not depend on the repertoire, so
    /// this test runs without it.
    #[test]
    fn epf_roundtrip_through_canonical_form_is_a_fixpoint(idx in 0usize..EPF_CASES.len()) {
        let case = &EPF_CASES[idx];

        let (once, twice) = roundtrip_epf(case.source)
            .unwrap_or_else(|e| panic!("case `{}` roundtrip: {e}", case.id));

        let once_len = once.len();
        let twice_len = twice.len();
        proptest::prop_assert_eq!(
            once,
            twice,
            "case `{}` EPF round-trip diverges ({} vs {})",
            case.id,
            once_len,
            twice_len,
        );
    }
}
