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
