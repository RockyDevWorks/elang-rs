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
/// This value pins the *mechanism* for the v0.1.0 freeze; it is **not** a
/// declaration that v0.1.0 is frozen. The corpus it covers is the ratified
/// OA-1..OA-16 artifacts minus OA-14, which is still open. ADR 0011 requires
/// the manifest and this constant to be regenerated during final T0123, once
/// OA-14 (canonical ELD goldens) has landed and entered the corpus. The
/// release-tag half of OA-17 stays blocked by OA-14 and is not implemented
/// here.
pub const PINNED_SPEC_CHECKSUM: &str =
    "f50bf40763dd0599311ea0948f07a10107d799fcca4ce6a05a43e7b8b145a8fa";

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
/// escaped per RFC 8785 §3.2.2.2 (control characters must be escaped using
/// `\uXXXX`; all other characters may pass through).
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
    // Iterate by Unicode scalar value so we escape control characters and
    // surrogate halves consistently with RFC 8785.
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
            c => {
                // Non-BMP characters are emitted by `chars()` as their
                // original scalar; serde_json/JCS require surrogate-pair
                // encoding for them in JSON strings.
                if (c as u32) >= 0x10000 {
                    let cp = c as u32 - 0x10000;
                    let hi = 0xD800 + (cp >> 10);
                    let lo = 0xDC00 + (cp & 0x3FF);
                    out.push_str(&format!("\\u{:04x}\\u{:04x}", hi, lo));
                } else {
                    out.push(c);
                }
            }
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
    let mut entries: Vec<(String, String, String)> = Vec::with_capacity(ARTIFACTS.len());
    for a in ARTIFACTS {
        let full = release_root.join(a.path);
        let bytes = fs::read(&full).map_err(|e| ManifestError::Io {
            path: full.clone(),
            message: e.to_string(),
        })?;
        entries.push((a.id.to_string(), a.path.to_string(), sha256_hex(&bytes)));
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
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ManifestError::Io {
            path: parent.to_path_buf(),
            message: e.to_string(),
        })?;
    }
    fs::write(&manifest_path, &jcs).map_err(|e| ManifestError::Io {
        path: manifest_path.clone(),
        message: e.to_string(),
    })?;
    let digest = sha256_hex(&jcs);
    Ok((jcs, digest))
}

/// Verify the committed manifest against the artifact corpus on disk.
///
/// 1. Reads the committed manifest file.
/// 2. Parses it and re-canonicalizes to JCS bytes.
/// 3. Checks the recomputed JCS digest equals `PINNED_SPEC_CHECKSUM`; a
///    mismatch is a [`ManifestError::DigestMismatch`].
/// 4. For each artifact entry, re-hashes the file on disk and checks it
///    equals the entry's recorded `sha256`.
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

    // Parse and re-canonicalize the committed manifest; the result must be
    // byte-identical to the committed bytes.
    let value: serde_json::Value =
        serde_json::from_slice(&committed).map_err(|e| ManifestError::Parse {
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
    for a in ARTIFACTS {
        let full = release_root.join(a.path);
        let bytes = fs::read(&full).map_err(|e| ManifestError::Io {
            path: full.clone(),
            message: e.to_string(),
        })?;
        disk_entries.insert((a.id.to_string(), a.path.to_string()), sha256_hex(&bytes));
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
                    "artifact `{}` ({}) is in the manifest but not in ARTIFACTS",
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
                "ARTIFACTS contains entries not present in the manifest: {}",
                leftover.join(", ")
            ),
        });
    }
    Ok(())
}
