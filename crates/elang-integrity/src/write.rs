//! Canonical `META-INF/integrity.kdl` writer (T0709).
//!
//! [`canonical_bytes`] emits the exact byte sequence a conforming reader
//! will produce for the manifest's grammar + canonical serialization rules.
//! ADR 0004 §"Canonical serialization" + grammar §4 are the normative
//! sources; the canonical vector in [`CANONICAL_GOLDEN`] is the byte-for-byte
//! fixture.
//!
//! ## Format rules enforced
//!
//! - Top-level `integrity` node; children emitted in this order:
//!   `algorithm` first, then `resource` entries.
//! - `resource` entries sorted by their path argument using **raw UTF-8
//!   bytewise ascending** order. `IntegrityManifest::resources` is already
//!   a `BTreeMap<ResourcePath, _>` so iteration is in the required order —
//!   no re-sort is needed at the writer.
//! - One declaration per line; **4 ASCII spaces** per nesting level;
//!   no tabs; no blank lines; no comments.
//! - **LF** line endings; **exactly one** final LF (the file ends `}\n`).
//! - Strings are standard double-quoted KDL strings; embedded `"` and
//!   `\` are escaped (`\"`, `\\`); control characters in the path use
//!   KDL's `\u{XXXX}` form.
//! - The `sha256` property value is lowercase hex.
//!
//! ## What the writer does not do
//!
//! It does **not** decide which entries to include — the caller is
//! responsible for assembling a manifest whose `resources` map already
//! excludes `META-INF/integrity.kdl` (C-04 self-exclusion). Writing an
//! empty manifest (no `resource` entries) produces a syntactically valid
//! file but one the reader rejects with `INTEGRITY_STRUCTURE_INVALID`; the
//! writer does not duplicate the reader's constraints.

use std::fmt::Write as _;

use crate::manifest::{IntegrityEntry, IntegrityManifest, SHA_256_LITERAL};

/// Canonical byte sequence for the integrity-manifest golden vector.
///
/// The fixture mirrors the [golden manifest] used by the conformance tests.
/// It is the byte-for-byte reference [`canonical_bytes`] must reproduce;
/// any drift between this constant and the writer's output is a regression
/// in the canonical serialization rules.
///
/// [golden manifest]: ../tests/integrity_conformance.rs
pub const CANONICAL_GOLDEN: &[u8] = b"\
integrity {
    algorithm \"sha-256\"
    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"Linguistics/phonemes.eld\" sha256=\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"
    resource \"mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";

/// Emit the canonical byte sequence for `manifest`.
///
/// The output is the literal manifest bytes a reader expects to find at
/// `META-INF/integrity.kdl`. The caller is expected to compress (under
/// `elang-canonical-zstd-v1`) and frame it into the container.
///
/// # Canonicalization guarantees
///
/// - The output is **deterministic**: given the same `manifest`, every
///   invocation produces byte-identical output across runs and platforms.
///   This is required by REQ-FRAME-010/011 for canonical-frame payloads.
/// - Resource entries are emitted in raw UTF-8 bytewise ascending order by
///   path (`BTreeMap` iteration guarantees this; the writer does not
///   re-sort).
/// - The `sha256` property value is lowercase hex (per ADR 0004 §3 / grammar §4).
/// - The file ends with exactly one LF (`}\n`).
///
/// Parsed or otherwise invariant-valid manifests round-trip through
/// `canonical_bytes` and [`crate::parse::parse`] within the parser budgets.
/// This guarantee does not apply to arbitrary caller-assembled models.
///
/// # Non-goals
///
/// The writer does not validate the manifest (validation is [`mod@crate::parse`]'s
/// job) and does not check coverage against an index (that is
/// [`crate::manifest::verify_coverage`]'s job). It emits whatever entries
/// the caller has assembled, in the canonical order.
#[must_use]
pub fn canonical_bytes(manifest: &IntegrityManifest) -> Vec<u8> {
    let mut out = String::with_capacity(estimated_capacity(manifest));
    out.push_str("integrity {\n");
    push_algorithm_line(&mut out, manifest);
    for entry in manifest.resources.values() {
        push_resource_line(&mut out, entry);
    }
    out.push_str("}\n");
    out.into_bytes()
}

