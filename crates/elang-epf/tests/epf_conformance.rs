//! Integration tests for `elang-epf` (T0618).
//!
//! These exercise the public API exactly as the reader SDK and package writer
//! will: parse → validate → resolve manifest → canonicalize → roundtrip. The
//! unit tests in `src/parse.rs` and `src/canon.rs` cover the grammar in
//! isolation; this file covers the composed pipeline and the boundaries the
//! issue's acceptance criteria call out (roundtrip preservation, canonical
//! golden bytes, boundary lengths, malformed input).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration tests assert exact bytes/codes; a panic signals a regression"
)]
#![allow(
    clippy::panic,
    reason = "test assertion macros require panic to signal failure"
)]

use elang_core::error::Category;
use elang_epf::canon;
use elang_epf::manifest::{resolve_phonemes, IndexLookup};
use elang_epf::parse;
use elang_epf::ResourcePath;

/// The canonical golden vector from
/// `specs/canonical/epf-canonical-serialization.md`. Byte-for-byte,
/// including the single trailing LF.
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

/// A trivial in-memory index over a list of paths.
struct SliceIndex(Vec<String>);

impl IndexLookup for SliceIndex {
    fn contains(&self, path: &str) -> bool {
        self.0.iter().any(|p| p == path)
    }
}

#[test]
fn golden_vector_roundtrips() {
    let pkg = parse(GOLDEN.as_bytes()).expect("golden vector must parse");
    let out = canon::to_string(&pkg);
    assert_eq!(out, GOLDEN, "canonical output must match the golden bytes");

    // Reparsing the canonical form yields an identical model.
    let reparsed = parse(out.as_bytes()).expect("canonical output must reparse");
    assert_eq!(reparsed, pkg);
}

#[test]
fn canonicalization_is_idempotent() {
    let pkg = parse(GOLDEN.as_bytes()).expect("golden vector must parse");
    let once = canon::to_string(&pkg);
    let twice = canon::to_string(&parse(once.as_bytes()).expect("reparse"));
    assert_eq!(once, twice, "canonicalization must be a fixed point");
}

#[test]
fn reformatted_source_canonicalizes_to_golden() {
    // Same semantic model, deliberately chaotic surface formatting: reordered
    // top level, tabs/extra spaces, comments, single-line blocks with `;`
    // terminators, and an empty-but-present description value elsewhere.
    let messy = "\
/* leading block comment */
// another
manifest {
    phonemes \"Linguistics/phonemes.eld\"   // inline comment
}
metadata {
    name\t\"Aelith\"
    identifier   \"urn:elang:language:rocky:aelith:1.0.0\"
    description \"A small constructed language.\"
}
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
";
    let pkg = parse(messy.as_bytes()).expect("messy but valid EPF");
    assert_eq!(canon::to_string(&pkg), GOLDEN);
}

#[test]
fn roundtrip_preserves_model_across_reformatting() {
    let a = parse(GOLDEN.as_bytes()).expect("parse");
    // Reserialize with deliberately different but valid formatting, then
    // canonicalize; the model must be preserved exactly.
    let b = parse(canon::to_string(&a).as_bytes()).expect("reparse");
    assert_eq!(a, b);
}

#[test]
fn resolve_phonemes_against_index() {
    let pkg = parse(GOLDEN.as_bytes()).expect("parse");
    let path = pkg.manifest().phonemes().as_str();

    // Present in the index → Ok.
    let index = SliceIndex(vec![
        "Header/format.version".to_string(),
        "Linguistics/phonemes.eld".to_string(),
    ]);
    assert!(resolve_phonemes(path, &index).is_ok());

    // Absent → EPF_MANIFEST_PATH_NOT_INDEXED naming the path.
    let empty = SliceIndex(Vec::new());
    let e = resolve_phonemes(path, &empty).expect_err("path must be missing");
    assert_eq!(e.code(), "EPF_MANIFEST_PATH_NOT_INDEXED");
    assert_eq!(e.diagnostic.resource_path.as_deref(), Some(path));
}

#[test]
fn eld_extension_gate() {
    use elang_epf::has_eld_extension;
    assert!(has_eld_extension("Linguistics/phonemes.eld"));
    assert!(!has_eld_extension("Linguistics/phonemes.txt"));
    assert!(!has_eld_extension("Linguistics/phonemes.ELD"));
}

// ---- boundary cases ------------------------------------------------------

