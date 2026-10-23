//! `META-INF/integrity.kdl` v0.1.0 schema validator (T0702, T0703).
//!
//! The validator runs over a `elang_kdl::ast::KdlDocument` produced by the
//! generic parser and enforces every constraint labelled `[C-nn]` in
//! [`specs/grammar/integrity-0.1.0.md`](../../../specs/grammar/integrity-0.1.0.md).
//!
//! | Constraint | Code |
//! |---|---|
//! | C-01 wrong top-level node name / extra top-level node / args / properties / missing or empty block | `INTEGRITY_STRUCTURE_INVALID` |
//! | C-01 duplicate top-level `integrity` node | `INTEGRITY_STRUCTURE_INVALID` |
//! | C-02 `algorithm` ≠ `"sha-256"` / duplicate / property / block | `INTEGRITY_BAD_ALGORITHM` |
//! | C-03 `resource` path malformed / arg missing / extra arg | `INTEGRITY_STRUCTURE_INVALID` |
//! | C-03 `sha256` property missing / extra property / wrong shape / wrong hex | `INTEGRITY_BAD_HEX` or `INTEGRITY_STRUCTURE_INVALID` |
//! | C-03 `resource` carries a block | `INTEGRITY_STRUCTURE_INVALID` |
//! | C-05 duplicate `resource` path | `INTEGRITY_DUPLICATE_ENTRY` |
//! | C-05 unknown child of `integrity` / unknown `resource` arg or property | `INTEGRITY_STRUCTURE_INVALID` |
//!
//! NFC / forbidden-category violations on `resource` path args are routed
//! through the `UNICODE` category so they surface in stage-9 diagnostics
//! alongside the schema codes. Constraint C-04 (coverage against the index)
//! is enforced separately by [`crate::manifest::verify_coverage`].

use elang_core::error::{Category, Error};
use elang_core::ids::ResourcePath;
use elang_core::limits::MAX_INTEGRITY_BYTES;
use elang_kdl::ast::{KdlDocument, KdlEntry};
use elang_kdl::budget::BudgetTracker;
use elang_kdl::parse_with_budgets as kdl_parse_with_budgets;
use elang_kdl::KdlValue;
use elang_unicode::UnicodeViolation;

use crate::error::{from_kdl, integrity_err, integrity_err_at};
use crate::manifest::{parse_hex64, Algorithm, IntegrityEntry, IntegrityManifest, SHA_256_LITERAL};

/// Build a `LIMIT` diagnostic pointing at the integrity input size cap
/// (`Category::Limit` with the stage-9 reader pipeline tag).
fn limit_err(code: &'static str, message: impl Into<String>) -> Error {
    Error {
        diagnostic: elang_core::error::Diagnostic::new(code, Category::Limit, message)
            .with_stage(9),
    }
}

/// Parse `input` as `META-INF/integrity.kdl` v0.1.0 and return the validated
/// semantic model.
///
/// Runs every constraint except C-04's index-existence cross-check; the
/// caller is responsible for that via [`crate::manifest::verify_coverage`]
/// once both the manifest and the archive index are in hand.
///
/// Input is rejected with `LIMIT_INTEGRITY_BYTES` before any KDL allocation
/// if it exceeds [`MAX_INTEGRITY_BYTES`] (16 MiB) — this guard exists so a
/// hostile or truncated archive cannot drive the lexer into O(n) work on
/// attacker-controlled bytes (matches the EPF entrypoint's pattern).
///
/// # Errors
///
/// Returns an `INTEGRITY_*` diagnostic for any constraint violation, a KDL
/// parser error for malformed input, or a `UNICODE_*` diagnostic for NFC /
/// forbidden-category violations on `resource` path args.
pub fn parse(input: &[u8]) -> Result<IntegrityManifest, Error> {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
}

/// Parse `input` with a caller-supplied [`BudgetTracker`], useful when
/// multiple resources from the same archive share aggregate budgets.
///
/// The 16 MiB input-size ceiling is enforced unconditionally at the
/// `META-INF/integrity.kdl` entrypoint — KDL-level token/string budgets are
/// not a substitute for an explicit byte-input cap.
pub fn parse_with_budgets(
    input: &[u8],
    budgets: &mut BudgetTracker,
) -> Result<IntegrityManifest, Error> {
    check_input_size(input)?;
    let doc = kdl_parse_with_budgets(input, budgets).map_err(from_kdl)?;
    parse_document(&doc)
}

