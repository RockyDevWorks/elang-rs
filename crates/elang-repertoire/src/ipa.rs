//! IPA identifier validation against the pinned repertoire (steering §7).
//!
//! An IPA identifier is a validated ELD string used as a `phoneme`/`allophone`
//! argument or a `previous-phoneme`/`next-phoneme` value. This module parses an
//! identifier into **logical base units** and enforces the locked structural
//! rules:
//!
//! - every scalar is NFC and category-clean (delegated to [`crate::unicode`]);
//! - every base/diacritic/modifier is in the approved allowlist
//!   ([`crate::repertoire`]); anything else is rejected;
//! - length 1–8 Unicode scalars **and** at most 4 logical base units (both
//!   enforced independently);
//! - a tie bar U+0361/U+035C joins exactly two bases into one unit; no overlap
//!   or chaining; a tied-unit modifier may appear only after the second base;
//! - at most one modification per unit (one combining diacritic **or** one
//!   spacing modifier, never both);
//! - standalone/leading modifiers and combining marks are rejected;
//! - a comparison key treats upper/lower tie bar as equivalent (uniqueness).
//!
//! Prosodic notation (stress, length, tone, intonation) is not in the
//! allowlist, so it is rejected here as "not in repertoire".

use std::collections::BTreeSet;

use crate::repertoire::Repertoire;
use crate::unicode::UnicodeValidator;

/// Upper tie bar (COMBINING DOUBLE INVERTED BREVE).
pub const TIE_UPPER: char = '\u{0361}';
/// Lower tie bar (COMBINING DOUBLE BREVE BELOW).
pub const TIE_LOWER: char = '\u{035C}';

/// Max Unicode scalars in an identifier (steering §7).
pub const MAX_SCALARS: usize = 8;
/// Max logical base units in an identifier (steering §7).
pub const MAX_UNITS: usize = 4;

/// Why an IPA identifier is invalid. Codes map to the `IPA` category
/// (`specs/registry/error-codes.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpaError {
    /// Empty identifier.
    Empty,
    /// More than 8 Unicode scalars. Code: `IPA_TOO_MANY_SCALARS`.
    TooManyScalars(usize),
    /// More than 4 logical base units. Code: `IPA_TOO_MANY_UNITS`.
    TooManyUnits(usize),
    /// A base symbol is not in the approved allowlist. Code:
    /// `IPA_NOT_IN_REPERTOIRE`.
    BaseNotApproved(char),
    /// A modifier/diacritic is not in the approved allowlist, or a disallowed
    /// scalar appeared. Code: `IPA_NOT_IN_REPERTOIRE`.
    ModifierNotApproved(char),
    /// A modifier or combining mark had no preceding base. Code:
    /// `IPA_STANDALONE_MODIFIER`.
    StandaloneModifier(char),
    /// More than one modification on a single unit. Code:
    /// `IPA_TOO_MANY_MODIFICATIONS`.
    TooManyModifications,
    /// Tie bar misused: not exactly two bases, chained, or a modifier appeared
    /// between the two tied bases. Code: `IPA_TIE_BAR_INVALID`.
    TieBarInvalid,
    /// A scalar failed a Unicode rule. Code: `UNICODE_*` (see inner).
    Unicode(crate::unicode::UnicodeViolation),
}

impl IpaError {
    /// The stable error code from the registry (`specs/registry/error-codes.md`)
    /// for this failure. Unicode violations delegate to their own code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            // `Empty` is a schema-level emptiness; the closest stable code is
            // the standalone-modifier/zero-content family. It is reported as
            // `IPA_NOT_IN_REPERTOIRE` here only if it ever surfaced from a
            // non-empty parse; a truly empty identifier is rejected before the
            // ELD schema would ever accept it. Kept explicit for completeness.
            IpaError::Empty => "IPA_EMPTY",
            IpaError::TooManyScalars(_) => "IPA_TOO_MANY_SCALARS",
            IpaError::TooManyUnits(_) => "IPA_TOO_MANY_UNITS",
            IpaError::BaseNotApproved(_) | IpaError::ModifierNotApproved(_) => {
                "IPA_NOT_IN_REPERTOIRE"
            }
            IpaError::StandaloneModifier(_) => "IPA_STANDALONE_MODIFIER",
            IpaError::TooManyModifications => "IPA_TOO_MANY_MODIFICATIONS",
            IpaError::TieBarInvalid => "IPA_TIE_BAR_INVALID",
            IpaError::Unicode(v) => v.code(),
        }
    }
}

/// One logical base unit after parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// One base symbol, or two if tied.
    pub bases: Vec<char>,
    /// The tie bar used (normalized away in the comparison key), if tied.
    pub tie: Option<char>,
    /// The single optional modification (combining diacritic or spacing mod).
    pub modifier: Option<char>,
}

/// A fully validated IPA identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpaIdentifier {
    /// Original NFC string, preserved exactly (including tie-bar variant).
    pub original: String,
    /// Parsed logical units.
    pub units: Vec<Unit>,
}

