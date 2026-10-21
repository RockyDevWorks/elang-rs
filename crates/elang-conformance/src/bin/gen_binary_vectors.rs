//! Generator for the binary header / index / footer reference vectors and the
//! byte-exact minimal-archive fixture (T0118, open artifact OA-11).
//!
//! Running this reproduces, byte-for-byte:
//!
//! - `fixtures/valid/container/minimal-archive/` — the raw header, index,
//!   footer and the complete archive, each with a `.sha256` sidecar, plus an
//!   `expected.json` in the conformance format and a `layout.json` offset map.
//! - `specs/registry/binary-reference-vectors.json` — the human-reviewable,
//!   machine-checkable reference-vectors document.
//!
//! Every offset, size and SHA-256 is computed from the actual serialized bytes
//! by [`elang_conformance::binary_vectors`]; nothing is hand-written. Run with:
//! `cargo run -p elang-conformance --bin gen_binary_vectors`.
//!
//! This is a deterministic build-time developer tool operating over the
//! hard-coded, trusted minimal-archive layout (not untrusted archive bytes),
//! so the workspace's untrusted-input lints are relaxed here, mirroring
//! `elang-repertoire`'s `gen_repertoire`: a panic is the intended fail-closed
//! mode and the bounded arithmetic is over values already asserted in the
//! builder.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use elang_conformance::binary_vectors::{
    self, ReferenceArchive, ResourceLayout, ENTRY_FIXED_LEN, FOOTER_LEN, HEADER_LEN,
    INDEX_HEADER_LEN,
};

fn main() {
    let root = workspace_root();
    let a = binary_vectors::minimal_archive();

    write_fixture(&root, &a);
    write_reference_doc(&root, &a);

    eprintln!(
        "wrote binary reference vectors: {} resources, index {} bytes @ offset {}, archive {} bytes",
        a.resources.len(),
        a.index_size,
        a.index_offset,
        a.archive.len()
    );
}

/// `crates/elang-conformance` -> workspace root.
fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

/// A golden byte file plus its lowercase-hex `.sha256` sidecar.
fn write_golden(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write golden");
    let digest = binary_vectors::sha256(bytes);
    let sidecar = dir.join(format!("{name}.sha256"));
    // Sidecar: `<hex>  <name>\n`, matching common sha256sum format.
    let line = format!("{}  {name}\n", binary_vectors::hex(&digest));
    fs::write(&sidecar, line).expect("write sidecar");
}