/// Reject `input` longer than [`MAX_INTEGRITY_BYTES`]. Emitted as a stage-9
/// `LIMIT_INTEGRITY_BYTES` diagnostic so it surfaces in the same channel as
/// every other stage-9 diagnostic.
fn check_input_size(input: &[u8]) -> Result<(), Error> {
    if input.len() as u64 > MAX_INTEGRITY_BYTES {
        return Err(limit_err(
            "LIMIT_INTEGRITY_BYTES",
            format!(
                "integrity.kdl input exceeds {} bytes (got {})",
                MAX_INTEGRITY_BYTES,
                input.len()
            ),
        ));
    }
    Ok(())
}

/// Validate a parsed KDL document against the frozen integrity-manifest
/// grammar.
///
/// This is the schema validator half of the pipeline; the byte-cap half lives
/// in [`parse`] / [`parse_with_budgets`].
///
/// # Errors
///
/// Returns the first constraint violation encountered. The traversal is
/// fail-closed: any unrecognized construct is rejected.
pub fn parse_document(doc: &KdlDocument) -> Result<IntegrityManifest, Error> {
    let nodes = doc.nodes();
    if nodes.len() != 1 {
        return Err(integrity_err(
            "INTEGRITY_STRUCTURE_INVALID",
            format!(
                "integrity.kdl must contain exactly one top-level node, got {}",
                nodes.len()
            ),
        ));
    }
    let integ = nodes.first().ok_or_else(|| {
        integrity_err(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity.kdl has no top-level node",
        )
    })?;
    if integ.name() != "integrity" {
        return Err(integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            format!("top-level node must be 'integrity', got '{}'", integ.name()),
            integ.span(),
        ));
    }
    // C-01: zero args, zero properties on the integrity node.
    if integ.args().next().is_some() {
        return Err(integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity node must have no args",
            integ.span(),
        ));
    }
    if integ.props().next().is_some() {
        return Err(integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity node must have no properties",
            integ.span(),
        ));
    }
    let children = integ.children().ok_or_else(|| {
        integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity node must have a non-empty block",
            integ.span(),
        )
    })?;
    // C-01: mandatory nonempty block. KDL syntactically permits `{}`; reject it.
    if children.is_empty() {
        return Err(integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity node must have a non-empty block",
            integ.span(),
        ));
    }

    let mut algorithm: Option<(Algorithm, elang_kdl::span::SourceSpan)> = None;
    let mut resources: std::collections::BTreeMap<ResourcePath, IntegrityEntry> =
        std::collections::BTreeMap::new();

    for child in children.iter() {
        match child.name() {
            "algorithm" => {
                if algorithm.is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_BAD_ALGORITHM",
                        "algorithm node must appear exactly once",
                        child.span(),
                    ));
                }
                // C-02: no properties, no block.
                if child.props().next().is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "algorithm node must have no properties",
                        child.span(),
                    ));
                }
                if child.children().is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "algorithm node must not have a block",
                        child.span(),
                    ));
                }
                // Exactly one arg, the algorithm literal.
                let mut args = child.args();
                let first = args.next().ok_or_else(|| {
                    integrity_err_at(
                        "INTEGRITY_BAD_ALGORITHM",
                        "algorithm node must have exactly one arg",
                        child.span(),
                    )
                })?;
                if args.next().is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "algorithm node must have exactly one arg",
                        child.span(),
                    ));
                }
                let KdlValue::String { value, .. } = first else {
                    return Err(integrity_err_at(
                        "INTEGRITY_BAD_ALGORITHM",
                        "algorithm arg must be a string literal",
                        span_of(first),
                    ));
                };
                match value.as_ref() {
                    SHA_256_LITERAL => {
                        algorithm = Some((Algorithm::Sha256, child.span()));
                    }
                    other => {
                        return Err(integrity_err_at(
                            "INTEGRITY_BAD_ALGORITHM",
                            format!("algorithm must be \"{SHA_256_LITERAL}\", got \"{other}\""),
                            child.span(),
                        ));
                    }
                }
            }
            "resource" => {
                if child.children().is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource node must not have a block",
                        child.span(),
                    ));
                }
                // Exactly one positional arg (the path).
                let mut args = child.args();
                let first = args.next().ok_or_else(|| {
                    integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource node must have exactly one path arg",
                        child.span(),
                    )
                })?;
                if args.next().is_some() {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource node must have exactly one path arg",
                        child.span(),
                    ));
                }
                let KdlValue::String {
                    value: path_str,
                    span: path_span,
                    ..
                } = first
                else {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource path arg must be a string literal",
                        span_of(first),
                    ));
                };

                // Exactly one property: `sha256` with a 64-char lowercase-hex value.
                let mut sha256_value: Option<&KdlValue> = None;
                for entry in child.entries() {
                    let KdlEntry::Prop { key, value, .. } = entry else {
                        // Arg re-check is already done above; this branch is
                        // unreachable in practice but defends against future
                        // KdlEntry variants.
                        continue;
                    };
                    if key.as_ref() != "sha256" {
                        return Err(integrity_err_at(
                            "INTEGRITY_STRUCTURE_INVALID",
                            format!(
                                "resource node has unknown property '{}'; only 'sha256' is allowed",
                                key.as_ref()
                            ),
                            child.span(),
                        ));
                    }
                    if sha256_value.is_some() {
                        return Err(integrity_err_at(
                            "INTEGRITY_STRUCTURE_INVALID",
                            "resource node has duplicate 'sha256' property",
                            child.span(),
                        ));
                    }
                    sha256_value = Some(value);
                }
                let Some(sha256_value) = sha256_value else {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource node is missing required 'sha256' property",
                        child.span(),
                    ));
                };
                let KdlValue::String {
                    value: hex_str,
                    span: hex_span,
                    ..
                } = sha256_value
                else {
                    return Err(integrity_err_at(
                        "INTEGRITY_STRUCTURE_INVALID",
                        "resource sha256 property must be a string literal",
                        span_of(sha256_value),
                    ));
                };

                // NFC enforcement on the path arg (grammar §1).
                ensure_unicode(path_str).map_err(|e| unicode_to_error(e, *path_span))?;
                // `ResourcePath` validation (steering §3.4).
                let path = ResourcePath::new(path_str.clone()).map_err(|e| {
                    // ResourcePath errors carry their own RESOURCE_PATH category;
                    // re-stage to 9 to match the rest of stage-9 diagnostics.
                    let mut d = e.diagnostic;
                    d.stage = Some(9);
                    Error { diagnostic: d }
                })?;
                // SHA-256 hex validation.
                let digest = parse_hex64(hex_str).map_err(|e| {
                    // Re-stage any parse_hex64 error to point at the property span.
                    let mut d = e.diagnostic;
                    d.span = span_to_kdl_opt(*hex_span);
                    Error { diagnostic: d }
                })?;

                if resources.contains_key(&path) {
                    return Err(integrity_err(
                        "INTEGRITY_DUPLICATE_ENTRY",
                        format!("duplicate resource entry for path '{}'", path.as_str()),
                    ));
                }
                resources.insert(path.clone(), IntegrityEntry { path, digest });
            }
            other => {
                return Err(integrity_err_at(
                    "INTEGRITY_STRUCTURE_INVALID",
                    format!(
                        "integrity block contains unknown child node '{}'; only 'algorithm' and 'resource' are allowed",
                        other
                    ),
                    child.span(),
                ));
            }
        }
    }

    let (algorithm, _) = algorithm.ok_or_else(|| {
        integrity_err_at(
            "INTEGRITY_BAD_ALGORITHM",
            "integrity block must contain exactly one 'algorithm' child",
            integ.span(),
        )
    })?;

    if resources.is_empty() {
        return Err(integrity_err_at(
            "INTEGRITY_STRUCTURE_INVALID",
            "integrity block must contain at least one 'resource' child",
            integ.span(),
        ));
    }

    Ok(IntegrityManifest {
        algorithm,
        resources,
    })
}