impl IpaIdentifier {
    /// A comparison key for global uniqueness: identical to `original` except
    /// the lower tie bar U+035C is normalized to the upper tie bar U+0361, so
    /// `t͡ʃ` and `t͜ʃ` compare equal while `tʃ` (untied) stays distinct.
    #[must_use]
    pub fn comparison_key(&self) -> String {
        self.original
            .chars()
            .map(|c| if c == TIE_LOWER { TIE_UPPER } else { c })
            .collect()
    }
}

/// Classify a scalar against the repertoire.
enum Kind {
    Base,
    Combining,
    Spacing,
    Tie,
    Unknown,
}

struct Classifier {
    bases: BTreeSet<char>,
    combining: BTreeSet<char>,
    spacing: BTreeSet<char>,
}

impl Classifier {
    fn from_repertoire(rep: &Repertoire) -> Self {
        // All approved entries here are single-scalar.
        let collect = |v: &[crate::repertoire::Symbol]| -> BTreeSet<char> {
            v.iter()
                .filter_map(|s| s.codepoints.first().copied())
                .collect()
        };
        Self {
            bases: collect(&rep.base_symbols),
            combining: collect(&rep.combining_diacritics),
            spacing: collect(&rep.spacing_modifiers),
        }
    }

    fn classify(&self, c: char) -> Kind {
        if c == TIE_UPPER || c == TIE_LOWER {
            Kind::Tie
        } else if self.bases.contains(&c) {
            Kind::Base
        } else if self.combining.contains(&c) {
            Kind::Combining
        } else if self.spacing.contains(&c) {
            Kind::Spacing
        } else {
            Kind::Unknown
        }
    }
}

