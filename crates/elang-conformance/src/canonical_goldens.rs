//! OA-14 — the canonical ELD golden corpus (Phase 11, T1121).
//!
//! Steering §9 locks the canonical ELD serialization profile; §12.5 and
//! OA-14 require the profile to be published as **golden files plus their
//! SHA-256 values** so that an independent implementation can be checked
//! byte-for-byte without reading this repository's Rust code.
//!
//! This module owns the corpus:
//!
//! * [`CASES`] — the authored (deliberately *non*-canonical) ELD sources,
//!   one per published behavior. Every source is valid ELD; the point of
//!   each case is the *canonical* bytes it produces.
//! * [`generate`] — runs the real pipeline (`parse` → `inventory` → `canon`)
//!   over every case and returns the canonical bytes with their SHA-256.
//!
//! Nothing here is hand-written: the golden bytes are produced by
//! `elang_eld::canon::to_string`, and the digests are computed from those
//! bytes. The generator binary `gen_canonical_goldens` writes them to
//! `fixtures/valid/canonical/` and publishes the digests in
//! `specs/registry/eld-canonical-vectors.json`.
//!
//! # Corpus layout
//!
//! Each case is one fixture directory:
//!
//! ```text
//! fixtures/valid/canonical/<id>/
//!   input.eld                     the authored source
//!   expected.json                 conformance expected-result record
//!   golden/canonical.eld          the canonical bytes
//!   golden/canonical.eld.sha256   "<hex>  canonical.eld"
//! ```
//!
//! The `input.eld` files are *inputs*, not goldens: they are intentionally
//! written in non-canonical declaration order so that the goldens prove the
//! canonicalization rules do something.

use elang_repertoire::repertoire::Repertoire;

use crate::binary_vectors::{hex, sha256};

/// One published canonical-ELD golden case.
#[derive(Debug, Clone, Copy)]
pub struct GoldenCase {
    /// Stable fixture id — the directory name under `fixtures/valid/canonical/`.
    pub id: &'static str,
    /// One-line human description of the behavior under test.
    pub description: &'static str,
    /// Requirement ids this case exercises.
    pub requirements: &'static [&'static str],
    /// The authored ELD source. Valid ELD, deliberately *not* canonical.
    pub source: &'static str,
}

