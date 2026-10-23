//! EPF `package.epf` v0.1.0 schema validator (T0601–T0615).
//!
//! The validator runs over a `elang_kdl::ast::KdlDocument` produced by the
//! generic parser and enforces every constraint labelled `[C-nn]` in
//! `specs/grammar/epf-0.1.0.md`. The constraints it checks structurally
//! (C-01..C-11) are derived into the ABNF; the cross-field C-12 "path exists
//! in the index" check is performed by [`crate::manifest::resolve_phonemes`]
//! once the caller has the parsed index.
//!
//! Error codes come from `specs/registry/error-codes.md`:
//!
//! | Constraint | Code |
//! |---|---|
//! | C-01 unknown top-level node | `EPF_UNKNOWN_NODE` |
//! | C-01 duplicate top-level node | `EPF_DUPLICATE_NODE` |
//! | C-01 missing required node | `EPF_MISSING_NODE` |
//! | C-01 wrong arg / property / block presence | `EPF_BAD_ARITY` |
//! | C-02 `epf`/`eld` ≠ `0.1.0` | `EPF_BAD_VERSION` |
//! | C-05 missing/empty block | `EPF_BAD_ARITY` |
//! | C-05 `language-version` not strict semver | `LANG_BAD_VERSION` |
//! | C-06 unknown metadata child / duplicate | `EPF_UNKNOWN_NODE` / `EPF_DUPLICATE_NODE` |
//! | C-07..C-09 identifier URN malformed | `EPF_BAD_URN` |
//! | C-08/C-09 metadata length/content | `EPF_BAD_METADATA_FIELD` |
//! | C-10 version mismatch | `EPF_VERSION_MISMATCH` |
//! | C-11 manifest child problems | `EPF_MANIFEST_INVALID` |
//! | C-12 path not indexed | `EPF_MANIFEST_PATH_NOT_INDEXED` |
//!
//! NFC / forbidden-category violations on string values are routed through
//! the `UNICODE` category so they surface in stage-10 diagnostics alongside
//! the schema codes.

use elang_core::error::{Category, Error};
use elang_core::ids::ResourcePath;
use elang_core::model::epf as model;
use elang_core::version::SemVer;
use elang_kdl::ast::{KdlChildren, KdlDocument, KdlNode};
use elang_kdl::budget::BudgetTracker;
use elang_kdl::error::ParseError;
use elang_kdl::parse_with_budgets as kdl_parse_with_budgets;
use elang_kdl::value::KdlValue;
use elang_unicode::UnicodeViolation;

use crate::error::{epf_err, epf_err_at, from_core, from_kdl};
use crate::manifest::has_eld_extension;

/// Parse `input` as `package.epf` v0.1.0 and return the validated semantic
/// model.
///
/// Runs every schema check except C-12's index-existence cross-check; the
/// caller is responsible for that once both the EPF and the archive index
/// are in hand.
pub fn parse(input: &[u8]) -> Result<model::EpfPackage, Error> {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
}

/// Parse `input` with a caller-supplied [`BudgetTracker`], useful when
/// multiple resources from the same archive share aggregate budgets.
pub fn parse_with_budgets(
    input: &[u8],
    budgets: &mut BudgetTracker,
) -> Result<model::EpfPackage, Error> {
    let doc = kdl_parse_with_budgets(input, budgets).map_err(from_kdl)?;
    parse_document(&doc)
}

/// Parse a `package.epf` document that has **already** been tokenised by
/// `elang-kdl`.
///
/// This split exists so the reader pipeline can run the KDL parser once with
/// shared budgets and feed the resulting `KdlDocument` into multiple schema
/// validators.
pub fn parse_document(doc: &KdlDocument) -> Result<model::EpfPackage, Error> {
    let (epf_v, eld_v, language_v, metadata, manifest_path) = validate_top_level(doc)?;
    let manifest = build_manifest(&manifest_path)?;
    model::EpfPackage::new(epf_v, eld_v, language_v, metadata, manifest).map_err(from_core)
}

/// KDL parse-error → crate `Error` adapter, exposed so callers that already
/// hold a `ParseError` (e.g. after re-running the parser on a single
/// resource) can route through the EPF layer without a second parse.
pub fn parse_error(err: ParseError) -> Error {
    from_kdl(err)
}

