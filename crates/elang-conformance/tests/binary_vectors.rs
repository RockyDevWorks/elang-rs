//! Conformance checks over the published binary header/index/footer reference
//! vectors and the byte-exact minimal-archive fixture (T0118, OA-11).
//!
//! These tests prove the committed artifacts under
//! `fixtures/valid/container/minimal-archive/` and
//! `specs/registry/binary-reference-vectors.json` match, byte-for-byte, the
//! layout the deterministic builder produces — and that every published
//! SHA-256 is the *real* digest of the bytes it describes. If the builder, the
//! steering layout, or a committed golden drifts, these fail closed.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use std::path::PathBuf;

use elang_conformance::binary_vectors::{self, FOOTER_LEN};
use sha2::{Digest, Sha256};

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

fn read_fixture(name: &str) -> Vec<u8> {
    let p = workspace_root()
        .join("fixtures/valid/container/minimal-archive")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[test]
fn committed_fixtures_match_the_builder_byte_for_byte() {
    let a = binary_vectors::minimal_archive();
    assert_eq!(read_fixture("header.bin"), a.header, "header.bin drift");
    assert_eq!(read_fixture("index.bin"), a.index_bytes, "index.bin drift");
    assert_eq!(read_fixture("footer.bin"), a.footer, "footer.bin drift");
    assert_eq!(read_fixture("archive.elang"), a.archive, "archive drift");
}

#[test]
fn sidecar_digests_are_real() {
    for name in ["header.bin", "index.bin", "footer.bin", "archive.elang"] {
        let bytes = read_fixture(name);
        let want = binary_vectors::hex(&sha256(&bytes));
        let sidecar = read_fixture(&format!("{name}.sha256"));
        let text = String::from_utf8(sidecar).expect("sidecar utf8");
        let got = text.split_whitespace().next().expect("hex field");
        assert_eq!(got, want, "{name}.sha256 does not match the file bytes");
    }
}

#[test]
fn footer_embeds_the_real_index_digest() {
    let footer = read_fixture("footer.bin");
    let index = read_fixture("index.bin");
    assert_eq!(&footer[0..8], b"ELEND001", "footer magic");
    let embedded = &footer[24..56];
    assert_eq!(
        embedded,
        &sha256(&index),
        "footer index hash != SHA-256(index)"
    );
}

#[test]
fn framing_relation_holds_for_the_fixture() {
    let archive = read_fixture("archive.elang");
    let footer = read_fixture("footer.bin");
    let file_size = archive.len() as u64;
    let index_offset = u64::from_le_bytes(footer[8..16].try_into().expect("8 bytes"));
    let index_size = u64::from_le_bytes(footer[16..24].try_into().expect("8 bytes"));
    assert_eq!(
        index_offset + index_size,
        file_size - FOOTER_LEN,
        "index_offset + index_size == file_size - 64"
    );
    // The footer sits exactly at the end of the index.
    assert_eq!(index_offset + index_size, file_size - FOOTER_LEN);
    // The archive ends with the committed footer.
    assert_eq!(&archive[archive.len() - 64..], &footer[..]);
}

#[test]
fn reference_doc_digests_match_the_bytes() {
    let doc = workspace_root().join("specs/registry/binary-reference-vectors.json");
    let src = std::fs::read_to_string(&doc).expect("read reference doc");
    let v: serde_json::Value = serde_json::from_str(&src).expect("reference doc parses");

    let a = binary_vectors::minimal_archive();

    // The document's index SHA-256 must equal the real digest of the index.
    let doc_index_hash = v["index"]["sha256"].as_str().expect("index sha256");
    assert_eq!(doc_index_hash, binary_vectors::hex(&a.index_sha256));

    // Offsets and sizes agree with the builder.
    assert_eq!(v["index"]["offset"].as_u64(), Some(a.index_offset));
    assert_eq!(v["index"]["length"].as_u64(), Some(a.index_size));
    assert_eq!(
        v["framing_relation"]["file_size"].as_u64(),
        Some(a.archive.len() as u64)
    );
    assert_eq!(v["framing_relation"]["holds"].as_bool(), Some(true));
}
