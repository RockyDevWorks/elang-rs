//! Generator for the OA-14 canonical ELD golden corpus (Phase 11, T1121).
//!
//! Running this reproduces, byte-for-byte:
//!
//! - `fixtures/valid/canonical/<case>/` — the authored `input.eld`, the
//!   byte-exact `golden/canonical.eld`, its `.sha256` sidecar and the
//!   conformance `expected.json` record for each published case.
//! - `specs/registry/eld-canonical-vectors.json` — the human-reviewable,
//!   machine-checkable vector document publishing every canonical golden's
//!   SHA-256.
//!
//! Every golden byte and every digest is produced by the real pipeline
//! (`elang_eld::parse` → `inventory::from_document` → `canon::to_string`);
//! nothing is hand-written. Run with:
//! `cargo run -p elang-conformance --bin gen_canonical_goldens`.
//!
//! This is a deterministic build-time developer tool operating over
//! hard-coded, trusted ELD sources, so the workspace's untrusted-input lints
//! are relaxed here, mirroring `gen_binary_vectors`: a panic is the intended
//! fail-closed mode.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::arithmetic_side_effects
)]

use std::fs;
use std::path::{Path, PathBuf};

use elang_conformance::binary_vectors::{hex, sha256};
use elang_conformance::canonical_goldens::{self, GeneratedGolden};

/// Repository-relative path of the published vector document.
const VECTORS_PATH: &str = "specs/registry/eld-canonical-vectors.json";

fn main() {
    let root = workspace_root();
    let rep = elang_repertoire::repertoire::load().expect("pinned repertoire loads");
    let generated = canonical_goldens::generate(&rep).expect("golden corpus generates");

    write_fixtures(&root, &generated);
    write_vector_doc(&root, &generated);

    eprintln!("wrote canonical ELD goldens: {} cases", generated.len());
}

/// `crates/elang-conformance` -> workspace root.
fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

fn write_fixtures(root: &Path, generated: &[GeneratedGolden]) {
    for golden in generated {
        let dir = root.join("fixtures/valid/canonical").join(golden.id);
        let golden_dir = dir.join("golden");
        fs::create_dir_all(&golden_dir).expect("create fixture dir");

        // The authored source (an input, not a golden).
        fs::write(dir.join("input.eld"), golden.source).expect("write input.eld");

        // The byte-exact golden output plus its digest sidecar.
        fs::write(
            golden_dir.join("canonical.eld"),
            golden.canonical.as_bytes(),
        )
        .expect("write canonical.eld");
        let sidecar = format!("{}  canonical.eld\n", golden.sha256);
        fs::write(golden_dir.join("canonical.eld.sha256"), sidecar).expect("write sidecar");

        // The conformance expected-result record (see fixtures/README.md).
        let requirements = golden
            .requirements
            .iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let expected = format!(
            r#"{{
  "fixture": "valid/canonical/{id}",
  "description": "{description}",
  "requirements": [{requirements}],
  "input": "input.eld",
  "modes": ["inspect", "validate"],
  "outcome": "pass",
  "golden": [
    {{ "path": "golden/canonical.eld", "sha256": "{sha256}" }}
  ]
}}
"#,
            id = golden.id,
            description = json_escape(golden.description),
            requirements = requirements,
            sha256 = golden.sha256,
        );
        fs::write(dir.join("expected.json"), expected).expect("write expected.json");
    }
}

/// Escape a string for a JSON string literal (RFC 8785 §3.2.2.2 subset:
/// quote, backslash and the control characters below U+0020).
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
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
    out
}

/// Write the published vector document.
fn write_vector_doc(root: &Path, generated: &[GeneratedGolden]) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"metadata\": {\n");
    out.push_str("    \"vector_set\": \"eld-canonical-vectors\",\n");
    out.push_str("    \"vector_set_version\": 1,\n");
    out.push_str("    \"eld_version\": \"0.1.0\",\n");
    out.push_str(
        "    \"generator\": \"cargo run -p elang-conformance --bin gen_canonical_goldens\",\n",
    );
    out.push_str("    \"fixture_root\": \"fixtures/valid/canonical\",\n");
    out.push_str(
        "    \"steering_reference\": \"\\u00a79 and \\u00a79.1 (canonical ELD serialization)\",\n",
    );
    out.push_str("    \"requirements\": [\"REQ-CANON-001\", \"REQ-CANON-002\", \"REQ-CANON-003\", \"REQ-CANON-004\", \"REQ-CANON-005\", \"REQ-CANON-006\", \"REQ-CANON-007\", \"REQ-CANON-008\", \"REQ-CANON-009\"],\n");
    out.push_str("    \"provenance\": \"Byte-exact, independently reproducible canonical ELD goldens. Every golden file and every SHA-256 is computed by the real pipeline (elang_eld::parse -> inventory::from_document -> canon::to_string); no digest is hand-written. The authored input.eld sources are deliberately non-canonical so the goldens exercise the ordering rules rather than merely echoing the input (steering \\u00a70.3; ADR 0003).\"\n");
    out.push_str("  },\n");
    out.push_str("  \"vectors\": [\n");

    for (index, golden) in generated.iter().enumerate() {
        let comma = if index + 1 == generated.len() {
            ""
        } else {
            ","
        };
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": \"{}\",\n", golden.id));
        out.push_str(&format!(
            "      \"description\": \"{}\",\n",
            json_escape(golden.description)
        ));
        out.push_str(&format!(
            "      \"fixture\": \"valid/canonical/{}\",\n",
            golden.id
        ));
        out.push_str("      \"input\": \"input.eld\",\n");
        out.push_str("      \"golden\": \"golden/canonical.eld\",\n");
        out.push_str(&format!("      \"sha256\": \"{}\",\n", golden.sha256));
        out.push_str(&format!(
            "      \"canonical_size\": {},\n",
            golden.canonical.len()
        ));
        let requirements = golden
            .requirements
            .iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("      \"requirements\": [{requirements}]\n"));
        out.push_str(&format!("    }}{comma}\n"));
    }

    out.push_str("  ]\n");
    out.push_str("}\n");

    let path = root.join(VECTORS_PATH);
    fs::write(&path, &out).expect("write eld-canonical-vectors.json");

    // Sanity: the JSON must parse (the generator fails closed otherwise).
    let _: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("generated vector JSON is invalid: {e}"));

    // Sanity: the published digest must match the bytes on disk.
    for golden in generated {
        let path = root
            .join("fixtures/valid/canonical")
            .join(golden.id)
            .join("golden/canonical.eld");
        let bytes = fs::read(&path).expect("read golden");
        assert_eq!(
            hex(&sha256(&bytes)),
            golden.sha256,
            "case `{}` digest does not match the bytes on disk",
            golden.id
        );
    }
}
