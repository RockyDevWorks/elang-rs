//! Verifies the published worked example (`examples/hello-en/`) stays
//! reproducible and round-trips through the full reader pipeline
//! (T1610).
//!
//! Three invariants:
//!
//!
//! 1. **Byte-identity with a fresh build.** Re-packing the example sources
//!    with the canonical profile must produce bytes byte-identical to the
//!    committed `hello-en.elang`. The committed `.sha256` sidecar must match
//!    the digest of those bytes. If a future change to a pinned source file
//!    (or to the writer) silently alters the canonical output, this test
//!    fails before the artifact ships.
//!
//! 2. **Sidecar / `expected.json` consistency.** The committed sidecar
//!    digest equals the `expected.json` golden's `sha256`. The fixture is
//!    self-consistent — a hand-edited sidecar or `expected.json` would fail
//!    here.
//!
//! 3. **Full pipeline round-trip.** Opening the committed archive through
//!    the production reader must succeed under `inspect`, `verify` and
//!    `validate`, and the resource listing must be exactly the five
//!    mandatory resources (`Header/mimetype`, `Header/format.version`,
//!    `Linguistics/phonemes.eld`, `META-INF/integrity.kdl`, `package.epf`).
//!
//! The test never hand-writes a digest: it rebuilds the archive and the
//! sidecar from the source files via the production writer, then compares.
//! The integration test in `crates/elang-conformance/tests/normative_manifest.rs`
//! has the analogous property for the normative manifest.

#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use elang_conformance::binary_vectors::{hex, sha256};
use elang_reader::Package;
use elang_writer::{write_archive, PackageConfig, Profile};

/// Repository root resolved from the `CARGO_MANIFEST_DIR` of this crate.
fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../..");
    p
}

fn read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Build the example archive from the published sources and return the
/// canonical bytes.
fn build_canonical_example() -> Vec<u8> {
    let root = repo_root();
    let epf_bytes = read(&root.join("examples/hello-en/package.epf"));
    let eld_bytes = read(&root.join("examples/hello-en/Linguistics/phonemes.eld"));

    let epf = elang_epf::parse(&epf_bytes).expect("parse example EPF");
    let rep = elang_repertoire::repertoire::load().expect("load pinned repertoire");
    let doc = elang_eld::parse(&eld_bytes, &rep).expect("parse example ELD");
    let eld = elang_eld::inventory::from_document(&doc, &rep).expect("example ELD validates");

    let config = PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile: Profile::Canonical,
        format_version: "0.1.0".to_string(),
    };
    write_archive(&config).expect("write example archive")
}

#[test]
fn example_package_is_byte_identical_to_a_fresh_canonical_build() {
    let root = repo_root();
    let committed = read(&root.join("examples/hello-en/hello-en.elang"));
    let fresh = build_canonical_example();
    assert_eq!(
        committed, fresh,
        "committed example.elang drifted from a fresh canonical build — \
         re-run `cargo run -p elang-conformance --bin gen_example_package` \
         and commit the result"
    );
}

#[test]
fn example_package_sidecar_matches_the_committed_archive() {
    let root = repo_root();
    let bytes = read(&root.join("examples/hello-en/hello-en.elang"));
    let digest = hex(&sha256(&bytes));

    let sidecar = std::fs::read_to_string(root.join("examples/hello-en/hello-en.elang.sha256"))
        .expect("read sidecar");
    let expected_line = format!("{digest}  hello-en.elang\n");
    assert_eq!(
        sidecar, expected_line,
        "sidecar does not match the actual digest of the committed archive"
    );
}

#[test]
fn example_package_expected_json_matches_the_sidecar_digest() {
    let root = repo_root();
    let bytes = read(&root.join("examples/hello-en/hello-en.elang"));
    let digest = hex(&sha256(&bytes));

    let expected: serde_json::Value =
        serde_json::from_slice(&read(&root.join("examples/hello-en/expected.json")))
            .expect("parse expected.json");
    let recorded = expected
        .get("golden")
        .and_then(|g| g.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e.get("sha256"))
        .and_then(|s| s.as_str())
        .expect("expected.json golden[0].sha256");
    assert_eq!(recorded, digest);
}

#[test]
fn example_package_inspects_to_exactly_the_five_mandatory_resources() {
    let root = repo_root();
    let bytes = read(&root.join("examples/hello-en/hello-en.elang"));
    let pkg = Package::open(&bytes).expect("open committed example");
    let inspected = pkg.inspect();
    let listing = inspected.listing().listing();

    let mut paths: Vec<&str> = listing.iter().map(|e| e.path.as_str()).collect();
    paths.sort_unstable();

    assert_eq!(
        paths,
        vec![
            "Header/format.version",
            "Header/mimetype",
            "Linguistics/phonemes.eld",
            "META-INF/integrity.kdl",
            "package.epf",
        ],
        "example package resource listing drifted from the mandatory set"
    );
}

#[test]
fn example_package_verifies_and_validates_cleanly() {
    let root = repo_root();
    let bytes = read(&root.join("examples/hello-en/hello-en.elang"));
    let pkg = Package::open(&bytes).expect("open committed example");
    let verified = pkg.verify().expect("example package must verify");
    assert_eq!(verified.resource_count(), 5);

    let validated = Package::open(&bytes)
        .expect("re-open for validate")
        .validate()
        .expect("example package must validate");
    assert_eq!(validated.root_count(), 14);
}

#[test]
fn gen_example_package_cli_regenerates_byte_identical_output() {
    // End-to-end: running the public generator binary produces the same
    // artifact as a fresh in-process build. Catches drift in the CLI / I/O
    // path that the in-process build would not exercise.
    //
    // The generator is pointed at a scratch `--out-dir`, so it never
    // touches the published artefacts. Writing them in place would let
    // this test *repair* a drifted artefact before another test read
    // it, and two concurrently-running tests could read the archive
    // while it was being rewritten.
    let root = repo_root();
    let committed_archive = read(&root.join("examples/hello-en/hello-en.elang"));
    let committed_sidecar = read(&root.join("examples/hello-en/hello-en.elang.sha256"));
    let committed_expected = read(&root.join("examples/hello-en/expected.json"));

    let scratch = root
        .join("target")
        .join(format!("example-package-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("create scratch directory");

    let status = Command::new(env!("CARGO_BIN_EXE_gen_example_package"))
        .args(["--out-dir", scratch.to_str().expect("utf-8 scratch path")])
        .status()
        .expect("spawn gen_example_package");
    assert!(status.success(), "gen_example_package exited non-zero");

    let produced = scratch.join("examples/hello-en");
    assert_eq!(
        read(&produced.join("hello-en.elang")),
        committed_archive,
        "the generator's archive drifted from the committed one — re-run \
         `cargo run -p elang-conformance --bin gen_example_package`"
    );
    assert_eq!(
        read(&produced.join("hello-en.elang.sha256")),
        committed_sidecar,
        "the generator's digest sidecar drifted from the committed one"
    );
    assert_eq!(
        read(&produced.join("expected.json")),
        committed_expected,
        "the generator's expected.json drifted from the committed one"
    );

    // The published artefacts must be exactly as they were before the
    // generator ran.
    assert_eq!(
        read(&root.join("examples/hello-en/hello-en.elang")),
        committed_archive,
        "the generator wrote into the repository instead of the scratch directory"
    );

    let _ = fs::remove_dir_all(&scratch);
}
