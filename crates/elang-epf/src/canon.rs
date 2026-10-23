//! Canonical `package.epf` serializer (T0616, T0617).
//!
//! Implements the profile ratified by
//! [`specs/canonical/epf-canonical-serialization.md`](../../specs/canonical/epf-canonical-serialization.md)
//! and [ADR 0005](../../docs/decisions/0005-canonical-epf-serialization.md):
//!
//! 1. Top-level node order: `epf`, `eld`, `language-version`, `metadata`,
//!    `manifest`.
//! 2. `metadata` children: `identifier`, `name`, then `description` (only if
//!    present).
//! 3. `manifest` children: `phonemes` (the sole permitted entry).
//! 4. Indent: exactly 4 ASCII spaces per nesting level; no tabs.
//! 5. Layout: one declaration per line; no blank lines; no comments; LF line
//!    endings; exactly one trailing LF.
//! 6. Strings: standard double-quoted, escaping embedded `"` and `\`, with
//!    validated NFC Unicode emitted directly as UTF-8 and no silent
//!    normalization.
//! 7. No properties and no extra positional arguments are emitted.
//!
//! The output is byte-deterministic: `to_string(pkg)` depends only on the
//! validated semantic model, never on how the source was formatted.

use elang_core::model::epf as model;
use elang_kdl::escape::encode_string;

/// Serialize a validated package into canonical `package.epf` bytes.
///
/// The result always ends with exactly one LF and contains no trailing
/// whitespace. Serializing the same model twice yields identical bytes.
#[must_use]
pub fn to_string(pkg: &model::EpfPackage) -> String {
    let mut out = String::with_capacity(256);

    // 1. Top-level order: epf, eld, language-version, metadata, manifest.
    push_line(&mut out, 0, &format!("epf {}", quote(&pkg.epf_version().to_string())));
    push_line(&mut out, 0, &format!("eld {}", quote(&pkg.eld_version().to_string())));
    push_line(
        &mut out,
        0,
        &format!("language-version {}", quote(&pkg.language_version().to_string())),
    );

    // 2. metadata block.
    push_line(&mut out, 0, "metadata {");
    let metadata = pkg.metadata();
    push_line(
        &mut out,
        1,
        &format!("identifier {}", quote(metadata.identifier().as_str())),
    );
    push_line(&mut out, 1, &format!("name {}", quote(metadata.name())));
    // `description` is emitted only when actually present in the model. A
    // present-but-empty description round-trips as `description ""`.
    if let Some(description) = metadata.description() {
        push_line(&mut out, 1, &format!("description {}", quote(description)));
    }
    push_line(&mut out, 0, "}");

    // 3. manifest block: sole entry `phonemes`.
    push_line(&mut out, 0, "manifest {");
    push_line(
        &mut out,
        1,
        &format!("phonemes {}", quote(pkg.manifest().phonemes().as_str())),
    );
    push_line(&mut out, 0, "}");

    out
}

/// Serialize to owned bytes. Equivalent to `to_string(pkg).into_bytes()` but
/// avoids an intermediate `String` reallocation for callers writing straight
/// into an output buffer.
#[must_use]
pub fn to_bytes(pkg: &model::EpfPackage) -> Vec<u8> {
    to_string(pkg).into_bytes()
}

/// Append one 4-space-per-level indented line followed by a single LF.
///
/// `depth` is the nesting level: `0` is top level, `1` is a block child.
fn push_line(out: &mut String, depth: usize, text: &str) {
    // 4 spaces per level. `depth` is a small compile-time-controlled constant
    // at every call site, so a saturating multiply keeps the lint policy
    // (`arithmetic_side_effects`) satisfied without an `expect`.
    let indent = 4usize.saturating_mul(depth);
    for _ in 0..indent {
        out.push(' ');
    }
    out.push_str(text);
    out.push('\n');
}

/// Quote a value as a canonical KDL 2.0 double-quoted string.
fn quote(s: &str) -> String {
    encode_string(s)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "serializer tests assert exact golden bytes; a panic signals a canonicalization regression"
)]
mod tests {
    use super::*;
    use crate::parse;