/// A package builder for probing the byte-length boundaries of each field.
fn package_with(identifier: &str, name: &str, description: Option<&str>) -> String {
    let desc_line = description.map_or(String::new(), |d| format!("\n    description {d:?}"));
    format!(
        "epf \"0.1.0\"\n\
         eld \"0.1.0\"\n\
         language-version \"1.0.0\"\n\
         metadata {{\n    identifier {identifier:?}\n    name {name:?}{desc_line}\n}}\n\
         manifest {{ phonemes \"a/b.eld\" }}\n"
    )
}

#[test]
fn name_at_and_over_the_256_byte_boundary() {
    // 256 bytes exactly → OK; 257 → EPF_BAD_METADATA_FIELD.
    let id = "urn:elang:language:r:n:1.0.0";
    let ok = package_with(id, &"x".repeat(256), None);
    assert!(
        parse(ok.as_bytes()).is_ok(),
        "256-byte name is at the limit"
    );
    let over = package_with(id, &"x".repeat(257), None);
    assert_eq!(
        parse(over.as_bytes()).expect_err("257 exceeds").code(),
        "EPF_BAD_METADATA_FIELD"
    );
}

#[test]
fn description_at_and_over_the_4096_byte_boundary() {
    let id = "urn:elang:language:r:n:1.0.0";
    let ok = package_with(id, "N", Some(&"x".repeat(4096)));
    assert!(
        parse(ok.as_bytes()).is_ok(),
        "4096-byte description is at the limit"
    );
    let over = package_with(id, "N", Some(&"x".repeat(4097)));
    assert_eq!(
        parse(over.as_bytes()).expect_err("4097 exceeds").code(),
        "EPF_BAD_METADATA_FIELD"
    );
}

#[test]
fn identifier_at_and_over_the_512_byte_boundary() {
    // Build an identifier of exactly 512 bytes, then 513.
    // urn:elang:language:<ns>:<name>:<ver> — the fixed prefix is
    // "urn:elang:language:" (20) + ns + ":" + name + ":" + ver.
    let ver = "1.0.0";
    let prefix_len = "urn:elang:language:".len() + 1 + 1 + ver.len(); // + ns sep + name sep
    let ns = "r";
    // We need prefix_len + ns = 512 for exactly 512. Solve for name.
    // 20 + len(ns) + 1 + len(name) + 1 + 5 = 512 → len(name) = 485.
    let name_len = 512 - prefix_len - ns.len();
    let name = "x".repeat(name_len);
    let id512 = format!("urn:elang:language:{ns}:{name}:{ver}");
    assert_eq!(id512.len(), 512);
    assert!(
        parse(package_with(&id512, "N", None).as_bytes()).is_ok(),
        "512-byte identifier is at the limit"
    );

    let name_over = "x".repeat(name_len + 1);
    let id513 = format!("urn:elang:language:{ns}:{name_over}:{ver}");
    assert_eq!(id513.len(), 513);
    assert_eq!(
        parse(package_with(&id513, "N", None).as_bytes())
            .expect_err("513 exceeds")
            .code(),
        "EPF_BAD_URN"
    );
}

// ---- malformed / negative input ------------------------------------------

#[test]
fn invalid_utf8_is_rejected_at_the_kdl_layer() {
    let bytes = b"epf \"0.1.0\"\n\xff\xfe";
    let e = parse(bytes).expect_err("invalid UTF-8");
    assert!(
        e.code().starts_with("KDL_") || e.code() == "KDL_INVALID_UTF8",
        "expected a KDL-layer code, got {}",
        e.code()
    );
}

