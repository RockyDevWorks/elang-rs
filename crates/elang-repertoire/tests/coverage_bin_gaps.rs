//! Coverage-closure tests for the repertoire's developer binaries.
//!
//! `gen_repertoire` is the human-reviewable source of
//! `specs/schema/ipa-v1.json`; re-running it MUST reproduce the committed
//! file byte-for-byte. `print_digest` prints the RFC 8785 JCS SHA-256 digest
//! of that pinned JSON. Neither binary is reachable from `cargo test`
//! otherwise, because `cargo test` does not execute a package's `[[bin]]`
//! targets — an integration test has to run them explicitly.
//!
//! Both binaries are run with their working directory pointed at a scratch
//! directory under `target/`: `gen_repertoire` writes `specs/schema/ipa-v1.json`
//! *relative to the current directory*, so a scratch CWD keeps the repository
//! working tree untouched.
//!
//! Test code path: `.expect()` / panics signal fixture regressions, not
//! untrusted-input failure. The workspace lint policy denies these on
//! production code; tests live under this allow.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use std::fs;
use std::path::Path;
use std::process::Command;

use elang_repertoire::repertoire::{canonical_jcs_sha256, REPERTOIRE_JSON};

/// The generator reproduces the committed repertoire JSON byte-for-byte.
#[test]
fn gen_repertoire_reproduces_the_pinned_json() {
    let workdir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gen_repertoire");
    let schema_dir = workdir.join("specs").join("schema");
    fs::create_dir_all(&schema_dir).expect("scratch schema directory is created");

    // Remove the previous run's output first. Otherwise a generator that
    // exits successfully *without* writing the file would still be
    // satisfied by the stale file from an earlier run, and the assertion
    // below would pass without proving that this run wrote anything.
    let output_path = schema_dir.join("ipa-v1.json");
    match fs::remove_file(&output_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("stale generator output cannot be removed: {error}"),
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gen_repertoire"))
        .current_dir(&workdir)
        .output()
        .expect("the generator runs");
    assert!(output.status.success(), "generator failed: {output:?}",);

    let generated = fs::read_to_string(&output_path).expect("the generator wrote its output");
    assert_eq!(
        generated, REPERTOIRE_JSON,
        "the generator must reproduce the pinned repertoire JSON byte-for-byte"
    );
}

/// The digest tool prints the canonical JCS digest of the pinned JSON.
#[test]
fn print_digest_prints_the_pinned_jcs_digest() {
    let output = Command::new(env!("CARGO_BIN_EXE_print_digest"))
        .output()
        .expect("the digest tool runs");
    assert!(output.status.success(), "digest tool failed: {output:?}",);

    let printed = String::from_utf8(output.stdout).expect("the digest is utf-8");
    let expected =
        canonical_jcs_sha256(REPERTOIRE_JSON).expect("the pinned JSON has a canonical digest");
    assert_eq!(printed.trim(), expected);
}
