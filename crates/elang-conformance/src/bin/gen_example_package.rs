//! Generator for the published example language package (T1610).
//!
//! Running this regenerates, byte-for-byte:
//!
//! - `examples/hello-en/hello-en.elang` — the canonical `.elang` artifact
//!   built from the published example sources.
//! - `examples/hello-en/hello-en.elang.sha256` — the digest sidecar.
//! - `examples/hello-en/expected.json` — the conformance expected-result
//!   record for the example.
//!
//! Nothing is hand-written: the archive bytes are produced by the production
//! writer ([`elang_writer::write_archive`]) over the **published source
//! files** in `examples/hello-en/`, and the digest is computed from those
//! bytes. Run with:
//! `cargo run -p elang-conformance --bin gen_example_package`.
//!
//! The example is built with the **canonical** compression profile
//! (`elang-canonical-zstd-v1`), so repeated runs — and runs on any supported
//! platform — produce byte-identical output. The integration test
//! `crates/elang-conformance/tests/example_package.rs` asserts that the
//! committed artifact still matches a fresh generation, so a drift between
//! the published sources and the published archive fails CI.
//!
//! This is a deterministic build-time developer tool operating over trusted
//! repository files, so the workspace's untrusted-input lints are relaxed
//! here, mirroring `gen_canonical_goldens`.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::arithmetic_side_effects
)]

use std::fs;
use std::path::{Path, PathBuf};

use elang_conformance::binary_vectors::{hex, sha256};
use elang_writer::{write_archive, PackageConfig, Profile};

/// Repository-relative path of the example's EPF source.
const EPF_PATH: &str = "examples/hello-en/package.epf";

/// Repository-relative path of the example's ELD source.
const ELD_PATH: &str = "examples/hello-en/Linguistics/phonemes.eld";

/// Repository-relative path of the published canonical artifact.
const ARCHIVE_PATH: &str = "examples/hello-en/hello-en.elang";

/// Repository-relative path of the digest sidecar.
const SIDECAR_PATH: &str = "examples/hello-en/hello-en.elang.sha256";

/// Repository-relative path of the conformance expected-result record.
const EXPECTED_PATH: &str = "examples/hello-en/expected.json";

fn main() {
    let root = workspace_root();
    // The published archive, sidecar and conformance expected-result
    // record are written here by default. Tests and release scripts can
    // redirect the writes with `--out-dir <path>` so they never mutate
    // tracked repository files.
    let out_dir = out_dir_from_args().unwrap_or_else(|| root.clone());
    let bytes = build(&root).expect("example package builds");

    let digest = hex(&sha256(&bytes));
    let out_examples = out_dir.join(Path::new(ARCHIVE_PATH).parent().unwrap_or(Path::new(".")));
    fs::create_dir_all(&out_examples).expect("create output directory");

    let archive_path = out_examples.join(file_name(ARCHIVE_PATH));
    write_if_changed(&archive_path, &bytes);

    let sidecar = format!("{digest}  {}\n", file_name(ARCHIVE_PATH));
    write_if_changed(
        &out_examples.join(file_name(SIDECAR_PATH)),
        sidecar.as_bytes(),
    );

    let expected = format!(
        r#"{{
  "fixture": "valid/example/hello-en",
  "description": "Published worked example: a complete ELANG v0.1.0 language package packed from the sources in `examples/hello-en/` with the canonical profile.",
  "requirements": ["REQ-PKG-001", "REQ-PKG-004"],
  "input": "hello-en.elang",
  "modes": ["inspect", "verify", "validate"],
  "outcome": "pass",
  "golden": [
    {{ "path": "hello-en.elang", "sha256": "{digest}" }}
  ]
}}
"#
    );
    write_if_changed(
        &out_examples.join(file_name(EXPECTED_PATH)),
        expected.as_bytes(),
    );

    eprintln!(
        "wrote {ARCHIVE_PATH}: {} bytes, sha256 {digest}",
        bytes.len()
    );
}

/// The file-name component of a repository-relative path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Parse `--out-dir <path>`.
///
/// The generator writes its three published artefacts into the workspace
/// root by default. Passing `--out-dir <path>` redirects the writes to
/// `<path>/examples/hello-en/`, which is how the integration test in
/// `crates/elang-conformance/tests/example_package.rs` runs the generator
/// without mutating the repository.
fn out_dir_from_args() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--out-dir" {
            let path = args.next().expect("--out-dir requires a path argument");
            return Some(std::path::PathBuf::from(path));
        }
        if let Some(value) = arg.strip_prefix("--out-dir=") {
            return Some(std::path::PathBuf::from(value));
        }
    }
    None
}

/// Write `bytes` to `path` only when the on-disk contents differ.
///
/// Mirrors `gen_conformance_corpus::write_if_changed`: an identical
/// rewrite still bumps the file's mtime, which makes the reproducible-
/// build CI gate see spurious drift between two otherwise-identical
/// clean builds.
fn write_if_changed(path: &std::path::Path, bytes: &[u8]) {
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return;
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(path, bytes).expect("write file");
}

/// Build the canonical example archive from the published sources.
fn build(root: &Path) -> Result<Vec<u8>, String> {
    let epf_bytes = fs::read(root.join(EPF_PATH)).map_err(|e| format!("read {EPF_PATH}: {e}"))?;
    let eld_bytes = fs::read(root.join(ELD_PATH)).map_err(|e| format!("read {ELD_PATH}: {e}"))?;

    let epf = elang_epf::parse(&epf_bytes).map_err(|e| format!("parse {EPF_PATH}: {e}"))?;
    let rep =
        elang_repertoire::repertoire::load().map_err(|e| format!("load repertoire: {e:?}"))?;
    let doc = elang_eld::parse(&eld_bytes, &rep).map_err(|e| format!("parse {ELD_PATH}: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("validate {ELD_PATH}: {e}"))?;

    let config = PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile: Profile::Canonical,
        format_version: "0.1.0".to_string(),
    };

    write_archive(&config).map_err(|e| format!("write archive: [{}] {e}", e.code()))
}

/// `crates/elang-conformance` -> workspace root.
fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}