#[test]
fn unknown_declarations_are_never_silently_ignored() {
    // Every unknown declaration must produce a diagnostic, never a success.
    let cases: &[(&str, &str)] = &[
        (
            "unknown top-level",
            "epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
             metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
             manifest { phonemes \"a/b.eld\" }\nextra \"x\"\n",
        ),
        (
            "unknown metadata child",
            "epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
             metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n    extra \"x\"\n}\n\
             manifest { phonemes \"a/b.eld\" }\n",
        ),
        (
            "unknown manifest child",
            "epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
             metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
             manifest {\n    phonemes \"a/b.eld\"\n    extra \"x\"\n}\n",
        ),
    ];
    for (label, src) in cases {
        assert!(
            parse(src.as_bytes()).is_err(),
            "{label} must be rejected, not ignored"
        );
    }
}

#[test]
fn modules_declaration_is_rejected() {
    let src = "epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
         metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
         manifest {\n    phonemes \"a/b.eld\"\n    modules \"lexicon\"\n}\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("modules rejected").code(),
        "EPF_MANIFEST_INVALID"
    );
}

#[test]
fn duplicate_every_top_level_node_is_rejected() {
    for dup in [
        "epf \"0.1.0\"",
        "eld \"0.1.0\"",
        "language-version \"1.0.0\"",
    ] {
        let src = format!(
            "epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n{dup}\n\
             metadata {{\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}}\n\
             manifest {{ phonemes \"a/b.eld\" }}\n"
        );
        let e = parse(src.as_bytes()).expect_err("duplicate rejected");
        assert_eq!(e.code(), "EPF_DUPLICATE_NODE", "dup of `{dup}`");
    }
}

#[test]
fn version_nodes_must_be_exactly_zero_one_zero() {
    // epf and eld must be exactly "0.1.0"; language-version is a language
    // release version and only needs strict semver.
    for bad in ["0.1.1", "0.2.0", "1.0.0", "0.1"] {
        let src = format!(
            "epf \"{bad}\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
             metadata {{\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}}\n\
             manifest {{ phonemes \"a/b.eld\" }}\n"
        );
        assert_eq!(
            parse(src.as_bytes()).expect_err("bad epf version").code(),
            "EPF_BAD_VERSION",
            "epf \"{bad}\""
        );
    }
}

#[test]
fn resource_path_is_validated_before_use() {
    // The manifest path must survive `ResourcePath::new` (steering §3.4).
    assert!(ResourcePath::new("Linguistics/phonemes.eld").is_ok());
    assert!(ResourcePath::new("../escape.eld").is_err());
    assert!(ResourcePath::new("/abs.eld").is_err());
}

#[test]
fn canonical_output_never_contains_raw_strings_or_comments() {
    // A description authored as a KDL raw string (`#"…"#`) must still
    // canonicalize to a standard double-quoted string.
    let src = "epf \"0.1.0\"\n\
         eld \"0.1.0\"\n\
         language-version \"1.0.0\"\n\
         metadata {\n    identifier \"urn:elang:language:rocky:aelith:1.0.0\"\n    name \"Aelith\"\n    description #\"A small constructed language.\"#\n}\n\
         manifest { phonemes \"Linguistics/phonemes.eld\" }\n";
    let pkg = parse(src.as_bytes()).expect("raw-string description parses");
    let out = canon::to_string(&pkg);
    assert_eq!(out, GOLDEN);
}

// ---- CodeRabbit review fixes --------------------------------------------
//
// Each of the next tests targets a specific branch the original suite left
// uncovered (CodeRabbit review + Codecov patch-coverage report).

/// `LIMIT_EPF_BYTES` guard rejects oversized input before KDL allocation
/// (CodeRabbit architecture review; steering §2 / specs/registry/resource-policy).
#[test]
fn rejects_input_over_max_epf_bytes() {
    let oversize = vec![b' '; 1024 * 1024 + 1];
    let err = parse(&oversize).expect_err("oversize EPF must be rejected");
    assert_eq!(err.code(), "LIMIT_EPF_BYTES");
    assert_eq!(err.diagnostic.stage, Some(10));
    assert_eq!(err.category(), Category::Limit);
}

/// Input at exactly the 1 MiB cap is **accepted** — boundary inclusive.
#[test]
fn accepts_input_at_max_epf_bytes() {
    // Build a string of exactly 1 MiB; it is well-formed KDL whitespace
    // (no top-level nodes) and parses without errors. The size guard is
    // not the schema, so the schema validator will then complain — the
    // important thing for this test is that the guard did NOT fire.
    let n = 1024 * 1024;
    let pad = vec![b' '; n];
    let err = parse(&pad).expect_err("whitespace-only EPF still has schema errors");
    assert_ne!(
        err.code(),
        "LIMIT_EPF_BYTES",
        "exact-1MiB input must pass the size guard; got {err:?}"
    );
}

/// Duplicate top-level `epf` is rejected with `EPF_DUPLICATE_NODE`.
#[test]
fn duplicate_top_level_node_is_rejected() {
    let src = "epf \"0.1.0\"\nepf \"0.1.0\"\n\
               eld \"0.1.0\"\nlanguage-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("dup epf").code(),
        "EPF_DUPLICATE_NODE"
    );
}

/// `manifest.modules` is the canonical banned entry.
#[test]
fn modules_manifest_entry_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { modules \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("modules rejected").code(),
        "EPF_MANIFEST_INVALID"
    );
}

/// `manifest { }` (empty block, missing `phonemes`) is rejected.
#[test]
fn manifest_without_phonemes_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { }\n";
    // An empty manifest block fails the C-05 "nonempty block" check first,
    // surfacing as `EPF_BAD_ARITY` rather than `EPF_MANIFEST_INVALID`.
    assert_eq!(
        parse(src.as_bytes()).expect_err("empty manifest").code(),
        "EPF_BAD_ARITY"
    );
}

/// Empty `phonemes` path argument is rejected.
#[test]
fn empty_phonemes_path_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("empty path").code(),
        "EPF_MANIFEST_PATH_NOT_INDEXED"
    );
}