/// Reject strings that are not NFC or that contain a forbidden Unicode
/// category / noncharacter.
fn ensure_unicode(value: &str) -> Result<(), UnicodeViolation> {
    elang_unicode::validate_str(value).map(|_| ())
}

/// Translate a [`UnicodeViolation`] into an `INTEGRITY` diagnostic carrying
/// the `UNICODE` category, with its span attached.
///
/// The code is delegated to [`UnicodeViolation::code`] so the registry
/// stays in sync with `elang-epf` (same `UNICODE_NOT_NFC`,
/// `UNICODE_FORBIDDEN_CATEGORY`, `UNICODE_NONCHARACTER` codes).
fn unicode_to_error(violation: UnicodeViolation, span: elang_kdl::span::SourceSpan) -> Error {
    let code = violation.code();
    let message = match violation {
        UnicodeViolation::NotNfc => "string is not in Normalization Form C".to_string(),
        UnicodeViolation::ForbiddenCategory { scalar } => format!(
            "string contains forbidden Unicode scalar U+{:04X}",
            scalar as u32
        ),
        UnicodeViolation::Noncharacter { scalar } => format!(
            "string contains Unicode noncharacter U+{:04X}",
            scalar as u32
        ),
    };
    let mut d = elang_core::error::Diagnostic::new(code, Category::Unicode, message).with_stage(9);
    if let Some(s) = span_to_kdl_opt(span) {
        d = d.with_span(s);
    }
    Error { diagnostic: d }
}

