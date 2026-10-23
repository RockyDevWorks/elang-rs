//! Generator for the OA-17 normative artifact manifest (T0123 mechanism).
//!
//! Running this regenerates, byte-for-byte:
//!
//! - `specs/registry/normative-manifest.json` — the RFC 8785 JCS canonical
//!   manifest of every ratified normative artifact in the v0.1.0 corpus,
//!   with each artifact's SHA-256.
//!
//! Nothing is hand-written: every digest is computed from the exact bytes on
//! disk by [`elang_conformance::normative_manifest`]. Run with:
//! `cargo run -p elang-conformance --bin gen_normative_manifest`.
//!
//! After regenerating, replace `PINNED_SPEC_CHECKSUM` in
//! `crates/elang-conformance/src/normative_manifest.rs` with the printed
//! digest and commit both changes together. The pinned digest is
//! provisional until final T0123, which runs after OA-14 lands (ADR 0011).
//!
//! This is a deterministic build-time developer tool over trusted repository
//! files (not untrusted archive bytes), so the workspace's untrusted-input
//! lints are relaxed here, mirroring `gen_binary_vectors`.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;

use elang_conformance::normative_manifest::{self, MANIFEST_PATH};

fn main() {
    let root = workspace_root();
    let (bytes, digest) = normative_manifest::write_manifest(&root).expect("write manifest");
    eprintln!(
        "wrote {MANIFEST_PATH}: {} artifacts, {} bytes, sha256 {digest}",
        normative_manifest::ARTIFACTS.len(),
        bytes.len()
    );
    eprintln!(
        "replace PINNED_SPEC_CHECKSUM in crates/elang-conformance/src/normative_manifest.rs \
         with:\n  {digest}"
    );
}

/// `crates/elang-conformance` -> workspace root.
fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}