fn write_fixture(root: &Path, a: &ReferenceArchive) {
    let dir = root.join("fixtures/valid/container/minimal-archive");
    fs::create_dir_all(&dir).expect("create fixture dir");

    // Raw golden byte blocks + the whole archive.
    write_golden(&dir, "header.bin", &a.header);
    write_golden(&dir, "index.bin", &a.index_bytes);
    write_golden(&dir, "footer.bin", &a.footer);
    write_golden(&dir, "archive.elang", &a.archive);

    // Conformance expected-result record (see fixtures/README.md).
    let archive_digest = binary_vectors::sha256(&a.archive);
    let expected = format!(
        r#"{{
  "fixture": "valid/container/minimal-archive",
  "description": "Minimal valid phoneme-only package layout: byte-exact binary header, index and terminal footer with a real SHA-256 over the serialized index bytes. Resource frame bodies are labelled placeholders standing in for the Phase 4 Zstd frames; the header/index/footer framing is fully normative.",
  "requirements": ["REQ-BIN-001", "REQ-BIN-003", "REQ-BIN-004", "REQ-BIN-006", "REQ-BIN-007", "REQ-BIN-008", "REQ-BIN-009", "REQ-BIN-010", "REQ-BIN-013", "REQ-BIN-014", "REQ-BIN-015", "REQ-BIN-016"],
  "input": "archive.elang",
  "modes": ["inspect"],
  "outcome": "pass",
  "golden": [
    {{ "path": "header.bin", "sha256": "{}" }},
    {{ "path": "index.bin", "sha256": "{}" }},
    {{ "path": "footer.bin", "sha256": "{}" }},
    {{ "path": "archive.elang", "sha256": "{}" }}
  ]
}}
"#,
        binary_vectors::hex(&binary_vectors::sha256(&a.header)),
        binary_vectors::hex(&binary_vectors::sha256(&a.index_bytes)),
        binary_vectors::hex(&binary_vectors::sha256(&a.footer)),
        binary_vectors::hex(&archive_digest),
    );
    fs::write(dir.join("expected.json"), expected).expect("write expected.json");

    // A human-readable offset map of the whole archive.
    let mut layout = String::new();
    layout.push_str("{\n");
    let _ = writeln!(layout, "  \"archive_size\": {},", a.archive.len());
    let _ = writeln!(
        layout,
        "  \"header\": {{ \"offset\": 0, \"length\": {HEADER_LEN} }},"
    );
    layout.push_str("  \"frames\": [\n");
    for (i, r) in a.resources.iter().enumerate() {
        let comma = if i + 1 < a.resources.len() { "," } else { "" };
        let _ = writeln!(
            layout,
            "    {{ \"path\": \"{}\", \"frame_offset\": {}, \"compressed_size\": {}, \"uncompressed_size\": {}, \"compression\": {} }}{comma}",
            r.path, r.frame_offset, r.compressed_size, r.uncompressed_size, r.compression
        );
    }
    layout.push_str("  ],\n");
    let _ = writeln!(
        layout,
        "  \"index\": {{ \"offset\": {}, \"length\": {} }},",
        a.index_offset, a.index_size
    );
    let _ = writeln!(
        layout,
        "  \"footer\": {{ \"offset\": {}, \"length\": {FOOTER_LEN} }},",
        a.index_offset + a.index_size
    );
    let _ = writeln!(
        layout,
        "  \"index_sha256\": \"{}\"",
        binary_vectors::hex(&a.index_sha256)
    );
    layout.push_str("}\n");
    fs::write(dir.join("layout.json"), layout).expect("write layout.json");
}