/// A generated golden: canonical bytes plus the digest of those bytes.
#[derive(Debug, Clone)]
pub struct GeneratedGolden {
    /// The case id (matches [`GoldenCase::id`]).
    pub id: &'static str,
    /// The case description.
    pub description: &'static str,
    /// The requirements the case exercises.
    pub requirements: &'static [&'static str],
    /// The authored source.
    pub source: &'static str,
    /// The canonical ELD bytes (UTF-8, LF line endings, one final LF).
    pub canonical: String,
    /// Lowercase hex SHA-256 of the canonical bytes.
    pub sha256: String,
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// The smallest valid inventory: one consonant, one vowel.
const MINIMAL: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Groups declared in a deliberately scrambled order.
const GROUP_ORDER: &str = r#"phonemes {
  nasals {
    phoneme "ŋ" {
      roman "ng"
    }
  }
  consonants {
    phoneme "t" {
      roman "t"
    }
  }
  diphthongs {
    phoneme "aɪ" {
      roman "ai"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Roots declared out of canonical order.
const ROOT_ORDER: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
    }
    phoneme "k" {
      roman "k"
    }
    phoneme "s" {
      roman "s"
    }
    phoneme "z" {
      roman "z"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Allophones declared out of canonical order, mixing bare and fielded forms.
const ALLOPHONE_ORDER: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      allophone "tʰ" {
        roman "th"
      }
      allophone "ɾ"
      allophone "s"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// A fieldless allophone, which must be emitted as a bare leaf.
const BARE_ALLOPHONE: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      allophone "ɾ"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// A nested Boolean condition tree exercising every predicate kind.
///
/// Every `previous-phoneme` / `next-phoneme` reference must resolve to a
/// declared root or allophone (C-25 / REQ-WHEN-012), so the referenced
/// phonemes are declared in the same inventory.
const CONDITION_TREE: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      when {
        next-phoneme "a"
        word-position "initial"
        group {
          previous-phoneme "s"
          syllable-position "coda"
        }
        or {
          previous-phoneme "z"
          next-phoneme "k"
        }
      }
    }
    phoneme "s" {
      roman "s"
    }
    phoneme "z" {
      roman "z"
    }
    phoneme "k" {
      roman "k"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Duplicate predicates and contradictory conditions — both must survive.
const DUPLICATE_PREDICATES: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      when {
        word-position "initial"
        word-position "initial"
        previous-phoneme "s"
        previous-phoneme "z"
      }
    }
    phoneme "s" {
      roman "s"
    }
    phoneme "z" {
      roman "z"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// The authored *lower* tie bar (U+035C) must be preserved byte-for-byte.
const TIE_BAR_VARIANT: &str = r#"phonemes {
  consonants {
    phoneme "t͜ʃ" {
      roman "tsh"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Romanization case is preserved exactly as authored.
const ROMAN_CASE: &str = r#"phonemes {
  consonants {
    phoneme "θ" {
      roman "Th"
    }
    phoneme "ð" {
      roman "DH"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Permitted Unicode (a combining diacritic and a spacing modifier) is
/// emitted directly as UTF-8 and never silently normalized.
const UNICODE_PRESERVED: &str = r#"phonemes {
  consonants {
    phoneme "tʰ" {
      roman "th"
    }
  }
  vowels {
    phoneme "ã" {
      roman "an"
    }
  }
}
"#;

/// Condition children declared in reverse canonical order.
const CONDITION_ORDER: &str = r#"phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      when {
        previous-phoneme "s"
        next-phoneme "a"
        syllable-position "coda"
        word-position "initial"
      }
    }
    phoneme "s" {
      roman "s"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
"#;

/// Declaration-order variant A (see [`ORDER_VARIANT_B`]).
const ORDER_VARIANT_A: &str = r#"phonemes {
  consonants {
    phoneme "k" {
      roman "k"
    }
    phoneme "t" {
      roman "t"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
    phoneme "i" {
      roman "i"
    }
  }
}
"#;

/// Declaration-order variant B — semantically identical to
/// [`ORDER_VARIANT_A`], different declaration order, identical canonical
/// bytes (REQ-CANON-009 / T1119).
const ORDER_VARIANT_B: &str = r#"phonemes {
  vowels {
    phoneme "i" {
      roman "i"
    }
    phoneme "a" {
      roman "a"
    }
  }
  consonants {
    phoneme "t" {
      roman "t"
    }
    phoneme "k" {
      roman "k"
    }
  }
}
"#;

/// The published corpus.
///
/// The order is stable and is the order the golden files are written in; the
/// registry document lists the cases by id so the order is not load-bearing.
pub const CASES: &[GoldenCase] = &[
    GoldenCase {
        id: "minimal-inventory",
        description: "Smallest valid inventory: one `phonemes` root, one consonants group, one vowels group, one root each (REQ-CANON-001, REQ-CANON-002).",
        requirements: &["REQ-CANON-001", "REQ-CANON-002"],
        source: MINIMAL,
    },
    GoldenCase {
        id: "group-order",
        description: "Groups are emitted `consonants`, `vowels`, then custom groups in ASCII bytewise name order regardless of declaration order (REQ-CANON-001).",
        requirements: &["REQ-CANON-001"],
        source: GROUP_ORDER,
    },
    GoldenCase {
        id: "root-order",
        description: "Roots within a group are sorted by authored NFC IPA UTF-8 bytes (REQ-CANON-002).",
        requirements: &["REQ-CANON-002"],
        source: ROOT_ORDER,
    },
    GoldenCase {
        id: "allophone-order",
        description: "Allophones within a root are sorted by authored NFC IPA UTF-8 bytes, bare and fielded forms alike (REQ-CANON-002).",
        requirements: &["REQ-CANON-002"],
        source: ALLOPHONE_ORDER,
    },
    GoldenCase {
        id: "bare-allophone",
        description: "A fieldless allophone is emitted as a bare leaf node, never with an empty block (REQ-CANON-004).",
        requirements: &["REQ-CANON-004"],
        source: BARE_ALLOPHONE,
    },
    GoldenCase {
        id: "condition-tree",
        description: "Nested `group`/`or` trees keep their nesting and their condition children are sorted by the Canonical Condition Sorting Key v1 (REQ-CANON-005, REQ-CANON-006).",
        requirements: &["REQ-CANON-005", "REQ-CANON-006"],
        source: CONDITION_TREE,
    },
    GoldenCase {
        id: "duplicate-predicates",
        description: "Duplicate predicates and contradictory conditions are preserved verbatim — no Boolean simplification (REQ-CANON-006).",
        requirements: &["REQ-CANON-006"],
        source: DUPLICATE_PREDICATES,
    },
    GoldenCase {
        id: "tie-bar-variant",
        description: "The authored lower tie bar (U+035C) is preserved byte-for-byte; the tie-bar-folded comparison key is used for uniqueness only (REQ-CANON-006).",
        requirements: &["REQ-CANON-006"],
        source: TIE_BAR_VARIANT,
    },
    GoldenCase {
        id: "roman-case",
        description: "Romanization case is preserved exactly as authored (REQ-CANON-006).",
        requirements: &["REQ-CANON-006"],
        source: ROMAN_CASE,
    },
    GoldenCase {
        id: "unicode-preserved",
        description: "Permitted Unicode (combining diacritic, spacing modifier) is emitted directly as UTF-8 and never silently normalized (REQ-CANON-008).",
        requirements: &["REQ-CANON-008"],
        source: UNICODE_PRESERVED,
    },
    GoldenCase {
        id: "condition-order",
        description: "Condition children are emitted in canonical key order, not declaration order (REQ-CANON-005).",
        requirements: &["REQ-CANON-005"],
        source: CONDITION_ORDER,
    },
    GoldenCase {
        id: "order-variant-a",
        description: "Declaration-order variant A — identical canonical bytes to `order-variant-b` (REQ-CANON-009).",
        requirements: &["REQ-CANON-009"],
        source: ORDER_VARIANT_A,
    },
    GoldenCase {
        id: "order-variant-b",
        description: "Declaration-order variant B — identical canonical bytes to `order-variant-a` (REQ-CANON-009).",
        requirements: &["REQ-CANON-009"],
        source: ORDER_VARIANT_B,
    },
];

/// The two corpus cases that must canonicalize to identical bytes.
pub const ORDER_EQUIVALENCE_PAIR: (&str, &str) = ("order-variant-a", "order-variant-b");

/// Run the canonical pipeline over every case and collect the goldens.
///
/// The pipeline is the public Phase 11 workflow: parse the authored source
/// with the pinned repertoire, build the validated core inventory, and
/// serialize it with the canonical writer. A case that fails to parse or
/// validate is a corpus bug, not a runtime condition, so it is reported as an
/// error rather than panicking.
///
/// # Errors
///
/// Returns the id of the first case whose source does not parse or validate.
pub fn generate(rep: &Repertoire) -> Result<Vec<GeneratedGolden>, String> {
    let mut out = Vec::with_capacity(CASES.len());
    for case in CASES {
        let doc = elang_eld::parse(case.source.as_bytes(), rep)
            .map_err(|e| format!("case `{}` does not parse: {e}", case.id))?;
        let inventory = elang_eld::inventory::from_document(&doc, rep)
            .map_err(|e| format!("case `{}` does not validate: {e}", case.id))?;
        let canonical = elang_eld::canon::to_string(&inventory);
        let sha256 = hex(&sha256(canonical.as_bytes()));
        out.push(GeneratedGolden {
            id: case.id,
            description: case.description,
            requirements: case.requirements,
            source: case.source,
            canonical,
            sha256,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "golden-corpus tests assert the corpus is internally consistent; a panic signals a corpus bug"
)]
mod tests {
    use super::*;

    fn rep() -> Repertoire {
        elang_repertoire::repertoire::load().expect("repertoire must load")
    }

    #[test]
    fn every_case_canonicalizes() {
        let generated = generate(&rep()).expect("corpus generates");
        assert_eq!(generated.len(), CASES.len());
        for g in &generated {
            assert!(!g.canonical.is_empty(), "case `{}`", g.id);
            assert!(
                g.canonical.ends_with("}\n"),
                "case `{}` must end with exactly one final LF",
                g.id
            );
            assert!(
                !g.canonical.contains("\r"),
                "case `{}` must use LF line endings only",
                g.id
            );
            assert!(
                !g.canonical.contains("\n\n"),
                "case `{}` must not contain blank lines",
                g.id
            );
            assert_eq!(g.sha256.len(), 64);
        }
    }

    #[test]
    fn canonical_output_is_idempotent() {
        let rep = rep();
        for case in CASES {
            let doc = elang_eld::parse(case.source.as_bytes(), &rep).expect("parses");
            let inventory = elang_eld::inventory::from_document(&doc, &rep).expect("validates");
            let once = elang_eld::canon::to_string(&inventory);
            let doc2 = elang_eld::parse(once.as_bytes(), &rep).expect("canonical output parses");
            let inventory2 = elang_eld::inventory::from_document(&doc2, &rep).expect("validates");
            let twice = elang_eld::canon::to_string(&inventory2);
            assert_eq!(once, twice, "case `{}` is not idempotent", case.id);
        }
    }

    #[test]
    fn declaration_order_variants_agree() {
        let generated = generate(&rep()).expect("corpus generates");
        let (a, b) = ORDER_EQUIVALENCE_PAIR;
        let first = generated.iter().find(|g| g.id == a).expect("case a");
        let second = generated.iter().find(|g| g.id == b).expect("case b");
        assert_eq!(first.canonical, second.canonical);
        assert_eq!(first.sha256, second.sha256);
    }

    #[test]
    fn case_ids_are_unique() {
        let mut ids: Vec<&str> = CASES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate golden case id");
    }
}