/// Validate the five top-level nodes and return the components the model
/// needs.
///
/// The top-level order is *enforced* — not for canonical emission but so
/// that all five missing/duplicate failures are detected deterministically
/// in source order. The canonical emitter always rewrites the order itself;
/// this validator accepts any source order.
fn validate_top_level(
    doc: &KdlDocument,
) -> Result<(SemVer, SemVer, SemVer, model::Metadata, String), Error> {
    let mut epf_node: Option<&KdlNode> = None;
    let mut eld_node: Option<&KdlNode> = None;
    let mut lang_node: Option<&KdlNode> = None;
    let mut metadata_node: Option<&KdlNode> = None;
    let mut manifest_node: Option<&KdlNode> = None;

    for node in doc.nodes() {
        reject_type_annotation(node)?;
        match node.name() {
            "epf" => assign_once(&mut epf_node, node, "EPF_DUPLICATE_NODE", "epf")?,
            "eld" => assign_once(&mut eld_node, node, "EPF_DUPLICATE_NODE", "eld")?,
            "language-version" => {
                assign_once(&mut lang_node, node, "EPF_DUPLICATE_NODE", "language-version")?
            }
            "metadata" => {
                assign_once(&mut metadata_node, node, "EPF_DUPLICATE_NODE", "metadata")?
            }
            "manifest" => {
                assign_once(&mut manifest_node, node, "EPF_DUPLICATE_NODE", "manifest")?
            }
            _ => {
                return Err(epf_err_at(
                    "EPF_UNKNOWN_NODE",
                    format!("unknown top-level node `{}`", node.name()),
                    node.span(),
                ));
            }
        }
    }

    let epf_node = require_some(epf_node, "EPF_MISSING_NODE", "epf")?;
    let eld_node = require_some(eld_node, "EPF_MISSING_NODE", "eld")?;
    let lang_node = require_some(lang_node, "EPF_MISSING_NODE", "language-version")?;
    let metadata_node = require_some(metadata_node, "EPF_MISSING_NODE", "metadata")?;
    let manifest_node = require_some(manifest_node, "EPF_MISSING_NODE", "manifest")?;

    let epf_v = validate_version_node(epf_node, true)?;
    let eld_v = validate_version_node(eld_node, true)?;
    let language_v = validate_version_node(lang_node, false)?;
    let metadata = validate_metadata(metadata_node)?;
    let manifest_path = validate_manifest(manifest_node)?;

    Ok((epf_v, eld_v, language_v, metadata, manifest_path))
}

// ---- node-shape helpers --------------------------------------------------

/// Reject KDL type annotations (`(type)`). The grammar says nothing about
/// them and they would change the node's value typing in a way nothing
/// downstream expects. This is the fail-closed reading of "no silent
/// acceptance of ambiguous syntax" (steering §0.5).
fn reject_type_annotation(node: &KdlNode) -> Result<(), Error> {
    if let Some(ty) = node.ty() {
        // `KdlNode::ty()` returns only the annotation text, so the diagnostic
        // points at the whole node rather than the annotation alone. That is
        // still precise enough to locate the offending construct.
        return Err(epf_err_at(
            "EPF_BAD_ARITY",
            format!("node `{}` has an unexpected type annotation `({ty})`", node.name()),
            node.span(),
        ));
    }
    Ok(())
}

/// Mark a node slot as filled; reject a second node of the same name with
/// `code`.
fn assign_once<'a>(
    slot: &mut Option<&'a KdlNode>,
    node: &'a KdlNode,
    code: &'static str,
    name: &str,
) -> Result<(), Error> {
    if slot.is_some() {
        return Err(epf_err_at(
            code,
            format!("duplicate top-level node `{name}`"),
            node.span(),
        ));
    }
    *slot = Some(node);
    Ok(())
}

/// Require a previously-marked node, failing with `code`.
fn require_some<'a>(
    slot: Option<&'a KdlNode>,
    code: &'static str,
    name: &str,
) -> Result<&'a KdlNode, Error> {
    match slot {
        Some(n) => Ok(n),
        None => Err(epf_err(code, format!("missing required node `{name}`"))),
    }
}