/// Render a byte slice as a JSON array of `"0xNN"` strings (reviewable hex).
fn bytes_to_hex_array(bytes: &[u8], indent: &str) -> String {
    // 8 bytes per line for readability.
    let mut out = String::from("[\n");
    for (i, chunk) in bytes.chunks(8).enumerate() {
        out.push_str(indent);
        out.push_str("  ");
        let parts: Vec<String> = chunk.iter().map(|b| format!("\"{b:02X}\"")).collect();
        out.push_str(&parts.join(", "));
        let last = (i + 1) * 8 >= bytes.len();
        if !last {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(indent);
    out.push(']');
    out
}

fn write_reference_doc(root: &Path, a: &ReferenceArchive) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"metadata\": {\n");
    out.push_str("    \"vector_set\": \"binary-reference-vectors\",\n");
    out.push_str("    \"vector_set_version\": 1,\n");
    out.push_str("    \"container_version\": \"0.1.0\",\n");
    out.push_str("    \"endianness\": \"little\",\n");
    out.push_str(
        "    \"generator\": \"cargo run -p elang-conformance --bin gen_binary_vectors\",\n",
    );
    out.push_str("    \"fixture\": \"fixtures/valid/container/minimal-archive\",\n");
    out.push_str(
        "    \"steering_reference\": \"\\u00a73 (binary container physical contract)\",\n",
    );
    out.push_str("    \"requirements\": [\"REQ-BIN-001\", \"REQ-BIN-003\", \"REQ-BIN-004\", \"REQ-BIN-006\", \"REQ-BIN-007\", \"REQ-BIN-008\", \"REQ-BIN-009\", \"REQ-BIN-010\", \"REQ-BIN-013\", \"REQ-BIN-014\", \"REQ-BIN-015\", \"REQ-BIN-016\"],\n");
    out.push_str("    \"provenance\": \"Byte-exact, independently reproducible header/index/footer reference vectors. Every offset, size and SHA-256 is computed from the exact little-endian bytes by elang_conformance::binary_vectors; no digest or offset is hand-written. Resource frame bodies are deterministic placeholders (ASCII 'ELANGFRAME:<path>#' padded with '.') standing in for the Phase 4 Zstd frames; the header/index/footer framing is fully normative. No fabricated hashes (steering \\u00a70.3; ADR 0003).\"\n");
    out.push_str("  },\n");

    // Header field table + raw bytes.
    out.push_str("  \"header\": {\n");
    out.push_str("    \"length\": 32,\n");
    out.push_str("    \"fields\": [\n");
    out.push_str("      { \"offset\": 0, \"width\": 8, \"encoding\": \"raw\", \"name\": \"magic\", \"value_hex\": \"45 4C 41 4E 47 0D 0A 1A\", \"note\": \"ELANG\\\\r\\\\n\\\\x1A\" },\n");
    out.push_str("      { \"offset\": 8, \"width\": 2, \"encoding\": \"u16le\", \"name\": \"major\", \"value\": 0 },\n");
    out.push_str("      { \"offset\": 10, \"width\": 2, \"encoding\": \"u16le\", \"name\": \"minor\", \"value\": 1 },\n");
    out.push_str("      { \"offset\": 12, \"width\": 4, \"encoding\": \"u32le\", \"name\": \"flags\", \"value\": 0 },\n");
    out.push_str("      { \"offset\": 16, \"width\": 16, \"encoding\": \"raw\", \"name\": \"reserved\", \"value\": \"all zero\" }\n");
    out.push_str("    ],\n");
    let _ = writeln!(
        out,
        "    \"bytes\": {},",
        bytes_to_hex_array(&a.header, "    ")
    );
    let _ = writeln!(
        out,
        "    \"sha256\": \"{}\"",
        binary_vectors::hex(&binary_vectors::sha256(&a.header))
    );
    out.push_str("  },\n");

    // Index header + entries.
    out.push_str("  \"index\": {\n");
    let _ = writeln!(out, "    \"offset\": {},", a.index_offset);
    let _ = writeln!(out, "    \"length\": {},", a.index_size);
    out.push_str("    \"header\": {\n");
    out.push_str("      \"length\": 16,\n");
    out.push_str("      \"fields\": [\n");
    out.push_str("        { \"offset\": 0, \"width\": 8, \"encoding\": \"raw\", \"name\": \"magic\", \"value_ascii\": \"ELIDX001\" },\n");
    out.push_str("        { \"offset\": 8, \"width\": 2, \"encoding\": \"u16le\", \"name\": \"index_version\", \"value\": 1 },\n");
    out.push_str("        { \"offset\": 10, \"width\": 2, \"encoding\": \"u16le\", \"name\": \"entry_format\", \"value\": 1 },\n");
    let _ = writeln!(
        out,
        "        {{ \"offset\": 12, \"width\": 4, \"encoding\": \"u32le\", \"name\": \"resource_count\", \"value\": {} }}",
        a.resources.len()
    );
    out.push_str("      ]\n");
    out.push_str("    },\n");
    out.push_str("    \"entry_layout\": [\n");
    out.push_str("      { \"field\": \"path_length\", \"width\": 2, \"encoding\": \"u16le\" },\n");
    out.push_str(
        "      { \"field\": \"path\", \"width\": \"path_length\", \"encoding\": \"utf8-nfc\" },\n",
    );
    out.push_str("      { \"field\": \"frame_offset\", \"width\": 8, \"encoding\": \"u64le\" },\n");
    out.push_str(
        "      { \"field\": \"compressed_size\", \"width\": 8, \"encoding\": \"u64le\" },\n",
    );
    out.push_str(
        "      { \"field\": \"uncompressed_size\", \"width\": 8, \"encoding\": \"u64le\" },\n",
    );
    out.push_str(
        "      { \"field\": \"compression\", \"width\": 1, \"encoding\": \"u8\", \"value\": 1 }\n",
    );
    out.push_str("    ],\n");
    out.push_str("    \"entries\": [\n");
    for (i, r) in a.resources.iter().enumerate() {
        write_entry(&mut out, r, i + 1 == a.resources.len());
    }
    out.push_str("    ],\n");
    let _ = writeln!(
        out,
        "    \"bytes\": {},",
        bytes_to_hex_array(&a.index_bytes, "    ")
    );
    let _ = writeln!(
        out,
        "    \"sha256\": \"{}\"",
        binary_vectors::hex(&a.index_sha256)
    );
    out.push_str("  },\n");

    // Footer.
    out.push_str("  \"footer\": {\n");
    let _ = writeln!(out, "    \"offset\": {},", a.index_offset + a.index_size);
    out.push_str("    \"length\": 64,\n");
    out.push_str("    \"fields\": [\n");
    out.push_str("      { \"offset\": 0, \"width\": 8, \"encoding\": \"raw\", \"name\": \"magic\", \"value_ascii\": \"ELEND001\" },\n");
    let _ = writeln!(
        out,
        "      {{ \"offset\": 8, \"width\": 8, \"encoding\": \"u64le\", \"name\": \"index_offset\", \"value\": {} }},",
        a.index_offset
    );
    let _ = writeln!(
        out,
        "      {{ \"offset\": 16, \"width\": 8, \"encoding\": \"u64le\", \"name\": \"index_size\", \"value\": {} }},",
        a.index_size
    );
    let _ = writeln!(
        out,
        "      {{ \"offset\": 24, \"width\": 32, \"encoding\": \"raw\", \"name\": \"index_sha256\", \"value_hex\": \"{}\" }},",
        binary_vectors::hex(&a.index_sha256)
    );
    out.push_str("      { \"offset\": 56, \"width\": 8, \"encoding\": \"raw\", \"name\": \"reserved\", \"value\": \"all zero\" }\n");
    out.push_str("    ],\n");
    let _ = writeln!(
        out,
        "    \"bytes\": {},",
        bytes_to_hex_array(&a.footer, "    ")
    );
    let _ = writeln!(
        out,
        "    \"sha256\": \"{}\"",
        binary_vectors::hex(&binary_vectors::sha256(&a.footer))
    );
    out.push_str("  },\n");

    // The framing relation, worked out against the concrete archive.
    out.push_str("  \"framing_relation\": {\n");
    out.push_str("    \"rule\": \"index_offset + index_size == file_size - 64\",\n");
    let file_size = a.archive.len() as u64;
    let _ = writeln!(out, "    \"index_offset\": {},", a.index_offset);
    let _ = writeln!(out, "    \"index_size\": {},", a.index_size);
    let _ = writeln!(out, "    \"footer_length\": {FOOTER_LEN},");
    let _ = writeln!(out, "    \"file_size\": {file_size},");
    let _ = writeln!(
        out,
        "    \"check\": \"{} + {} == {} - 64\",",
        a.index_offset, a.index_size, file_size
    );
    let _ = writeln!(
        out,
        "    \"holds\": {}",
        a.index_offset + a.index_size == file_size - FOOTER_LEN
    );
    out.push_str("  },\n");

    // Archive digest for completeness.
    let _ = writeln!(
        out,
        "  \"archive_sha256\": \"{}\"",
        binary_vectors::hex(&binary_vectors::sha256(&a.archive))
    );
    out.push_str("}\n");

    let path = root.join("specs/registry/binary-reference-vectors.json");
    fs::write(&path, &out).expect("write binary-reference-vectors.json");

    // Sanity: the JSON must parse (the generator fails closed otherwise).
    let _: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("generated reference-vectors JSON is invalid: {e}"));

    assert_eq!(
        INDEX_HEADER_LEN + entries_len_check(a),
        a.index_size,
        "index size == header + entries"
    );
}

fn entries_len_check(a: &ReferenceArchive) -> u64 {
    a.resources
        .iter()
        .map(|r| ENTRY_FIXED_LEN + r.path.len() as u64)
        .sum()
}

fn write_entry(out: &mut String, r: &ResourceLayout, last: bool) {
    let comma = if last { "" } else { "," };
    let _ = writeln!(out, "      {{");
    let _ = writeln!(out, "        \"path\": \"{}\",", r.path);
    let _ = writeln!(out, "        \"path_length\": {},", r.path.len());
    let _ = writeln!(out, "        \"frame_offset\": {},", r.frame_offset);
    let _ = writeln!(out, "        \"compressed_size\": {},", r.compressed_size);
    let _ = writeln!(
        out,
        "        \"uncompressed_size\": {},",
        r.uncompressed_size
    );
    let _ = writeln!(out, "        \"compression\": {}", r.compression);
    let _ = writeln!(out, "      }}{comma}");
}
