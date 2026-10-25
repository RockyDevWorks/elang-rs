//! OA-17 normative artifact manifest (ADR 0011).
//!
//! The manifest is the **freeze-mechanism half** of OA-17: a generated,
//! deterministic, RFC 8785 JCS canonical serialization of every ratified
//! normative artifact in the v0.1.0 corpus, with its own pinned SHA-256
//! digest. The release-tag half of OA-17 remains blocked by OA-14 (canonical
//! ELD goldens, T1121 / issue #21) and is **not** implemented here.
//!
//! Scope (T0123): the manifest covers the files that actually define the
//! format — schemas, grammars (`.md` + `.abnf`), canonical profiles,
//! registries, pinned source data (UNITIPA 2020 chart and the pinned UCD
//! 18.0.0 files), and the byte-exact OA-11 fixture files. Governance and
//! prose outside `specs/` (ADRs, traceability, public docs, the manifest
//! itself) are explicitly excluded. The `.sha256` sidecars next to OA-11
//! fixtures are derived and not authoritative, so they are excluded too.
//!
//! Regeneration: the manifest and `PINNED_SPEC_CHECKSUM` are regenerated at
//! final T0123 after OA-14 lands. Any change to the artifact list, to a
//! covered file's bytes, or to the JCS writer invalidates the pinned digest
//! and is caught by [`verify`].
//!
//! Build via the `gen_normative_manifest` binary; verify via the
//! `normative_manifest` integration test and [`verify`] itself.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// One entry in the normative artifact corpus.
#[derive(Debug, Clone, Copy)]
pub struct Artifact {
    /// Comma-separated OA ids covered by this file (e.g. `"OA-1,OA-4"`).
    pub id: &'static str,
    /// Repository-relative path.
    pub path: &'static str,
}

/// The OA-14 canonical ELD golden corpus (Phase 11, T1121), expanded at
/// manifest-build time from [`crate::canonical_goldens::CASES`].
///
/// Each case directory holds the authored (deliberately non-canonical)
/// source, the byte-exact canonical golden and the conformance
/// expected-result record. The `.sha256` sidecar next to the golden is
/// derived and therefore excluded, exactly as for the OA-11 fixtures.
///
/// Deriving the corpus scope from `CASES` means the freeze and the
/// generator share a single reviewed source of truth: a case cannot be
/// added to (or removed from) the corpus without the freeze scope
/// following it, and a typo in a hand-maintained list of case directories
/// is impossible by construction.
const OA14_CANONICAL_DIR: &str = "fixtures/valid/canonical";
const OA14_CASE_FILES: &[&str] = &["expected.json", "golden/canonical.eld", "input.eld"];

/// Expand the OA-14 canonical ELD corpus into one entry per file, derived
/// from [`crate::canonical_goldens::CASES`].
///
/// Each entry is `(id, repo_relative_path)`. The id is the literal
/// `"OA-14"`. The path is `<dir>/<case_id>/<file>`. The order matches
/// the iteration order of `CASES`; [`artifact_entries`] sorts by path so
/// the manifest is deterministic regardless.
pub fn oa14_artifact_entries() -> Vec<(String, String)> {
    crate::canonical_goldens::CASES
        .iter()
        .flat_map(|case| {
            OA14_CASE_FILES.iter().map(move |f| {
                (
                    "OA-14".to_string(),
                    format!("{OA14_CANONICAL_DIR}/{}/{f}", case.id),
                )
            })
        })
        .collect()
}

/// The full effective artifact corpus: every entry in the static
/// [`ARTIFACTS`] list plus the OA-14 canonical ELD corpus.
///
/// Used by `build_manifest` and `verify_manifest_bytes` as the
/// authoritative source for "every artifact the freeze covers".
/// [`ARTIFACTS`] remains a `const` because the integration test asserts
/// it stays free of duplicates and excludes the manifest itself; the
/// expansion into the runtime corpus is a deterministic function of
/// [`crate::canonical_goldens::CASES`] and the crate-private
/// `OA14_CASE_FILES` table.
pub fn artifact_entries() -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = ARTIFACTS
        .iter()
        .map(|a| (a.id.to_string(), a.path.to_string()))
        .collect();
    entries.extend(oa14_artifact_entries());
    entries
}