/// Validate an `epf`, `eld` or `language-version` top-level node.
///
/// `must_be_locked`: for `epf` and `eld`, the string value must be exactly
/// `0.1.0` (C-02/C-04); for `language-version` it must be a strict semver
/// (C-05).
fn validate_version_node(node: &KdlNode, must_be_locked: bool) -> Result<SemVer, Error> {
    reject_props(node)?;
    reject_block(node)?;
    let value = require_single_string_arg(node)?;
    let lexeme = value.as_str().ok_or_else(|| {
        epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` argument must be a string", node.name()),
        )
    })?;

    if must_be_locked && lexeme != elang_core::version_domains::EPF_VERSION {
        return Err(epf_err(
            "EPF_BAD_VERSION",
            format!(
                "node `{}` must be exactly `{}`, got `{lexeme}`",
                node.name(),
                elang_core::version_domains::EPF_VERSION,
            ),
        ));
    }

    SemVer::parse(lexeme).map_err(|e| {
        // `SemVer::parse` reports its failures under `EPF_BAD_VERSION`; for
        // `language-version` re-route through `LANG_BAD_VERSION` (C-05) so
        // the diagnostic is unambiguous about which version node failed.
        if must_be_locked {
            from_core(e)
        } else {
            epf_err(
                "LANG_BAD_VERSION",
                format!("language-version `{lexeme}` is not strict MAJOR.MINOR.PATCH"),
            )
        }
    })
}

/// Validate the `metadata` block (C-05, C-06..C-09).
fn validate_metadata(node: &KdlNode) -> Result<model::Metadata, Error> {
    reject_props(node)?;
    reject_args(node)?;
    let children = require_block(node)?;
    reject_empty(children, "metadata")?;

    let mut identifier: Option<&KdlNode> = None;
    let mut name_node: Option<&KdlNode> = None;
    let mut description_node: Option<&KdlNode> = None;

    for child in children.iter() {
        reject_type_annotation(child)?;
        match child.name() {
            "identifier" => {
                assign_once(&mut identifier, child, "EPF_DUPLICATE_NODE", "identifier")?
            }
            "name" => assign_once(&mut name_node, child, "EPF_DUPLICATE_NODE", "name")?,
            "description" => {
                assign_once(&mut description_node, child, "EPF_DUPLICATE_NODE", "description")?
            }
            other => {
                return Err(epf_err_at(
                    "EPF_UNKNOWN_NODE",
                    format!("unknown metadata child `{other}`"),
                    child.span(),
                ));
            }
        }
    }

    let identifier_node = require_some(identifier, "EPF_MISSING_NODE", "identifier")?;
    let name_node = require_some(name_node, "EPF_MISSING_NODE", "name")?;

    // `identifier` is checked for shape only (no props, no block, one string
    // arg); its length/emptiness rules live in the URN itself, so that a bad
    // URN is reported as `EPF_BAD_URN` (C-07) rather than a generic metadata
    // field violation. The ≤512-byte cap is re-checked here because the
    // diagnostic must come from the EPF layer, not from `elang-core`.
    let identifier_str = validate_identifier(identifier_node)?;
    let name_str = validate_metadata_string(name_node, "name", MAX_NAME_BYTES, false)?;
    // C-09: `description` is optional and may be present but empty.
    let description_str: Option<String> = match description_node {
        Some(n) => Some(
            validate_metadata_string(n, "description", MAX_DESCRIPTION_BYTES, true)?.to_string(),
        ),
        None => None,
    };

    // NFC + forbidden-category check on every string before handing to the
    // core model. The grammar (epf-0.1.0.md §1) requires NFC; the canonical
    // spec (ADR 0005 rule 6) requires "free of controls/Cf/Co/Cn". Both are
    // checked by `elang_unicode::validate_str`.
    ensure_unicode(identifier_str)?;
    ensure_unicode(name_str)?;
    if let Some(d) = description_str.as_deref() {
        ensure_unicode(d)?;
    }

    let language_id = model::LanguageIdentifier::parse(identifier_str).map_err(from_core)?;
    model::Metadata::new(language_id, name_str, description_str).map_err(from_core)
}

/// Validate the `manifest` block (C-11).
///
/// Returns the validated phoneme path string so the caller can hand it to
/// [`crate::manifest::resolve_phonemes`] for the index-existence check.
fn validate_manifest(node: &KdlNode) -> Result<String, Error> {
    reject_props(node)?;
    reject_args(node)?;
    let children = require_block(node)?;
    reject_empty(children, "manifest")?;

    let mut phonemes: Option<&KdlNode> = None;
    for child in children.iter() {
        reject_type_annotation(child)?;
        match child.name() {
            "phonemes" => {
                assign_once(&mut phonemes, child, "EPF_MANIFEST_INVALID", "phonemes")?
            }
            // `modules` is the canonical banned entry (registry row); any
            // other unknown child is rejected with the same code so a typo
            // can't slip through as a different diagnostic.
            "modules" => {
                return Err(epf_err_at(
                    "EPF_MANIFEST_INVALID",
                    "`modules` is not permitted in EPF v0.1.0",
                    child.span(),
                ));
            }
            other => {
                return Err(epf_err_at(
                    "EPF_MANIFEST_INVALID",
                    format!("unknown manifest entry `{other}`"),
                    child.span(),
                ));
            }
        }
    }
    let phonemes_node = require_some(phonemes, "EPF_MANIFEST_INVALID", "phonemes")?;

    reject_props(phonemes_node)?;
    reject_block(phonemes_node)?;
    let value = require_single_string_arg(phonemes_node)?;
    let path = value
        .as_str()
        .ok_or_else(|| {
            epf_err(
                "EPF_MANIFEST_INVALID",
                "`phonemes` argument must be a string",
            )
        })?
        .to_string();

    // C-12 first half (syntactic): nonempty, ends with `.eld`, safe
    // package-relative resource path. The second half (path exists in the
    // archive index) is the caller's job — see `crate::manifest`.
    if path.is_empty() {
        return Err(epf_err(
            "EPF_MANIFEST_PATH_NOT_INDEXED",
            "`phonemes` path must be nonempty",
        ));
    }
    if !has_eld_extension(&path) {
        return Err(epf_err(
            "EPF_MANIFEST_PATH_NOT_INDEXED",
            format!("`phonemes` path {path:?} must end with `.eld`"),
        ));
    }
    ensure_unicode(&path)?;
    // The `ResourcePath` constructor enforces the canonical path rules; any
    // failure is a manifest-level error in the v0.1.0 schema.
    ResourcePath::new(path.clone()).map_err(|_| {
        epf_err(
            "EPF_MANIFEST_INVALID",
            format!("invalid phoneme path {path:?}"),
        )
    })?;

    Ok(path)
}

/// Build a `Manifest` from a path string the validator already accepted.
fn build_manifest(path: &str) -> Result<model::Manifest, Error> {
    let rp = ResourcePath::new(path).map_err(|_| {
        epf_err(
            "EPF_MANIFEST_INVALID",
            format!("invalid phoneme path {path:?}"),
        )
    })?;
    Ok(model::Manifest::new(rp))
}

// ---- shared node-shape primitives ---------------------------------------

/// Reject any properties on `node`. Properties are allowed by KDL 2.0 in
/// general but the EPF grammar lists none.
fn reject_props(node: &KdlNode) -> Result<(), Error> {
    if node.props().next().is_some() {
        return Err(epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` may not declare properties", node.name()),
        ));
    }
    Ok(())
}