/// Validate and parse an IPA identifier against the repertoire and Unicode
/// semantics.
pub fn validate(identifier: &str, rep: &Repertoire) -> Result<IpaIdentifier, IpaError> {
    if identifier.is_empty() {
        return Err(IpaError::Empty);
    }

    // Unicode layer: NFC + category/noncharacter checks on the whole string.
    let uni = UnicodeValidator::new();
    if let Err(v) = uni.validate_str(identifier) {
        // NFC failure and forbidden categories both surface here. Combining
        // marks (Mn) are allowed by category, so only genuinely forbidden
        // scalars or non-NFC input fail at this step.
        if v != crate::unicode::UnicodeViolation::NotNfc {
            // A forbidden category on a non-allowlisted scalar is reported as
            // a Unicode violation; allowlisted combining marks never reach
            // here because they are category Mn (allowed).
            return Err(IpaError::Unicode(v));
        }
        return Err(IpaError::Unicode(v));
    }

    let scalars: Vec<char> = identifier.chars().collect();
    if scalars.len() > MAX_SCALARS {
        return Err(IpaError::TooManyScalars(scalars.len()));
    }

    let cls = Classifier::from_repertoire(rep);
    let mut units: Vec<Unit> = Vec::new();
    let mut i = 0usize;

    // `i` only ever moves forward via `saturating_add` and is always read with
    // `.get()`, so there is no unchecked indexing or overflow on this cursor.
    while let Some(&c) = scalars.get(i) {
        match cls.classify(c) {
            Kind::Base => {
                // Start a new unit with this base.
                let mut unit = Unit {
                    bases: vec![c],
                    tie: None,
                    modifier: None,
                };
                i = i.saturating_add(1);

                // Optional tie bar -> must be followed by a second base.
                if let Some(&maybe_tie) = scalars.get(i) {
                    if let Kind::Tie = cls.classify(maybe_tie) {
                        i = i.saturating_add(1);
                        // Next must be a base (no modifier between tied bases).
                        let Some(&next) = scalars.get(i) else {
                            return Err(IpaError::TieBarInvalid);
                        };
                        match cls.classify(next) {
                            Kind::Base => {
                                unit.bases.push(next);
                                unit.tie = Some(maybe_tie);
                                i = i.saturating_add(1);
                            }
                            _ => return Err(IpaError::TieBarInvalid),
                        }
                        // No chaining: another tie immediately here is invalid.
                        if let Some(&after) = scalars.get(i) {
                            if let Kind::Tie = cls.classify(after) {
                                return Err(IpaError::TieBarInvalid);
                            }
                        }
                    }
                }

                // Optional single modification (combining or spacing).
                if let Some(&m) = scalars.get(i) {
                    if matches!(cls.classify(m), Kind::Combining | Kind::Spacing) {
                        unit.modifier = Some(m);
                        i = i.saturating_add(1);
                        // A second modification on the same unit is invalid.
                        if let Some(&m2) = scalars.get(i) {
                            if matches!(cls.classify(m2), Kind::Combining | Kind::Spacing) {
                                return Err(IpaError::TooManyModifications);
                            }
                        }
                    }
                }

                units.push(unit);
            }
            Kind::Combining | Kind::Spacing => {
                return Err(IpaError::StandaloneModifier(c));
            }
            Kind::Tie => return Err(IpaError::TieBarInvalid),
            Kind::Unknown => {
                // Not in any approved inventory.
                return Err(IpaError::BaseNotApproved(c));
            }
        }
    }

    if units.is_empty() {
        return Err(IpaError::Empty);
    }
    if units.len() > MAX_UNITS {
        return Err(IpaError::TooManyUnits(units.len()));
    }

    Ok(IpaIdentifier {
        original: identifier.to_string(),
        units,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::repertoire::load;

    fn rep() -> Repertoire {
        load().expect("repertoire")
    }

    #[test]
    fn single_base_ok() {
        let r = rep();
        let id = validate("t", &r).expect("t is valid");
        assert_eq!(id.units.len(), 1);
        assert_eq!(id.units[0].bases, vec!['t']);
    }

    #[test]
    fn untied_two_units() {
        let r = rep();
        let id = validate("t\u{0283}", &r).expect("tʃ"); // t + ʃ
        assert_eq!(id.units.len(), 2);
    }

    #[test]
    fn tied_one_unit() {
        let r = rep();
        let id = validate("t\u{0361}\u{0283}", &r).expect("t͡ʃ"); // t + tie + ʃ
        assert_eq!(id.units.len(), 1);
        assert_eq!(id.units[0].bases, vec!['t', '\u{0283}']);
        assert_eq!(id.units[0].tie, Some(TIE_UPPER));
    }

    #[test]
    fn tie_variants_compare_equal_but_are_preserved() {
        let r = rep();
        let upper = validate("t\u{0361}\u{0283}", &r).expect("upper tie");
        let lower = validate("t\u{035C}\u{0283}", &r).expect("lower tie");
        assert_ne!(upper.original, lower.original, "originals preserved");
        assert_eq!(
            upper.comparison_key(),
            lower.comparison_key(),
            "tie variants equal for uniqueness"
        );
    }

    #[test]
    fn untied_and_tied_are_distinct() {
        let r = rep();
        let untied = validate("t\u{0283}", &r).expect("tʃ");
        let tied = validate("t\u{0361}\u{0283}", &r).expect("t͡ʃ");
        assert_ne!(untied.comparison_key(), tied.comparison_key());
    }

    #[test]
    fn aspirated_modifier_ok() {
        let r = rep();
        let id = validate("t\u{02B0}", &r).expect("tʰ"); // t + modifier h
        assert_eq!(id.units.len(), 1);
        assert_eq!(id.units[0].modifier, Some('\u{02B0}'));
    }

    #[test]
    fn two_modifications_rejected() {
        let r = rep();
        // t + ʰ (spacing) + ʷ (spacing) on one base -> invalid
        assert_eq!(
            validate("t\u{02B0}\u{02B7}", &r),
            Err(IpaError::TooManyModifications)
        );
    }

    #[test]
    fn standalone_modifier_rejected() {
        let r = rep();
        assert_eq!(
            validate("\u{02B0}", &r),
            Err(IpaError::StandaloneModifier('\u{02B0}'))
        );
    }

    #[test]
    fn leading_combining_mark_rejected() {
        let r = rep();
        // lone combining tilde U+0303
        assert!(matches!(
            validate("\u{0303}", &r),
            Err(IpaError::StandaloneModifier(_)) | Err(IpaError::Unicode(_))
        ));
    }

    #[test]
    fn unapproved_symbol_rejected() {
        let r = rep();
        // U+1D00 LATIN LETTER SMALL CAPITAL A is not in the IPA base allowlist.
        assert_eq!(
            validate("\u{1D00}", &r),
            Err(IpaError::BaseNotApproved('\u{1D00}'))
        );
    }

    #[test]
    fn prosody_rejected_as_not_approved() {
        let r = rep();
        // ː length mark U+02D0 is excluded prosody -> not in allowlist.
        assert_eq!(
            validate("t\u{02D0}", &r),
            Err(IpaError::BaseNotApproved('\u{02D0}'))
        );
    }

    #[test]
    fn too_many_scalars_rejected() {
        let r = rep();
        // 9 bases = 9 scalars > 8
        assert!(matches!(
            validate("aaaaaaaaa", &r),
            Err(IpaError::TooManyScalars(9))
        ));
    }

    #[test]
    fn too_many_units_rejected() {
        let r = rep();
        // 5 single-base units (5 scalars) -> units > 4 (scalars still <= 8)
        assert!(matches!(
            validate("aeiou", &r),
            Err(IpaError::TooManyUnits(5))
        ));
    }

    #[test]
    fn modifier_between_tied_bases_rejected() {
        let r = rep();
        // t + ʰ + tie + ʃ : modifier before the tie -> the tie now follows a
        // modified base; per rule the modifier for a tied unit must be after
        // the second base, so this must be rejected.
        assert_eq!(
            validate("t\u{02B0}\u{0361}\u{0283}", &r),
            Err(IpaError::TieBarInvalid)
        );
    }
}