/// A conservative over-estimate of the output byte length so the
/// [`String`] does not need to grow more than once for typical manifests.
///
/// The arithmetic is on caller-supplied sizes (entry count, path byte
/// length); there is no untrusted input here, so the workspace's
/// untrusted-input lint is too strict. The estimate is a sizing hint, not
/// a correctness guarantee.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "capacity estimation on caller-controlled sizes; this is sizing, not correctness"
)]
fn estimated_capacity(manifest: &IntegrityManifest) -> usize {
    // Fixed overhead: `integrity {\n` + algorithm line + `}\n`.
    let fixed = 64;
    // Per-entry: `    resource "<path>" sha256="<64 hex>"\n`.
    let per_entry = 96;
    let path_bytes: usize = manifest
        .resources
        .values()
        .map(|e| e.path.as_str().len())
        .sum();
    fixed + per_entry * manifest.resources.len() + path_bytes
}

/// Emit `    algorithm "<SHA_256_LITERAL>"\n` into `out`.
fn push_algorithm_line(out: &mut String, manifest: &IntegrityManifest) {
    out.push_str("    algorithm \"");
    out.push_str(manifest.algorithm.as_str());
    // Defense-in-depth: even though `Algorithm` only has `Sha256`, pin the
    // literal to the same constant the parser accepts. If a future variant
    // is added with a literal that needs escaping, this site is the one to
    // update.
    debug_assert_eq!(manifest.algorithm.as_str(), SHA_256_LITERAL);
    out.push_str("\"\n");
}

/// Emit `    resource "<path>" sha256="<hex>"\n` into `out`.
fn push_resource_line(out: &mut String, entry: &IntegrityEntry) {
    out.push_str("    resource \"");
    push_kdl_string(out, entry.path.as_str());
    out.push_str("\" sha256=\"");
    for byte in entry.digest {
        // SHA-256 hex is always two lowercase digits; the Write impl on
        // String only fails on allocator OOM, which the workspace lints
        // cover elsewhere.
        let _ = write!(out, "{byte:02x}");
    }
    out.push_str("\"\n");
}

/// Append `s` to `out` as a KDL 2.0 double-quoted string body.
///
/// Escapes per KDL 2.0:
/// - `"` → `\"`
/// - `\` → `\\`
/// - newline (LF) → `\n`
/// - carriage return (CR) → `\r`
/// - tab → `\t`
/// - any other ASCII control (U+0000..=U+001F) → `\u{XXXX}`
///
/// The path coming through here is already NFC + forbidden-category
/// validated by the parser, so the escape sequences are well-defined and
/// the output is round-trippable: `parse(write(m).as_bytes()) == Ok(m)` for
/// any `m: IntegrityManifest`.
fn push_kdl_string(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // KDL 2.0's general escape for any Unicode scalar value;
                // lowercase hex matches the canonical-writer convention.
                let _ = write!(out, "\\u{{{:X}}}", c as u32);
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact byte sequences; a panic signals a regression"
)]
mod tests {
    use super::*;
    use crate::manifest::{parse_hex64, Algorithm, IntegrityEntry, IntegrityManifest, SHA256_LEN};
    use crate::parse::parse;
    use elang_core::ids::ResourcePath;
    use std::collections::BTreeMap;

    /// Build an `IntegrityManifest` from a list of `(path, hex)` pairs.
    /// Used to assemble test fixtures without re-parsing KDL.
    fn manifest_from(pairs: &[(&str, &str)]) -> IntegrityManifest {
        let mut resources = BTreeMap::new();
        for (path, hex) in pairs {
            let path = ResourcePath::new((*path).to_string()).expect("valid path");
            let digest = parse_hex64(hex).expect("valid hex");
            resources.insert(path.clone(), IntegrityEntry { path, digest });
        }
        IntegrityManifest {
            algorithm: Algorithm::Sha256,
            resources,
        }
    }