/// Reject any positional arguments on `node`. Mirrors [`reject_props`] and is
/// used for the `metadata`/`manifest` blocks.
fn reject_args(node: &KdlNode) -> Result<(), Error> {
    if node.args().next().is_some() {
        return Err(epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` may not have positional arguments", node.name()),
        ));
    }
    Ok(())
}

/// Require a child block on `node`, returning it. The grammar requires the
/// block to be present and nonempty; the latter is checked separately by
/// [`reject_empty`].
fn require_block(node: &KdlNode) -> Result<&KdlChildren, Error> {
    node.children().ok_or_else(|| {
        epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` must have a child block", node.name()),
        )
    })
}

/// Reject any child block on `node`. Used for the version nodes and for the
/// leaf metadata fields and `manifest.phonemes`.
fn reject_block(node: &KdlNode) -> Result<(), Error> {
    if node.children().is_some() {
        return Err(epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` must not have a child block", node.name()),
        ));
    }
    Ok(())
}

/// Require `node` to have exactly one argument (used to pluck the version
/// string or the manifest path).
fn require_single_string_arg(node: &KdlNode) -> Result<&KdlValue, Error> {
    let mut args = node.args();
    let first = args.next().ok_or_else(|| {
        epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` must have exactly one argument", node.name()),
        )
    })?;
    if args.next().is_some() {
        return Err(epf_err(
            "EPF_BAD_ARITY",
            format!("node `{}` must have exactly one argument", node.name()),
        ));
    }
    Ok(first)
}