/// The normative artifact corpus for v0.1.0. The list is the source of truth
/// for freeze scope; changes require a PR to this array and a regeneration of
/// the manifest (and `PINNED_SPEC_CHECKSUM`).
pub const ARTIFACTS: &[Artifact] = &[
    // ---- specs/schema ----
    Artifact {
        id: "OA-3",
        path: "specs/schema/elang-ipa-repertoire-v1.schema.json",
    },
    Artifact {
        id: "OA-5",
        path: "specs/schema/json-encoding-policy.md",
    },
    Artifact {
        id: "OA-1,OA-4",
        path: "specs/schema/ipa-v1.json",
    },
    // ---- specs/grammar ----
    Artifact {
        id: "OA-9",
        path: "specs/grammar/eld-0.1.0.abnf",
    },
    Artifact {
        id: "OA-9",
        path: "specs/grammar/eld-0.1.0.md",
    },
    Artifact {
        id: "OA-8",
        path: "specs/grammar/epf-0.1.0.abnf",
    },
    Artifact {
        id: "OA-8",
        path: "specs/grammar/epf-0.1.0.md",
    },
    Artifact {
        id: "OA-6",
        path: "specs/grammar/integrity-0.1.0.abnf",
    },
    Artifact {
        id: "OA-6",
        path: "specs/grammar/integrity-0.1.0.md",
    },
    // ---- specs/canonical ----
    Artifact {
        id: "OA-7",
        path: "specs/canonical/epf-canonical-serialization.md",
    },
    // ---- specs/registry ----
    Artifact {
        id: "OA-11",
        path: "specs/registry/binary-reference-vectors.json",
    },
    Artifact {
        id: "OA-14",
        path: "specs/registry/eld-canonical-vectors.json",
    },
    Artifact {
        id: "OA-10",
        path: "specs/registry/error-codes.json",
    },
    Artifact {
        id: "OA-10",
        path: "specs/registry/error-codes.md",
    },
    Artifact {
        id: "OA-12",
        path: "specs/registry/ipa-tie-bar-vectors.json",
    },
    Artifact {
        id: "OA-10",
        path: "specs/registry/kdl-2.0-compatibility.md",
    },
    Artifact {
        id: "OA-15",
        path: "specs/registry/resource-policy-v0.1.0.md",
    },
    // ---- specs/sources (pinned source data) ----
    Artifact {
        id: "OA-13",
        path: "specs/sources/SOURCES.md",
    },
    Artifact {
        id: "OA-1",
        path: "specs/sources/ipa-association-full-chart.html",
    },
    Artifact {
        id: "OA-1",
        path: "specs/sources/unitipa-ipa-chart-2020.pdf",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/CompositionExclusions.txt",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/DerivedGeneralCategory.txt",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/DerivedNormalizationProps.txt",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/NormalizationTest.txt",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/SOURCES.md",
    },
    Artifact {
        id: "OA-13",
        path: "specs/sources/ucd-18.0.0/UnicodeData.txt",
    },
    // ---- OA-11 byte-exact fixtures (.sha256 sidecars are derived and not authoritative) ----
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/archive.elang",
    },
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/expected.json",
    },
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/footer.bin",
    },
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/header.bin",
    },
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/index.bin",
    },
    Artifact {
        id: "OA-11",
        path: "fixtures/valid/container/minimal-archive/layout.json",
    },
];

/// Repository-relative path of the generated manifest file. The manifest
/// self-excludes from `ARTIFACTS` to avoid a circular digest.
pub const MANIFEST_PATH: &str = "specs/registry/normative-manifest.json";

/// Manifest schema identifier (carried as a top-level field for forward
/// compatibility and obvious provenance).
pub const MANIFEST_SCHEMA: &str = "elang.normative-manifest.v1";