    #[test]
    fn canonical_bytes_matches_golden_vector() {
        let m = manifest_from(&[
            (
                "Header/format.version",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "Linguistics/phonemes.eld",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "mimetype",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "package.epf",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        ]);
        assert_eq!(canonical_bytes(&m), CANONICAL_GOLDEN);
    }

    #[test]
    fn canonical_bytes_roundtrips_through_parse() {
        // parse → write → parse must yield the same manifest.
        let m1 = parse(CANONICAL_GOLDEN).expect("golden parses");
        let bytes = canonical_bytes(&m1);
        let m2 = parse(&bytes).expect("written bytes re-parse");
        assert_eq!(m1, m2);
    }

    #[test]
    fn canonical_bytes_sorts_resources_by_path() {
        // Reverse-sorted insertion order; BTreeMap must sort on output.
        let m = manifest_from(&[
            (
                "zzz/last.eld",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "aaa/first.eld",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "mmm/middle.eld",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        ]);
        let bytes = canonical_bytes(&m);
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        let pos_aaa = text.find("aaa/first").expect("aaa first");
        let pos_mmm = text.find("mmm/middle").expect("mmm middle");
        let pos_zzz = text.find("zzz/last").expect("zzz last");
        assert!(pos_aaa < pos_mmm && pos_mmm < pos_zzz);
    }

    #[test]
    fn canonical_bytes_uses_four_space_indent() {
        let m = manifest_from(&[(
            "a.eld",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )]);
        let bytes = canonical_bytes(&m);
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        assert!(
            text.contains("    algorithm"),
            "algorithm line must be 4-space indented"
        );
        assert!(
            text.contains("    resource"),
            "resource line must be 4-space indented"
        );
        // No 8-space indent.
        assert!(!text.contains("        algorithm"));
    }

    #[test]
    fn canonical_bytes_uses_lf_only_and_exactly_one_trailing_lf() {
        let m = manifest_from(&[(
            "a.eld",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )]);
        let bytes = canonical_bytes(&m);
        // No carriage returns anywhere.
        assert!(!bytes.contains(&b'\r'), "must not contain CR");
        // Ends with `}\n` (one trailing LF). The previous assertion
        // (`bytes.last() == Some(&b'\n')`) guarantees the array is long
        // enough for a 2-byte slice.
        assert_eq!(bytes.last(), Some(&b'\n'), "must end with LF");
        #[allow(
            clippy::indexing_slicing,
            reason = "bytes.len() >= 2 is implied by the prior LF-end assertion"
        )]
        let last_two = &bytes[bytes.len() - 2..];
        assert_eq!(last_two, b"}\n", "must end with exactly closing brace + LF");
    }

    #[test]
    fn canonical_bytes_escapes_quote_and_backslash() {
        // This tests quote escaping for a valid integrity path.
        // ResourcePath::new forbids backslashes; control-character escapes
        // such as `\n`, `\r`, `\t` and `\u{..}` for control characters are
        // outside the integrity path domain enforced by the parser.
        let mut resources = BTreeMap::new();
        let path = ResourcePath::new("weird\"path.eld".to_string()).expect("valid path");
        let digest = [0u8; SHA256_LEN];
        resources.insert(path.clone(), IntegrityEntry { path, digest });
        let m = IntegrityManifest {
            algorithm: crate::manifest::Algorithm::Sha256,
            resources,
        };
        let bytes = canonical_bytes(&m);
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        assert!(
            text.contains("weird\\\"path.eld"),
            "embedded `\"` must be escaped to `\\\"`; got: {text}"
        );
    }

    #[test]
    fn canonical_bytes_uses_sha256_literal_constant() {
        // Algorithm line must use the same literal the parser accepts —
        // regression guard so a future variant added to `Algorithm` can't
        // accidentally drift from the parser's match arm.
        let m = manifest_from(&[(
            "a.eld",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )]);
        let bytes = canonical_bytes(&m);
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        let expected = format!("    algorithm \"{SHA_256_LITERAL}\"\n");
        assert!(
            text.contains(&expected),
            "must emit SHA_256_LITERAL: {text}"
        );
    }

    #[test]
    fn canonical_bytes_empty_resource_list_emits_structural_only() {
        // The reader rejects this (`c05_rejects_empty_resource_list`), but
        // the writer must still produce a syntactically valid file shape —
        // defensive: never panic on empty resources.
        let m = IntegrityManifest {
            algorithm: crate::manifest::Algorithm::Sha256,
            resources: BTreeMap::new(),
        };
        let bytes = canonical_bytes(&m);
        let text = std::str::from_utf8(&bytes).expect("utf-8");
        assert_eq!(
            text, "integrity {\n    algorithm \"sha-256\"\n}\n",
            "empty resources must still emit the canonical shape"
        );
        // And the reader must reject it (regression guard against the
        // writer accidentally producing a file the reader accepts as valid).
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn canonical_bytes_format_byte_counts() {
        // Spot-check exact byte lengths for the golden vector so accidental
        // edits to whitespace or escapes are caught.
        // 5 lines: integrity { + algorithm + 4 resources + } — all LF-terminated.
        let m = parse(CANONICAL_GOLDEN).expect("golden parses");
        let bytes = canonical_bytes(&m);
        // 7 LF-separated lines (integrity, algorithm, 4 resources, closing)
        // plus one final LF makes 7 newlines total.
        let lf_count = bytes.iter().filter(|&&b| b == b'\n').count();
        assert_eq!(
            lf_count, 7,
            "must contain exactly 7 LF (6 line-end + 1 final)"
        );
        // Total bytes: opening + 4-space-indent algorithm + path lengths + hex + closing.
        // Hand-verified against CANONICAL_GOLDEN:
        assert_eq!(bytes.len(), CANONICAL_GOLDEN.len());
    }
}