/// Reject an empty children block. The grammar requires `metadata` and
/// `manifest` to have a **nonempty** body (C-04).
fn reject_empty(children: &KdlChildren, name: &str) -> Result<(), Error> {
    if children.is_empty() {
        return Err(epf_err(
            "EPF_BAD_ARITY",
            format!("`{name}` block must not be empty"),
        ));
    }
    Ok(())
}

// ---- metadata string helpers --------------------------------------------

const MAX_NAME_BYTES: usize = 256;
const MAX_DESCRIPTION_BYTES: usize = 4096;

/// Validate the shape of a metadata child (`identifier`/`name`/`description`):
/// exactly one argument, no properties, no child block. The byte-length cap
/// is checked explicitly so the diagnostic is independent of the core
/// model's finer-grained codes.
///
/// `allow_empty` is true only for `description`, which C-09 permits to be
/// present and empty.
fn validate_metadata_string<'a>(
    node: &'a KdlNode,
    field: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<&'a str, Error> {
    reject_props(node)?;
    reject_block(node)?;
    let value = require_single_string_arg(node)?;
    let s = value.as_str().ok_or_else(|| {
        epf_err(
            "EPF_BAD_METADATA_FIELD",
            format!("metadata.{field} argument must be a string"),
        )
    })?;
    if !allow_empty && s.is_empty() {
        return Err(epf_err(
            "EPF_BAD_METADATA_FIELD",
            format!("metadata.{field} is empty"),
        ));
    }
    if s.len() > max_bytes {
        return Err(epf_err(
            "EPF_BAD_METADATA_FIELD",
            format!(
                "metadata.{field} is {n} UTF-8 bytes, exceeds the {m}-byte limit",
                n = s.len(),
                m = max_bytes,
            ),
        ));
    }
    Ok(s)
}

/// Reject a string that is not in NFC or contains a forbidden Unicode
/// scalar. The error category is `UNICODE` so consumers route it with the
/// rest of the Unicode stage (12) even though we catch it at stage 10.
fn ensure_unicode(value: &str) -> Result<&str, Error> {
    elang_unicode::validate_str(value).map_err(unicode_to_error)?;
    Ok(value)
}

/// Validate the *shape* of the `metadata.identifier` node: exactly one string
/// argument, no properties, no child block. Empty or over-length identifiers
/// are intentionally NOT rejected here — the URN parser enforces C-07 and
/// reports it as `EPF_BAD_URN`, with the underlying `EPF_IDENTIFIER_*` code
/// remapped by `from_core`.
fn validate_identifier(node: &KdlNode) -> Result<&str, Error> {
    reject_props(node)?;
    reject_block(node)?;
    let value = require_single_string_arg(node)?;
    value.as_str().ok_or_else(|| {
        epf_err(
            "EPF_BAD_URN",
            "metadata.identifier argument must be a string",
        )
    })
}

fn unicode_to_error(v: UnicodeViolation) -> Error {
    let code = v.code();
    let message = match v {
        UnicodeViolation::NotNfc => "string is not in Normalization Form C".to_string(),
        UnicodeViolation::ForbiddenCategory { scalar } => format!(
            "string contains forbidden Unicode scalar U+{:04X}",
            u32::from(scalar)
        ),
        UnicodeViolation::Noncharacter { scalar } => {
            format!("string contains Unicode noncharacter U+{:04X}", u32::from(scalar))
        }
    };
    Error::new(code, Category::Unicode, message)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact diagnostic codes/categories; a panic signals a schema regression"
)]
mod tests {
    use super::*;

    /// Canonical, well-formed document used as the baseline for the positive
    /// tests below.
    const VALID: &str = "\
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
metadata {
    identifier \"urn:elang:language:rocky:aelith:1.0.0\"
    name \"Aelith\"
    description \"A small constructed language.\"
}
manifest {
    phonemes \"Linguistics/phonemes.eld\"
}
";

    fn parse_ok(src: &str) -> model::EpfPackage {
        parse(src.as_bytes()).expect("parse should succeed")
    }