/// Current provisional v0.1.0 label. ADR 0011 states that the pinned digest
/// is regenerated at final T0123 once OA-14 lands.
pub const RELEASE: &str = "v0.1.0";

/// Pinned SHA-256 of the canonical (RFC 8785 JCS) bytes of the generated
/// manifest at the current ratified state.
///
/// This value pins the **complete v0.1.0 normative corpus**: the ratified
/// OA-1..OA-16 artifacts *plus* the OA-14 canonical ELD golden corpus
/// (`specs/registry/eld-canonical-vectors.json` and, for each of the
/// thirteen published cases, the authored source, the byte-exact golden
/// and the conformance record).
///
/// ADR 0011 §6 made this the final T0123 step: the freeze mechanism
/// shipped with a *provisional* digest covering OA-1..OA-16 minus OA-14,
/// and required the manifest to be regenerated once OA-14 landed so the
/// canonical ELD goldens entered the corpus and the digest was
/// re-derived. That regeneration is this value.
///
/// The digest attests that the corpus bytes have not changed. It is **not**
/// a declaration that the release is published — the `v0.1.0` tag is the
/// release act, and it is created separately (T1617).
pub const PINNED_SPEC_CHECKSUM: &str =
    "baca9f75d622ca6986d0aed4923964bd729e48e88afe00e0d21dbbd76296e50b";

/// Errors produced by [`build_manifest`] and [`verify`].
#[derive(Debug)]
pub enum ManifestError {
    /// Filesystem or I/O failure reading an artifact or the manifest.
    Io {
        /// Repository-relative or absolute path that failed.
        path: PathBuf,
        /// Underlying error message.
        message: String,
    },
    /// The committed manifest file is not parseable as JSON or is missing a
    /// required field.
    Parse {
        /// Repository-relative or absolute path that failed.
        path: PathBuf,
        /// Underlying error message.
        message: String,
    },
    /// A SHA-256 digest in the manifest did not match the recomputed digest
    /// of the artifact bytes on disk, or `PINNED_SPEC_CHECKSUM` did not match
    /// the committed manifest bytes.
    DigestMismatch {
        /// Repository-relative or absolute path that failed.
        path: PathBuf,
        /// Expected digest (hex).
        expected: String,
        /// Actual digest (hex).
        actual: String,
    },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io { path, message } => {
                write!(f, "io error at {}: {}", path.display(), message)
            }
            ManifestError::Parse { path, message } => {
                write!(f, "parse error in {}: {}", path.display(), message)
            }
            ManifestError::DigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "digest mismatch at {}: expected {} got {}",
                path.display(),
                expected,
                actual
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Compute the RFC 8785 JCS canonical bytes for `value`.
///
/// Sorted object keys by UTF-16 code units, no insignificant whitespace,
/// UTF-8 encoding, integers printed without exponent/decimal, and strings
/// escaped per RFC 8785 §3.2.2.2: quotes, backslashes, and control characters
/// below U+0020 are escaped; all other characters are emitted directly as UTF-8.
pub fn jcs_canonicalize(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_jcs(value, &mut out);
    out
}

fn write_jcs(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        serde_json::Value::Number(n) => out.push_str(&n.to_string()),
        serde_json::Value::String(s) => write_jcs_string(s, out),
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs(item, out);
            }
            out.push(']');
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs_string(k, out);
                out.push(':');
                if let Some(v) = map.get(*k) {
                    write_jcs(v, out);
                }
            }
            out.push('}');
        }
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn write_jcs_string(s: &str, out: &mut String) {
    out.push('"');
    // RFC 8785 §3.2.2.2: only `"`, `\` and control characters below U+0020 are
    // escaped. Every other character — including non-BMP scalars — is emitted
    // literally as UTF-8. Surrogate-pair escaping is a JSON-legal encoding
    // but is NOT JCS-canonical, so it must not be used here.
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{000A}' => out.push_str("\\n"),
            '\u{000D}' => out.push_str("\\r"),
            '\u{0009}' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Build the manifest for the artifact corpus rooted at `release_root`.
///
/// Reads each artifact file from disk, computes its SHA-256, and returns the
/// manifest as both a [`serde_json::Value`] and the canonical JCS bytes. The
/// returned value is sorted and deterministic.
pub fn build_manifest(release_root: &Path) -> Result<(serde_json::Value, Vec<u8>), ManifestError> {
    let corpus = artifact_entries();
    let mut entries: Vec<(String, String, String)> = Vec::with_capacity(corpus.len());
    for (id, path) in &corpus {
        let full = release_root.join(path);
        let bytes = fs::read(&full).map_err(|e| ManifestError::Io {
            path: full.clone(),
            message: e.to_string(),
        })?;
        entries.push((id.clone(), path.clone(), sha256_hex(&bytes)));
    }

    // Build the artifact array with stable ordering (by path) so the value is
    // deterministic; JCS does not reorder array elements.
    entries.sort_by(|a, b| utf16_cmp(&a.1, &b.1));

    let artifacts_json: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(id, path, sha)| {
            // id may contain commas; keep it as a JSON string verbatim.
            serde_json::json!({
                "id": id,
                "path": path,
                "sha256": sha,
            })
        })
        .collect();

    let value = serde_json::json!({
        "manifest_version": 1,
        "schema": MANIFEST_SCHEMA,
        "release": RELEASE,
        "generated_by": "elang-conformance gen_normative_manifest",
        "artifacts": artifacts_json,
    });

    let jcs = jcs_canonicalize(&value).into_bytes();
    Ok((value, jcs))
}