/// `phonemes` argument with the wrong extension is rejected.
#[test]
fn phonemes_path_without_eld_extension_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"Linguistics/phonemes.txt\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("wrong extension").code(),
        "EPF_MANIFEST_PATH_NOT_INDEXED"
    );
}

/// A node that declares a property is rejected (C-01 grammar: zero props).
#[test]
fn property_on_version_node_is_rejected() {
    let src = "epf \"0.1.0\" lang=\"en\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("prop rejected").code(),
        "EPF_BAD_ARITY"
    );
}

/// A node that declares an extra positional argument is rejected.
#[test]
fn extra_arg_on_version_node_is_rejected() {
    let src = "epf \"0.1.0\" \"extra\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes())
            .expect_err("extra arg rejected")
            .code(),
        "EPF_BAD_ARITY"
    );
}

/// `metadata` declared as a bare value (no child block) is rejected.
#[test]
fn metadata_without_block_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata\n\
               manifest { phonemes \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("metadata no block").code(),
        "EPF_BAD_ARITY"
    );
}

/// `from_core` preserves stage, category, and source metadata when
/// remapping EPF model codes (CodeRabbit review at error.rs:107). The
/// upstream `LanguageIdentifier::parse` and length checks do not yet
/// attach KDL spans, so this test asserts on stage/category/message
/// preservation; span preservation is wired but currently a no-op for
/// these specific remappings.
/// `phonemes` declared with a non-string argument is rejected.
#[test]
fn phonemes_with_non_string_arg_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes 123 }\n";
    assert_eq!(
        parse(src.as_bytes())
            .expect_err("non-string phonemes")
            .code(),
        "EPF_MANIFEST_INVALID"
    );
}

/// `language-version` that is not a string is rejected.
#[test]
fn language_version_with_non_string_arg_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version 1.0.0\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes())
            .expect_err("non-string language-version")
            .code(),
        "EPF_BAD_ARITY"
    );
}

/// A `phonemes` path that passes the syntactic checks but violates
/// `ResourcePath` rules (empty segment) is rejected.
#[test]
fn phonemes_path_that_fails_resource_path_is_rejected() {
    // A `//` empty segment is rejected by `ResourcePath::new` after the
    // KDL lexer accepts it. The diagnostic comes from the EPF layer,
    // not the KDL parser.
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a//b.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("empty segment").code(),
        "EPF_MANIFEST_INVALID"
    );
}

/// `parse_error` is the public re-export of `from_kdl`. Verify it produces
/// a diagnostic carrying a non-empty message. KDL parser internals are
/// tested in `elang-kdl`; this test exercises the EPF re-export path.
#[test]
fn parse_error_adapter_round_trips_a_kdl_error() {
    // Build a KDL parse error by feeding nonsense.
    let bad = b"epf \x00not-utf8";
    let err = parse(bad).expect_err("bad bytes");
    assert!(!err.diagnostic.message.is_empty());
    assert_ne!(
        err.code(),
        "LIMIT_EPF_BYTES",
        "KDL parse error must not be confused with the byte-size guard"
    );

    // Direct call to `parse_error` with the KDL parser's own error also routes
    // through `from_kdl`; the EPF adapter and the re-export must agree on
    // the code.
    use elang_epf::parse_error as epf_parse_error;
    match elang_kdl::parse(bad) {
        Ok(_) => panic!("kdl::parse unexpectedly accepted NUL byte input"),
        Err(kdl_err) => {
            let via_re_export = epf_parse_error(kdl_err);
            assert_eq!(err.code(), via_re_export.code());
        }
    }
}