    fn parse_err(src: &str) -> Error {
        parse(src.as_bytes()).expect_err("parse should fail")
    }

    /// Build an EPF document by overriding individual slots of the canonical
    /// baseline. Empty slots fall back to the baseline values, so every test
    /// reads as a single diff against the canonical example.
    ///
    /// KDL 2.0 requires a node terminator (LF or `;`) between sibling nodes,
    /// so this builder always emits one child per line. Hand-writing
    /// single-line blocks is the classic way to accidentally test the KDL
    /// parser instead of the EPF schema.
    struct Fixture<'a> {
        epf: &'a str,
        eld: &'a str,
        lang_version: &'a str,
        identifier: &'a str,
        name: &'a str,
        description: Option<&'a str>,
        manifest: &'a str,
        extra_top_level: &'a str,
    }

    impl<'a> Fixture<'a> {
        const fn canonical() -> Self {
            Self {
                epf: "\"0.1.0\"",
                eld: "\"0.1.0\"",
                lang_version: "\"1.0.0\"",
                identifier: "urn:elang:language:rocky:aelith:1.0.0",
                name: "Aelith",
                description: Some("A small constructed language."),
                manifest: "phonemes \"Linguistics/phonemes.eld\"",
                extra_top_level: "",
            }
        }

        fn render(&self) -> String {
            let description = match self.description {
                Some(d) => format!("\n    description {d:?}"),
                None => String::new(),
            };
            let extra = if self.extra_top_level.is_empty() {
                String::new()
            } else {
                format!("\n{}", self.extra_top_level)
            };
            let epf = self.epf;
            let eld = self.eld;
            let lv = self.lang_version;
            let id = format!("{:?}", self.identifier);
            let name = format!("{:?}", self.name);
            let manifest = self.manifest;
            format!(
                "epf {epf}\neld {eld}\nlanguage-version {lv}\nmetadata {{\n    identifier {id}\n    name {name}{description}\n}}\nmanifest {{ {manifest} }}{extra}\n"
            )
        }
    }

    #[test]
    fn accepts_the_canonical_example() {
        let pkg = parse_ok(VALID);
        assert_eq!(pkg.epf_version(), SemVer::new(0, 1, 0));
        assert_eq!(pkg.eld_version(), SemVer::new(0, 1, 0));
        assert_eq!(pkg.language_version(), SemVer::new(1, 0, 0));
        assert_eq!(
            pkg.metadata().identifier().as_str(),
            "urn:elang:language:rocky:aelith:1.0.0"
        );
        assert_eq!(pkg.metadata().name(), "Aelith");
        assert_eq!(
            pkg.metadata().description(),
            Some("A small constructed language.")
        );
        assert_eq!(
            pkg.manifest().phonemes().as_str(),
            "Linguistics/phonemes.eld"
        );
    }

    #[test]
    fn accepts_any_top_level_source_order() {
        // Canonical order is enforced by the writer; the parser accepts any.
        let f = Fixture::canonical();
        let manifest = f.manifest;
        let epf = f.epf;
        let eld = f.eld;
        let lv = f.lang_version;
        let id = format!("{:?}", f.identifier);
        let name = format!("{:?}", f.name);
        let src = format!(
            "manifest {{ {manifest}\n}}\nepf {epf}\nmetadata {{\n    identifier {id}\n    name {name}\n}}\neld {eld}\nlanguage-version {lv}\n"
        );
        assert!(parse(src.as_bytes()).is_ok());
    }

    #[test]
    fn accepts_missing_optional_description() {
        let mut f = Fixture::canonical();
        f.description = None;
        let pkg = parse_ok(&f.render());
        assert_eq!(pkg.metadata().description(), None);
    }

    #[test]
    fn accepts_empty_description() {
        // C-09: a present-but-empty description is distinct from absence.
        let mut f = Fixture::canonical();
        f.description = Some("");
        let pkg = parse_ok(&f.render());
        assert_eq!(pkg.metadata().description(), Some(""));
    }

    #[test]
    fn unknown_top_level_node_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.extra_top_level = "foo \"x\"";
        let e = parse_err(&f.render());
        assert_eq!(e.code(), "EPF_UNKNOWN_NODE");
        assert_eq!(e.category(), Category::EpfSchema);
    }

    #[test]
    fn duplicate_top_level_node_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.extra_top_level = "epf \"0.1.0\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_DUPLICATE_NODE");
    }

    #[test]
    fn missing_required_node_is_rejected() {
        // Drop `manifest` entirely.
        let mut f = Fixture::canonical();
        f.description = None;
        let src = f.render().replace(
            "manifest { phonemes \"Linguistics/phonemes.eld\" }\n",
            "",
        );
        assert_eq!(parse_err(&src).code(), "EPF_MISSING_NODE");
    }

    #[test]
    fn property_on_top_level_node_is_rejected() {
        let src = "epf \"0.1.0\" key=\"v\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn type_annotation_is_rejected() {
        let src = "(t) epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn extra_argument_on_version_node_is_rejected() {
        let src = "epf \"0.1.0\" \"extra\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn child_block_on_version_node_is_rejected() {
        let src = "epf \"0.1.0\" { }\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn epf_node_wrong_version_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.epf = "\"0.2.0\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_VERSION");
    }

    #[test]
    fn eld_node_wrong_version_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.eld = "\"0.2.0\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_VERSION");
    }

    #[test]
    fn language_version_not_strict_semver_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.lang_version = "\"01.0.0\"";
        f.identifier = "urn:elang:language:r:n:01.0.0";
        assert_eq!(parse_err(&f.render()).code(), "LANG_BAD_VERSION");
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.lang_version = "\"2.0.0\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_VERSION_MISMATCH");
    }

    #[test]
    fn malformed_urn_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.identifier = "urn:olang:language:r:n:1.0.0";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_URN");
    }

    #[test]
    fn uppercase_slug_in_urn_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.identifier = "urn:elang:language:Rocky:aelith:1.0.0";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_URN");
    }

    #[test]
    fn identifier_too_long_is_rejected() {
        let f = Fixture::canonical();
        let long_namespace = "a".repeat(600);
        let id = format!("urn:elang:language:{long_namespace}:n:1.0.0");
        let id_quoted = format!("{id:?}");
        let name_quoted = format!("{:?}", f.name);
        let epf = f.epf;
        let eld = f.eld;
        let lv = f.lang_version;
        let manifest = f.manifest;
        let src = format!(
            "epf {epf}\neld {eld}\nlanguage-version {lv}\nmetadata {{\n    identifier {id_quoted}\n    name {name_quoted}\n}}\nmanifest {{ {manifest} }}\n"
        );
        assert_eq!(parse_err(&src).code(), "EPF_BAD_URN");
    }

    #[test]
    fn empty_identifier_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.identifier = "";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_URN");
    }

    #[test]
    fn empty_name_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.name = "";
        assert_eq!(parse_err(&f.render()).code(), "EPF_BAD_METADATA_FIELD");
    }

    #[test]
    fn name_too_long_is_rejected() {
        let f = Fixture::canonical();
        let long_name = "x".repeat(257);
        let name_quoted = format!("{long_name:?}");
        let epf = f.epf;
        let eld = f.eld;
        let lv = f.lang_version;
        let id_quoted = format!("{:?}", f.identifier);
        let manifest = f.manifest;
        let src = format!(
            "epf {epf}\neld {eld}\nlanguage-version {lv}\nmetadata {{\n    identifier {id_quoted}\n    name {name_quoted}\n}}\nmanifest {{ {manifest} }}\n"
        );
        assert_eq!(parse_err(&src).code(), "EPF_BAD_METADATA_FIELD");
    }

    #[test]
    fn description_too_long_is_rejected() {
        let f = Fixture::canonical();
        let long = "x".repeat(4097);
        let epf = f.epf;
        let eld = f.eld;
        let lv = f.lang_version;
        let id_quoted = format!("{:?}", f.identifier);
        let name_quoted = format!("{:?}", f.name);
        let desc_quoted = format!("{long:?}");
        let manifest = f.manifest;
        let src = format!(
            "epf {epf}\neld {eld}\nlanguage-version {lv}\nmetadata {{\n    identifier {id_quoted}\n    name {name_quoted}\n    description {desc_quoted}\n}}\nmanifest {{ {manifest} }}\n"
        );
        assert_eq!(parse_err(&src).code(), "EPF_BAD_METADATA_FIELD");
    }

    #[test]
    fn unknown_metadata_child_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n    authors \"me\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_UNKNOWN_NODE");
    }

    #[test]
    fn duplicate_metadata_field_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_DUPLICATE_NODE");
    }

    #[test]
    fn duplicate_description_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n    description \"a\"\n    description \"b\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_DUPLICATE_NODE");
    }

    #[test]
    fn missing_metadata_name_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_MISSING_NODE");
    }

    #[test]
    fn nested_block_on_metadata_field_is_rejected() {
        // Valid KDL: `identifier` carries an unexpected child block after its
        // string argument. The KDL parser accepts it; the schema validator is
        // what rejects it (C-06: no nested blocks on metadata children).
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\" { }\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn empty_metadata_block_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata { }\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn argument_on_metadata_block_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata \"x\" {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn modules_in_manifest_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest {\n    phonemes \"a/b.eld\"\n    modules \"lexicon\"\n}\n";
        assert_eq!(parse_err(src).code(), "EPF_MANIFEST_INVALID");
    }

    #[test]
    fn unknown_manifest_entry_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest {\n    phonemes \"a/b.eld\"\n    lexemes \"x\"\n}\n";
        assert_eq!(parse_err(src).code(), "EPF_MANIFEST_INVALID");
    }

    #[test]
    fn duplicate_phonemes_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest {\n    phonemes \"a/b.eld\"\n    phonemes \"c/d.eld\"\n}\n";
        assert_eq!(parse_err(src).code(), "EPF_MANIFEST_INVALID");
    }

    #[test]
    fn empty_manifest_block_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"N\"\n}\n\
            manifest { }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_ARITY");
    }

    #[test]
    fn manifest_path_must_end_with_eld() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.manifest = "phonemes \"a/b.txt\"";
        assert_eq!(
            parse_err(&f.render()).code(),
            "EPF_MANIFEST_PATH_NOT_INDEXED"
        );
    }

    #[test]
    fn absolute_manifest_path_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.manifest = "phonemes \"/abs.eld\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_MANIFEST_INVALID");
    }

    #[test]
    fn manifest_path_with_dotdot_is_rejected() {
        let mut f = Fixture::canonical();
        f.description = None;
        f.manifest = "phonemes \"a/../b.eld\"";
        assert_eq!(parse_err(&f.render()).code(), "EPF_MANIFEST_INVALID");
    }

    #[test]
    fn pragma_is_accepted_when_value_is_two_point_oh() {
        let src = format!("/kdl-version \"2.0\"\n{VALID}");
        assert!(parse(src.as_bytes()).is_ok());
    }

    #[test]
    fn comment_in_source_is_accepted() {
        let src = "\
// leading comment
epf \"0.1.0\"
eld \"0.1.0\"
language-version \"1.0.0\"
metadata {
    identifier \"urn:elang:language:rocky:aelith:1.0.0\" // the URN
    name \"Aelith\"
    description \"hi\"
}
/* block comment */
manifest { phonemes \"Linguistics/phonemes.eld\" }
";
        assert!(parse(src.as_bytes()).is_ok());
    }

    #[test]
    fn non_nfc_metadata_value_is_rejected() {
        // U+00E9 is the NFC spelling of "é"; the decomposed form `e` + U+0301
        // is NOT in Normalization Form C (grammar §1 mandates NFC).
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"cafe\u{0301}\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        let e = parse_err(src);
        assert_eq!(e.code(), "UNICODE_NOT_NFC");
        assert_eq!(e.category(), Category::Unicode);
    }

    #[test]
    fn forbidden_category_in_metadata_is_rejected() {
        // U+202E RIGHT-TO-LEFT OVERRIDE is Cf (format), forbidden by ADR 0005
        // rule 6.
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name \"a\u{202E}b\"\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        let e = parse_err(src);
        assert_eq!(e.code(), "UNICODE_FORBIDDEN_CATEGORY");
    }

    #[test]
    fn non_string_metadata_argument_is_rejected() {
        let src = "epf \"0.1.0\"\n\
            eld \"0.1.0\"\n\
            language-version \"1.0.0\"\n\
            metadata {\n    identifier \"urn:elang:language:r:n:1.0.0\"\n    name 42\n}\n\
            manifest { phonemes \"a/b.eld\" }\n";
        assert_eq!(parse_err(src).code(), "EPF_BAD_METADATA_FIELD");
    }
}