/// Write the manifest file to `release_root`.
///
/// Returns the JCS bytes that were written and their SHA-256 hex digest. The
/// caller is responsible for replacing `PINNED_SPEC_CHECKSUM` with the
/// returned digest and committing the resulting changes together.
pub fn write_manifest(release_root: &Path) -> Result<(Vec<u8>, String), ManifestError> {
    let (_value, jcs) = build_manifest(release_root)?;
    let manifest_path = release_root.join(MANIFEST_PATH);
    // `build_manifest` above already read every artifact, including the ones
    // under `specs/registry/` (the manifest's own directory), so the parent
    // directory is guaranteed to exist and `create_dir_all` would be a no-op.
    fs::write(&manifest_path, &jcs).map_err(|e| ManifestError::Io {
        path: manifest_path.clone(),
        message: e.to_string(),
    })?;
    let digest = sha256_hex(&jcs);
    Ok((jcs, digest))
}

/// Verify the committed manifest against the artifact corpus on disk.
///
/// 1. Reads the committed manifest bytes.
/// 2. Checks their SHA-256 equals `PINNED_SPEC_CHECKSUM`; a mismatch is a
///    [`ManifestError::DigestMismatch`].
/// 3. Parses the manifest and verifies JCS re-canonicalization reproduces
///    the committed bytes exactly.
/// 4. Re-hashes the artifact files on disk and checks each digest equals
///    the manifest entry's recorded `sha256`. The manifest and [`ARTIFACTS`]
///    must list exactly the same entries (IDs and paths).
pub fn verify(release_root: &Path) -> Result<(), ManifestError> {
    let manifest_path = release_root.join(MANIFEST_PATH);
    let committed = fs::read(&manifest_path).map_err(|e| ManifestError::Io {
        path: manifest_path.clone(),
        message: e.to_string(),
    })?;
    let committed_digest = sha256_hex(&committed);
    if committed_digest != PINNED_SPEC_CHECKSUM {
        return Err(ManifestError::DigestMismatch {
            path: manifest_path,
            expected: PINNED_SPEC_CHECKSUM.to_string(),
            actual: committed_digest,
        });
    }
    verify_manifest_bytes(&committed, release_root)
}