    /// The golden document from the canonical spec (section "Canonical
    /// example"). Byte-for-byte identical, including the single trailing LF.
    const GOLDEN: &str = concat!(
        "epf \"0.1.0\"\n",
        "eld \"0.1.0\"\n",
        "language-version \"1.0.0\"\n",
        "metadata {\n",
        "    identifier \"urn:elang:language:rocky:aelith:1.0.0\"\n",
        "    name \"Aelith\"\n",
        "    description \"A small constructed language.\"\n",
        "}\n",
        "manifest {\n",
        "    phonemes \"Linguistics/phonemes.eld\"\n",
        "}\n",
    );

    fn model_from(src: &str) -> model::EpfPackage {
        parse(src.as_bytes()).expect("valid EPF")
    }

    #[test]
    fn matches_the_golden_vector() {
        let pkg = model_from(GOLDEN);
        assert_eq!(to_string(&pkg), GOLDEN);
    }

    #[test]
    fn roundtrips_through_the_validator() {
        let pkg = model_from(GOLDEN);
        let canonical = to_string(&pkg);
        let reparsed = parse(canonical.as_bytes()).expect("canonical output must reparse");
        assert_eq!(reparsed, pkg, "canonical form must preserve the model");
    }

    #[test]
    fn normalizes_source_formatting_and_order() {
        // Same model, wild source formatting: reordered, comments, single-line
        // blocks, extra whitespace. All must canonicalize identically.
        let messy = "\
/* header comment */
metadata { name \"Aelith\"; identifier \"urn:elang:language:rocky:aelith:1.0.0\"; description \"A small constructed language.\" }
epf \"0.1.0\"
manifest { phonemes \"Linguistics/phonemes.eld\" }  // trailing comment
eld \"0.1.0\"
language-version \"1.0.0\"
";
        let pkg = model_from(messy);
        assert_eq!(to_string(&pkg), GOLDEN);
    }

    #[test]
    fn omits_description_when_absent() {
        let src = "\
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
metadata {
    identifier \"urn:elang:language:rocky:aelith:1.0.0\"
    name \"Aelith\"
}
manifest { phonemes \"Linguistics/phonemes.eld\" }
";
        let out = to_string(&model_from(src));
        assert!(!out.contains("description"));
        assert!(out.ends_with("}\n"), "exactly one trailing LF");
    }

    #[test]
    fn preserves_present_but_empty_description() {
        let src = "\
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
metadata {
    identifier \"urn:elang:language:rocky:aelith:1.0.0\"
    name \"Aelith\"
    description \"\"
}
manifest { phonemes \"Linguistics/phonemes.eld\" }
";
        let out = to_string(&model_from(src));
        assert!(out.contains("description \"\""));
    }

    #[test]
    fn escapes_quotes_and_backslashes() {
        let src = "\
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
metadata {
    identifier \"urn:elang:language:rocky:aelith:1.0.0\"
    name \"He said \\\"hi\\\" \\\\ bye\"
}
manifest { phonemes \"Linguistics/phonemes.eld\" }
";
        let pkg = model_from(src);
        let out = to_string(&pkg);
        // The emitted string re-parses to the same value and is byte-stable.
        let reparsed = model_from(&out);
        assert_eq!(reparsed, pkg);
        let again = to_string(&reparsed);
        assert_eq!(again, out, "serialization is idempotent");
    }

    #[test]
    fn uses_exactly_four_space_indent_and_lf_only() {
        let out = to_string(&model_from(GOLDEN));
        assert!(out.contains("\n    identifier"), "children indented by 4");
        assert!(!out.contains('\t'), "no tabs");
        assert!(!out.contains("\r"), "no CR");
        assert!(!out.contains("\n\n"), "no blank lines");
    }

    #[test]
    fn to_bytes_matches_to_string() {
        let pkg = model_from(GOLDEN);
        assert_eq!(to_bytes(&pkg), to_string(&pkg).into_bytes());
    }
}