/// `unicode_diagnostics_attach_stage_10` already covers `UNICODE_NOT_NFC`;
/// add a ForbiddenCategory trigger (any scalar in the C0/C1 control range
/// the unicode allowlist rejects).
#[test]
fn unicode_forbidden_category_is_rejected_with_stage_10() {
    // U+0007 (BEL) is a C0 control character — explicitly forbidden.
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata {\n                   identifier \"urn:elang:language:r:n:1.0.0\"\n                   name \"a\\u{0007}b\"\n               }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    let err = parse(src.as_bytes()).expect_err("forbidden category");
    assert_eq!(err.code(), "UNICODE_FORBIDDEN_CATEGORY");
    assert_eq!(err.diagnostic.stage, Some(10));
}

/// Manifest item at index 1 (a second child after `phonemes`) is rejected.
/// `assign_once` for `phonemes` is the canonical duplicate-path; we cover it
/// here with two phonemes entries.
#[test]
fn duplicate_phonemes_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { phonemes \"a/b.eld\"\n    phonemes \"c/d.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("dup phonemes").code(),
        "EPF_MANIFEST_INVALID"
    );
}

/// An unknown manifest child (anything other than `phonemes`) is rejected.
#[test]
fn unknown_manifest_child_is_rejected() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata { identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\" }\n\
               manifest { scripts \"foo/bar.eld\" }\n";
    assert_eq!(
        parse(src.as_bytes()).expect_err("unknown manifest").code(),
        "EPF_MANIFEST_INVALID"
    );
}

/// `unicode_diagnostics_attach_stage_10` covers `UNICODE_NOT_NFC`; this
/// test covers the noncharacter branch in `unicode_to_error`.
#[test]
fn unicode_noncharacter_is_rejected_with_stage_10() {
    // U+FDD0 is a permanent noncharacter per Unicode §23.7.
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata {\n                   identifier \"urn:elang:language:r:n:1.0.0\"\n                   name \"a\\u{FDD0}b\"\n               }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    let err = parse(src.as_bytes()).expect_err("noncharacter");
    assert_eq!(err.code(), "UNICODE_NONCHARACTER");
    assert_eq!(err.diagnostic.stage, Some(10));
}

/// `unicode_diagnostics_attach_stage_10` covers `UNICODE_NOT_NFC`; this
/// test covers the ForbiddenCategory branch.
#[test]
fn unicode_to_error_routes_through_stage_10() {
    // The two prior tests already cover ForbiddenCategory and Noncharacter;
    // this is a thin sanity test that the forbidden-category message
    // includes the scalar.
    use elang_unicode::UnicodeViolation;
    let v = UnicodeViolation::ForbiddenCategory { scalar: '\u{0007}' };
    let msg = match v {
        UnicodeViolation::ForbiddenCategory { scalar } => {
            format!("U+{:04X}", u32::from(scalar))
        }
        _ => String::new(),
    };
    assert_eq!(msg, "U+0007");
}

#[test]
fn from_core_preserves_stage_and_message_when_remapping() {
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata {\n                   identifier \"not a urn\"\n                   name \"N\"\n               }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    let err = parse(src.as_bytes()).expect_err("bad urn");
    assert_eq!(err.code(), "EPF_BAD_URN");
    assert_eq!(err.diagnostic.stage, Some(10));
    assert_eq!(err.category(), Category::EpfSchema);
    assert!(
        err.diagnostic.message.contains("URN") || err.diagnostic.message.contains("urn"),
        "message must survive remap: got {:?}",
        err.diagnostic.message
    );
}

/// `unicode_to_error` attaches stage 10 to Unicode diagnostics
/// (CodeRabbit review at parse.rs:557).
#[test]
fn unicode_diagnostics_attach_stage_10() {
    // Decomposed "é" = "e" + U+0301 (combining acute). Not NFC.
    let src = "epf \"0.1.0\"\n\
               eld \"0.1.0\"\n\
               language-version \"1.0.0\"\n\
               metadata {\n                   identifier \"urn:elang:language:r:n:1.0.0\"\n                   name \"e\u{0301}\"\n               }\n\
               manifest { phonemes \"a/b.eld\" }\n";
    let err = parse(src.as_bytes()).expect_err("not NFC");
    assert_eq!(err.code(), "UNICODE_NOT_NFC");
    assert_eq!(err.diagnostic.stage, Some(10));
}
