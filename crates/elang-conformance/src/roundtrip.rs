//! T1417 + T1418 — write→read→write byte identity and canonical-order
//! stability.
//!
//! # T1417 — byte identity
//!
//! Writing a package and writing the **same canonical inputs again** must
//! produce **byte-identical** archives. This is the strongest
//! reproducibility guarantee the format can offer: two independent
//! implementations that agree on the canonical inputs agree on the
//! whole archive, byte for byte.
//!
//! The fixture writes the minimal golden package once, reads it back
//! with the production reader, recovers the canonical `package.epf` and
//! `Linguistics/phonemes.eld` payloads from the decoded frames, and
//! writes them again. The two archives must compare byte-for-byte. The
//! rebuild deliberately uses the *recovered* bytes rather than the
//! original source constants — otherwise the check would only prove
//! that the writer is deterministic, and would pass even if the reader
//! silently dropped or altered a resource.
//!
//! # T1418 — canonical order stability
//!
//! The **resource declaration order** in the writer's configuration must
//! not affect the archive bytes. The canonical ELD serializer sorts
//! declarations; the container index sorts paths. A writer that emitted
//! resources in configuration order would produce different archives for
//! the same logical package — breaking reproducibility for every
//! consumer that assembles resources in a different order.
//!
//! This fixture builds the *same logical package* twice — once with the
//! resources declared in one order, once shuffled — and asserts the two
//! archives are byte-identical.
//!
//! # The canonical ELD serializer is order-independent too
//!
//! The ELD source itself is authored in two different declaration orders
//! (consonants before vowels vs. vowels before consonants; phonemes in
//! reverse). The canonical serializer must produce identical bytes for
//! both. That is a separate guarantee from the archive-level one, and it
//! is asserted here as well.

use crate::binary_vectors::sha256;
use crate::golden_packages::{FULL_ELD, MINIMAL_ELD};
use elang_core::model::eld::PhonemeInventory;
use elang_epf::model::EpfPackage;
use elang_repertoire::repertoire::load;

/// The canonical bytes of an ELD source, produced by the production
/// canonicaliser.
fn canonical_eld(source: &str) -> Result<Vec<u8>, String> {
    let rep = load().map_err(|e| format!("repertoire: {e:?}"))?;
    let doc = elang_eld::parse(source.as_bytes(), &rep).map_err(|e| format!("parse: {e:?}"))?;
    let inv =
        elang_eld::inventory::from_document(&doc, &rep).map_err(|e| format!("inventory: {e:?}"))?;
    Ok(elang_eld::canon::to_string(&inv).into_bytes())
}

/// Parse the canonical sources into the typed models the writer consumes.
fn parse_into_models(
    epf_source: &[u8],
    eld_source: &[u8],
) -> Result<(EpfPackage, PhonemeInventory), String> {
    let epf = elang_epf::parse(epf_source).map_err(|e| format!("epf: {e:?}"))?;
    let rep = load().map_err(|e| format!("repertoire: {e:?}"))?;
    let doc = elang_eld::parse(eld_source, &rep).map_err(|e| format!("eld parse: {e:?}"))?;
    let inv = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("eld inventory: {e:?}"))?;
    Ok((epf, inv))
}

/// The two orderings of the same logical inventory.
///
/// Both declare the same six phonemes with the same romanisations; only
/// the declaration order differs.
const ELD_ORDER_A: &str = r#"phonemes {
    consonants {
        phoneme "p" {
            roman "p"
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
    nasals {
        phoneme "ŋ" {
            roman "ng"
        }
    }
}
"#;

/// The same inventory, declared in a different order.
const ELD_ORDER_B: &str = r#"phonemes {
    nasals {
        phoneme "ŋ" {
            roman "ng"
        }
    }
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
        phoneme "p" {
            roman "p"
        }
    }
}
"#;

/// Assert that two archives are byte-identical, returning a descriptive
/// error naming the first differing offset.
fn assert_identical(left: &[u8], right: &[u8], label: &str) -> Result<(), String> {
    if left == right {
        return Ok(());
    }
    let first_diff = left
        .iter()
        .zip(right.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| left.len().min(right.len()));
    Err(format!(
        "{label}: archives differ (len {} vs {}); first difference at byte {first_diff}",
        left.len(),
        right.len()
    ))
}

/// Build an archive from the canonical sources plus the given extras.
fn build_archive(
    epf_source: &[u8],
    eld_source: &[u8],
    extra_resources: Vec<elang_writer::ExtraResource>,
) -> Result<Vec<u8>, String> {
    let (epf, eld) = parse_into_models(epf_source, eld_source)?;
    let config = elang_writer::PackageConfig {
        epf,
        eld,
        extra_resources,
        profile: crate::golden_packages::GOLDEN_PROFILE,
        format_version: crate::golden_packages::FORMAT_VERSION.to_string(),
    };
    elang_writer::write_archive(&config).map_err(|e| format!("{e:?}"))
}