/// Verify the parse/re-canonicalization and per-artifact steps of [`verify`]
/// against a caller-supplied manifest byte slice.
///
/// Splitting out the digest gate keeps [`verify`]'s public contract intact
/// (bytes must hash to `PINNED_SPEC_CHECKSUM`) while letting the internal
/// validation steps be exercised with crafted manifests.
fn verify_manifest_bytes(committed: &[u8], release_root: &Path) -> Result<(), ManifestError> {
    let manifest_path = release_root.join(MANIFEST_PATH);

    // Parse and re-canonicalize the committed manifest; the result must be
    // byte-identical to the committed bytes.
    let value: serde_json::Value =
        serde_json::from_slice(committed).map_err(|e| ManifestError::Parse {
            path: manifest_path.clone(),
            message: e.to_string(),
        })?;
    let recomputed = jcs_canonicalize(&value).into_bytes();
    if recomputed != committed {
        return Err(ManifestError::DigestMismatch {
            path: manifest_path,
            expected: PINNED_SPEC_CHECKSUM.to_string(),
            actual: sha256_hex(&recomputed),
        });
    }

    // Verify each artifact entry against disk.
    let artifacts = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ManifestError::Parse {
            path: manifest_path.clone(),
            message: "missing `artifacts` array".to_string(),
        })?;
    let mut disk_entries: BTreeMap<(String, String), String> = BTreeMap::new();
    for (id, path) in artifact_entries() {
        let full = release_root.join(&path);
        let bytes = fs::read(&full).map_err(|e| ManifestError::Io {
            path: full.clone(),
            message: e.to_string(),
        })?;
        disk_entries.insert((id, path), sha256_hex(&bytes));
    }
    for entry in artifacts {
        let id = entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ManifestError::Parse {
                path: manifest_path.clone(),
                message: "missing entry.id".to_string(),
            })?;
        let path = entry
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ManifestError::Parse {
                path: manifest_path.clone(),
                message: "missing entry.path".to_string(),
            })?;
        let expected = entry
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ManifestError::Parse {
                path: manifest_path.clone(),
                message: "missing entry.sha256".to_string(),
            })?;
        let key = (id.to_string(), path.to_string());
        let actual = disk_entries
            .remove(&key)
            .ok_or_else(|| ManifestError::Parse {
                path: manifest_path.clone(),
                message: format!(
                    "artifact `{}` ({}) is in the manifest but not in the artifact corpus",
                    path, id
                ),
            })?;
        if actual != expected {
            return Err(ManifestError::DigestMismatch {
                path: release_root.join(path).to_path_buf(),
                expected: expected.to_string(),
                actual,
            });
        }
    }
    if !disk_entries.is_empty() {
        let leftover: Vec<String> = disk_entries
            .keys()
            .map(|(id, p)| format!("{} ({})", p, id))
            .collect();
        return Err(ManifestError::Parse {
            path: manifest_path,
            message: format!(
                "artifact corpus contains entries not present in the manifest: {}",
                leftover.join(", ")
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::jcs_canonicalize;
    use crate::normative_manifest::{ManifestError, MANIFEST_PATH};

    #[test]
    fn non_bmp_character_is_emitted_as_raw_utf8() {
        let value = serde_json::json!("\u{1f600}");
        assert_eq!(jcs_canonicalize(&value).as_bytes(), b"\"\xf0\x9f\x98\x80\"");
    }

    // -----------------------------------------------------------------------
    // Display impl for every ManifestError variant
    // -----------------------------------------------------------------------

    #[test]
    fn manifest_error_io_display() {
        let err = ManifestError::Io {
            path: "some/path".into(),
            message: "file not found".into(),
        };
        let s = err.to_string();
        assert!(s.contains("io error at"));
        assert!(s.contains("some/path"));
        assert!(s.contains("file not found"));
    }

    #[test]
    fn manifest_error_parse_display() {
        let err = ManifestError::Parse {
            path: "manifest.json".into(),
            message: "expected object".into(),
        };
        let s = err.to_string();
        assert!(s.contains("parse error in"));
        assert!(s.contains("manifest.json"));
        assert!(s.contains("expected object"));
    }

    #[test]
    fn manifest_error_digest_mismatch_display() {
        let err = ManifestError::DigestMismatch {
            path: "file.bin".into(),
            expected: "abc".into(),
            actual: "def".into(),
        };
        let s = err.to_string();
        assert!(s.contains("digest mismatch at"));
        assert!(s.contains("file.bin"));
        assert!(s.contains("abc"));
        assert!(s.contains("def"));
    }

    #[test]
    fn manifest_error_impl_error() {
        use std::error::Error;
        let err = ManifestError::Io {
            path: "p".into(),
            message: "m".into(),
        };
        assert!(err.source().is_none());
    }

    // -----------------------------------------------------------------------
    // write_jcs: Null, Bool coverage
    // -----------------------------------------------------------------------

    #[test]
    fn jcs_writer_null_bool() {
        let mut out = String::new();
        super::write_jcs(&serde_json::Value::Null, &mut out);
        assert_eq!(out, "null");

        let mut out = String::new();
        super::write_jcs(&serde_json::Value::Bool(true), &mut out);
        assert_eq!(out, "true");

        let mut out = String::new();
        super::write_jcs(&serde_json::Value::Bool(false), &mut out);
        assert_eq!(out, "false");
    }

    // -----------------------------------------------------------------------
    // write_jcs_string escape coverage: \b, \f, \n, \r, \t, control chars
    // -----------------------------------------------------------------------

    #[test]
    fn jcs_writer_escapes_all_control_forms() {
        let cases: &[(char, &str)] = &[
            ('\u{0008}', r#""\b""#),
            ('\u{000C}', r#""\f""#),
            ('\u{000A}', r#""\n""#),
            ('\u{000D}', r#""\r""#),
            ('\u{0009}', r#""\t""#),
        ];
        for (ch, expected) in cases {
            let mut out = String::new();
            super::write_jcs_string(&ch.to_string(), &mut out);
            assert_eq!(out, *expected, "escape for U+{:04X}", *ch as u32);
        }
    }

    #[test]
    fn jcs_writer_escapes_low_control_as_hex() {
        let mut out = String::new();
        super::write_jcs_string("\u{01}", &mut out);
        assert_eq!(out, r#""\u0001""#);
    }

    // -----------------------------------------------------------------------
    // build_manifest I/O error (nonexistent path)
    // -----------------------------------------------------------------------

    #[test]
    fn build_manifest_io_error_on_missing_file() {
        use std::path::Path;
        let root = Path::new("/nonexistent/path");
        let err = super::build_manifest(root).expect_err("expected Io error");
        assert!(matches!(err, ManifestError::Io { .. }));
    }

    // -----------------------------------------------------------------------
    // write_manifest with a temp scratch directory
    // -----------------------------------------------------------------------

    #[test]
    fn write_manifest_creates_file_and_returns_digest() {
        use std::fs;
        use std::path::Path;
        let scratch = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cache/scratch",));
        let test_root = scratch.join("test_write_manifest");
        let _ = fs::remove_dir_all(&test_root);
        fs::create_dir_all(&test_root).expect("create test root");

        // Create dummy artifact files so build_manifest succeeds.
        // We must populate the repo-relative paths the corpus references.
        populate_artifacts(&test_root);

        let (jcs, digest) =
            crate::normative_manifest::write_manifest(&test_root).expect("write_manifest");
        assert!(!jcs.is_empty());
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
        let written = fs::read(test_root.join(MANIFEST_PATH)).expect("read written manifest");
        assert_eq!(written, jcs);
        assert_eq!(digest, super::sha256_hex(&jcs));

        // Clean up.
        let _ = fs::remove_dir_all(&test_root);
    }

    // -----------------------------------------------------------------------
    // verify error paths via crafted manifest files
    // -----------------------------------------------------------------------

    #[test]
    fn verify_rejects_missing_manifest() {
        use std::path::Path;
        let root = Path::new("/nonexistent");
        let err = super::verify(root).expect_err("expected Io error");
        assert!(matches!(err, ManifestError::Io { .. }));
    }

    #[test]
    fn verify_rejects_wrong_pinned_digest() {
        use std::fs;
        use std::path::Path;
        let scratch = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cache/scratch",));
        let test_root = scratch.join("test_verify_wrong_digest");
        let _ = fs::remove_dir_all(&test_root);
        fs::create_dir_all(&test_root).expect("create test root");
        let manifest_path = test_root.join(MANIFEST_PATH);
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        // Write garbage (not the real manifest).
        fs::write(&manifest_path, b"garbage bytes").expect("write garbage manifest");

        let err = super::verify(&test_root).expect_err("expected Io error");
        assert!(matches!(err, ManifestError::DigestMismatch { .. }));
        let _ = fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_reports_artifact_digest_mismatch() {
        // Use the actual manifest (real bytes = real pinned digest) to get
        // past the pinned-digest guard.  Place it in a scratch directory
        // alongside artifact files whose content we control.  When an
        // artifact's content differs from what the manifest records, verify
        // returns DigestMismatch.
        use std::fs;
        use std::path::Path;
        let scratch = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cache/scratch",));
        let test_root = scratch.join("test_verify_artifact_digest");
        let _ = fs::remove_dir_all(&test_root);
        fs::create_dir_all(&test_root).expect("create test root");

        // Copy the real committed manifest (byte-identical) into the scratch
        // tree so its digest equals PINNED_SPEC_CHECKSUM.
        let real_root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let real_manifest = fs::read(real_root.join(MANIFEST_PATH)).expect("read real manifest");
        let manifest_path = test_root.join(MANIFEST_PATH);
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&manifest_path, &real_manifest).expect("write manifest copy");

        // Populate the corpus paths with dummy content (NOT matching the
        // real artifact digests).  This guarantees every manifest entry
        // will mismatch.
        populate_artifacts(&test_root);
        for (_id, path) in crate::normative_manifest::artifact_entries() {
            let full = test_root.join(&path);
            fs::write(&full, b"wrong content").expect("write dummy artifact");
        }

        let err = super::verify(&test_root).expect_err("expected artifact digest mismatch");
        assert!(
            matches!(
                err,
                ManifestError::DigestMismatch { path, .. }
                    if super::artifact_entries()
                        .iter()
                        .any(|(_, p)| path == test_root.join(p))
            ),
            "expected an artifact digest mismatch for a known artifact path"
        );
        let _ = fs::remove_dir_all(&test_root);
    }

    // -----------------------------------------------------------------------
    // write_manifest I/O error paths (create_dir_all / fs::write failures)
    // -----------------------------------------------------------------------

    /// Populate every artifact referenced by the effective corpus
    /// (`ARTIFACTS` plus the OA-14 canonical ELD expansion) inside
    /// `test_root` with dummy content.
    fn populate_artifacts(test_root: &std::path::Path) {
        use std::fs;
        for (_id, path) in crate::normative_manifest::artifact_entries() {
            let full = test_root.join(&path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&full, b"dummy content").expect("write dummy artifact");
        }
    }

    fn scratch_root(name: &str) -> std::path::PathBuf {
        use std::path::Path;
        let scratch = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cache/scratch",));
        let root = scratch.join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test root");
        root
    }

    #[test]
    fn write_manifest_fails_when_path_is_a_directory() {
        use std::fs;
        let test_root = scratch_root("test_write_manifest_path_is_dir");
        populate_artifacts(&test_root);

        // `specs/registry/normative-manifest.json` exists as a directory, so
        // `fs::write` fails.
        let dir = test_root.join(MANIFEST_PATH);
        fs::create_dir_all(&dir).expect("create manifest dir");

        let err =
            crate::normative_manifest::write_manifest(&test_root).expect_err("expected Io error");
        assert!(matches!(err, ManifestError::Io { .. }));

        let _ = fs::remove_dir_all(&test_root);
    }

    // -----------------------------------------------------------------------
    // verify_manifest_bytes error paths (post-digest validation steps)
    // -----------------------------------------------------------------------

    #[test]
    fn verify_manifest_bytes_rejects_invalid_json() {
        let test_root = scratch_root("test_verify_bytes_invalid_json");
        // No artifacts needed: parse fails before the artifact loop.
        let err = super::verify_manifest_bytes(b"not json at all", &test_root)
            .expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_non_canonical_bytes() {
        let test_root = scratch_root("test_verify_bytes_non_canonical");
        // White-space-laden JSON parses but is not JCS-canonical, so the
        // re-canonicalization byte comparison fails.
        let bytes = b"{ \"artifacts\": [] }";
        let err =
            super::verify_manifest_bytes(bytes, &test_root).expect_err("expected DigestMismatch");
        assert!(matches!(err, ManifestError::DigestMismatch { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_missing_artifacts_array() {
        let test_root = scratch_root("test_verify_bytes_missing_artifacts");
        let err = super::verify_manifest_bytes(b"{\"schema\":\"x\"}", &test_root)
            .expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_io_error_on_missing_artifact() {
        use std::fs;
        let test_root = scratch_root("test_verify_bytes_missing_artifact");
        // Populate all but the first artifact; the disk_entries loop hits Io.
        let corpus = crate::normative_manifest::artifact_entries();
        let missing = corpus[0].1.clone();
        for (_id, path) in corpus.iter().skip(1) {
            let full = test_root.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&full, b"dummy content").expect("write dummy artifact");
        }
        assert!(
            !test_root.join(missing).exists(),
            "missing artifact must stay absent"
        );

        let err = super::verify_manifest_bytes(b"{\"artifacts\":[]}", &test_root)
            .expect_err("expected Io error");
        assert!(matches!(err, ManifestError::Io { .. }));
        let _ = fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_missing_entry_id() {
        let test_root = scratch_root("test_verify_bytes_missing_id");
        populate_artifacts(&test_root);
        let bytes = b"{\"artifacts\":[{\"path\":\"p\",\"sha256\":\"s\"}]}";
        let err =
            super::verify_manifest_bytes(bytes, &test_root).expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_missing_entry_path() {
        let test_root = scratch_root("test_verify_bytes_missing_path");
        populate_artifacts(&test_root);
        let bytes = b"{\"artifacts\":[{\"id\":\"OA-1\",\"sha256\":\"s\"}]}";
        let err =
            super::verify_manifest_bytes(bytes, &test_root).expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_missing_entry_sha256() {
        let test_root = scratch_root("test_verify_bytes_missing_sha");
        populate_artifacts(&test_root);
        let bytes = b"{\"artifacts\":[{\"id\":\"OA-1\",\"path\":\"p\"}]}";
        let err =
            super::verify_manifest_bytes(bytes, &test_root).expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_unknown_artifact_entry() {
        let test_root = scratch_root("test_verify_bytes_unknown_entry");
        populate_artifacts(&test_root);
        // Valid canonical manifest referencing an artifact ARTIFACTS has no
        // knowledge of: the per-entry lookup must fail.
        let bytes =
            b"{\"artifacts\":[{\"id\":\"OA-99\",\"path\":\"ghost.bin\",\"sha256\":\"aa\"}]}";
        let err =
            super::verify_manifest_bytes(bytes, &test_root).expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn verify_manifest_bytes_rejects_leftover_artifacts() {
        use std::fs;
        let test_root = scratch_root("test_verify_bytes_leftover");
        // All ARTIFACTS files exist on disk, but the manifest lists none of
        // them — the post-loop leftover branch fires.
        populate_artifacts(&test_root);
        let err = super::verify_manifest_bytes(b"{\"artifacts\":[]}", &test_root)
            .expect_err("expected Parse error");
        assert!(matches!(err, ManifestError::Parse { .. }));
        let _ = fs::remove_dir_all(&test_root);
    }
}