fn span_of(v: &KdlValue) -> elang_kdl::span::SourceSpan {
    v.span()
}

fn span_to_kdl_opt(span: elang_kdl::span::SourceSpan) -> Option<elang_core::error::KdlSpan> {
    if span.is_invalid() {
        return None;
    }
    Some(elang_core::error::KdlSpan {
        line: span.start.line,
        column: span.start.column,
        length: span.len(),
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact codes/spans; a panic signals a regression"
)]
mod tests {
    use super::*;
    use elang_core::error::Category;

    use elang_core::limits::MAX_INTEGRITY_BYTES;

    /// The canonical golden manifest. Covers four resources with two
    /// known-answer SHA-256 digests; the others are zero hashes (legal —
    /// the parse layer doesn't care about digest values, only their shape).
    const GOLDEN: &str = "\
integrity {
    algorithm \"sha-256\"
    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"Linguistics/phonemes.eld\" sha256=\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"
    resource \"mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"
}
";

    /// The 64-character zero hash, used as a syntactically valid default for
    /// most tests (the parse layer doesn't care about digest values, only
    /// their shape).
    const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn golden_vector_parses() {
        let m = parse(GOLDEN.as_bytes()).expect("golden must parse");
        assert_eq!(m.algorithm, Algorithm::Sha256);
        assert_eq!(m.resources.len(), 4);
        assert!(m.resources.contains_key(&path("Header/format.version")));
        assert!(m.resources.contains_key(&path("Linguistics/phonemes.eld")));
        assert!(m.resources.contains_key(&path("mimetype")));
        assert!(m.resources.contains_key(&path("package.epf")));
    }

    #[test]
    fn rejects_oversize_input() {
        // The 16 MiB ceiling is well below `u32::MAX` so the cast is safe;
        // the test asserts the limit-trip path fires.
        #[allow(clippy::cast_possible_truncation)]
        let input = vec![b' '; (MAX_INTEGRITY_BYTES + 1) as usize];
        let err = parse(&input).unwrap_err();
        assert_eq!(err.diagnostic.code, "LIMIT_INTEGRITY_BYTES");
        assert_eq!(err.diagnostic.category, Category::Limit);
    }

    #[test]
    fn accepts_input_at_exact_cap() {
        // Build a syntactically valid manifest padded with a block comment
        // out to exactly MAX_INTEGRITY_BYTES. The parse may fail with a
        // non-cap diagnostic on a malformed body, but never with
        // LIMIT_INTEGRITY_BYTES.
        #[allow(clippy::cast_possible_truncation)]
        let target = MAX_INTEGRITY_BYTES as usize;
        let prefix = b"/* ";
        let suffix = b" */";
        let filler_len = target - prefix.len() - suffix.len();
        let mut input = Vec::with_capacity(target);
        input.extend_from_slice(prefix);
        input.resize(filler_len + prefix.len(), b' ');
        input.extend_from_slice(suffix);
        assert_eq!(input.len(), target);
        let result = parse(&input);
        if let Err(e) = &result {
            assert_ne!(
                e.diagnostic.code, "LIMIT_INTEGRITY_BYTES",
                "input at exactly MAX_INTEGRITY_BYTES must not trip the cap"
            );
        }
    }

    // ---- C-01: top-level shape -------------------------------------------

    #[test]
    fn c01_rejects_empty_block() {
        let doc = kdl_parse("integrity {}");
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c01_rejects_two_top_level_nodes() {
        let doc = kdl_parse(
            "integrity {\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\nintegrity {}\n",
        );
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c01_rejects_wrong_top_level_node_name() {
        let doc = kdl_parse("not-integrity {\n    algorithm \"sha-256\"\n}\n");
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c01_rejects_arg_on_top_level() {
        let doc = kdl_parse(
            "integrity \"0.1.0\" {\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\n",
        );
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c01_rejects_prop_on_top_level() {
        let doc = kdl_parse(
            "integrity x=\"1\" {\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\n",
        );
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c01_rejects_missing_block() {
        let doc = kdl_parse("integrity");
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    // ---- C-02: algorithm ------------------------------------------------

    #[test]
    fn c02_rejects_wrong_algorithm_value() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-512\"\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_ALGORITHM");
        assert!(err.diagnostic.message.contains("sha-512"));
    }

    #[test]
    fn c02_rejects_missing_algorithm() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_ALGORITHM");
    }

    #[test]
    fn c02_rejects_duplicate_algorithm() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_ALGORITHM");
    }

    #[test]
    fn c02_rejects_algorithm_with_property() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\" x=\"1\"\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c02_rejects_algorithm_with_block() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\" {{}}\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c02_rejects_algorithm_no_arg() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm\n    resource \"a\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_ALGORITHM");
    }

    // ---- C-03: resource entries -----------------------------------------

    #[test]
    fn c03_rejects_resource_no_arg() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c03_rejects_resource_two_args() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" \"b\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c03_rejects_resource_missing_sha256_property() {
        let doc = kdl_parse("integrity {\n    algorithm \"sha-256\"\n    resource \"a\"\n}\n");
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c03_rejects_resource_extra_property() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{ZERO}\" size=42\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c03_rejects_resource_with_block() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{ZERO}\" {{}}\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    #[test]
    fn c03_rejects_uppercase_hex() {
        let upper = "A".repeat(64);
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{upper}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
    }

    #[test]
    fn c03_rejects_short_hex() {
        let doc = kdl_parse(
            "integrity {\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"abcd\"\n}\n",
        );
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
    }

    #[test]
    fn c03_rejects_long_hex() {
        let long = "a".repeat(65);
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{long}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_BAD_HEX");
    }

    #[test]
    fn c03_rejects_invalid_path() {
        // Absolute path fails the steering §3.4 rules.
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"/abs\" sha256=\"{ZERO}\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.category, Category::ResourcePath);
        assert_eq!(err.diagnostic.code, "PATH_ABSOLUTE");
        // PATH_* codes are constructed stage-less by `ResourcePath::new`;
        // every caller re-stamps the pipeline position. Stage 9 here is the
        // integrity manifest validator — a future refactor that drops the
        // re-stage trips this assertion instead of silently changing the
        // diagnostic stream.
        assert_eq!(err.diagnostic.stage, Some(9));
    }

    // ---- C-05: duplicates and unknown constructs -----------------------

    #[test]
    fn c05_rejects_duplicate_resource_path() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{ZERO}\"\n    resource \"a\" sha256=\"1111111111111111111111111111111111111111111111111111111111111111\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_DUPLICATE_ENTRY");
    }

    #[test]
    fn c05_rejects_unknown_child() {
        let doc = kdl_parse(&format!(
            "integrity {{\n    algorithm \"sha-256\"\n    resource \"a\" sha256=\"{ZERO}\"\n    unknown \"x\"\n}}\n"
        ));
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
        assert!(err.diagnostic.message.contains("unknown"));
    }

    #[test]
    fn c05_rejects_empty_resource_list() {
        // Algorithm present, no resources: the empty-coverage guarantee from
        // the design (the manifest must cover something).
        let doc = kdl_parse("integrity {\n    algorithm \"sha-256\"\n}\n");
        let err = parse_document(&doc).unwrap_err();
        assert_eq!(err.diagnostic.code, "INTEGRITY_STRUCTURE_INVALID");
    }

    // ---- helpers --------------------------------------------------------

    fn kdl_parse(s: &str) -> KdlDocument {
        let mut b = BudgetTracker::new();
        kdl_parse_with_budgets(s.as_bytes(), &mut b).expect("valid KDL")
    }

    fn path(s: &str) -> ResourcePath {
        ResourcePath::new(s.to_string()).expect("valid path")
    }
}