/// The T1417 check: writing the same canonical inputs twice must be
/// byte-identical **and** reading an archive back and writing it again
/// must reproduce the original bytes.
///
/// The second half is the one that matters. The archive is opened with
/// the production reader, verified, and the canonical `package.epf` and
/// `Linguistics/phonemes.eld` payloads are recovered from the decoded
/// frames. The rebuild uses **those recovered bytes**, not the original
/// source constants — so a reader that dropped, reordered or altered a
/// resource would change the rebuilt archive and fail here. A writer
/// that was merely stable across runs, but not stable when its own
/// output was read back, would also fail here.
///
/// # Errors
///
/// Returns a descriptive error when the round-trip diverges.
pub fn roundtrip_is_byte_identical() -> Result<(), String> {
    let archive_a = build_archive(
        crate::golden_packages::MINIMAL_EPF,
        crate::golden_packages::MINIMAL_ELD,
        Vec::new(),
    )?;

    // Read the archive back with the production reader. `verify` runs
    // stages 8–11 and decodes every indexed resource, so the payloads
    // below come out of the frames rather than out of the writer's
    // inputs.
    let verified = elang_reader::Package::open(&archive_a)
        .map_err(|e| format!("reader open: {e:?}"))?
        .verify()
        .map_err(|e| format!("reader verify: {e:?}"))?;
    let epf_bytes = verified
        .get_resource("package.epf")
        .ok_or_else(|| "the archive does not carry `package.epf`".to_string())?;
    let eld_bytes = verified
        .get_resource("Linguistics/phonemes.eld")
        .ok_or_else(|| "the archive does not carry `Linguistics/phonemes.eld`".to_string())?;

    let archive_b = build_archive(epf_bytes, eld_bytes, Vec::new())?;
    assert_identical(&archive_a, &archive_b, "write->read->write")
}

/// The T1418 check: the resource declaration order must not change the
/// archive bytes.
///
/// # Errors
///
/// Returns a descriptive error when the two archives diverge.
pub fn declaration_order_is_stable() -> Result<(), String> {
    let eld_a = canonical_eld(ELD_ORDER_A)?;
    let eld_b = canonical_eld(ELD_ORDER_B)?;
    if eld_a != eld_b {
        return Err(format!(
            "canonical ELD differs by declaration order: {} vs {}",
            crate::binary_vectors::hex(&sha256(&eld_a)),
            crate::binary_vectors::hex(&sha256(&eld_b))
        ));
    }

    let epf = crate::golden_packages::MINIMAL_EPF;
    let extra_a = elang_writer::ExtraResource {
        path: crate::golden_packages::FULL_EXTRA_PATH.to_string(),
        canonical_bytes: crate::golden_packages::FULL_EXTRA_BYTES.to_vec(),
    };
    let extra_b = elang_writer::ExtraResource {
        path: "Linguistics/aaa-first.txt".to_string(),
        canonical_bytes: b"First alphabetically.\n".to_vec(),
    };

    let archive_a = build_archive(epf, &eld_a, vec![extra_a.clone(), extra_b.clone()])?;
    let archive_b = build_archive(epf, &eld_b, vec![extra_b, extra_a])?;
    assert_identical(&archive_a, &archive_b, "shuffled resource order")
}

/// The minimal and full golden ELD sources must canonicalise without
/// error, and the canonical bytes must round-trip through the parser.
///
/// # Errors
///
/// Returns a descriptive error when a golden source fails to canonicalise
/// or the canonical bytes fail to reparse.
pub fn golden_eld_sources_canonicalise() -> Result<(), String> {
    for (label, source) in [("minimal", MINIMAL_ELD), ("full", FULL_ELD.as_bytes())] {
        let text = std::str::from_utf8(source).map_err(|e| format!("{label}: {e:?}"))?;
        let canonical = canonical_eld(text)?;
        let rep = load().map_err(|e| format!("repertoire: {e:?}"))?;
        elang_eld::parse(&canonical, &rep)
            .map_err(|e| format!("{label} canonical bytes do not parse: {e:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_write_is_byte_identical() {
        roundtrip_is_byte_identical().unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn declaration_order_does_not_change_the_archive() {
        declaration_order_is_stable().unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn golden_eld_sources_canonicalise_and_reparse() {
        golden_eld_sources_canonicalise().unwrap_or_else(|e| panic!("{e}"));
    }

    /// The mismatch path of the byte-identity assertion: two archives
    /// that differ must be reported with the first differing offset.
    #[test]
    fn coverage_gap_assert_identical_reports_the_first_difference() {
        let err = assert_identical(&[1, 2, 3], &[1, 9, 3], "probe")
            .expect_err("different archives must be reported");
        assert!(err.contains("archives differ"), "{err}");
        assert!(err.contains("first difference at byte 1"), "{err}");

        // A shorter right-hand side is a prefix of the left-hand side:
        // there is no differing byte to point at, so the common length
        // is reported instead.
        let err = assert_identical(&[1, 2, 3], &[1], "probe")
            .expect_err("different archives must be reported");
        assert!(err.contains("len 3 vs 1"), "{err}");
        assert!(err.contains("first difference at byte 1"), "{err}");
    }
}
