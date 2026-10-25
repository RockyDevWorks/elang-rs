//! The source-release artifact is byte-reproducible (T1617 preparation).
//!
//! `scripts/build-release-artifacts.sh` assembles the tarball that a
//! `v0.1.0` release would publish. This test runs it twice into two
//! scratch directories and asserts that:
//!
//! * the two artifacts are byte-identical,
//! * the artifact contains the files the release claims to ship,
//! * the script leaves the working tree untouched.
//!
//! The script never creates a tag, publishes anything, or reaches the
//! network; it only writes under `target/`. The test asserts the first of
//! those properties indirectly, by diffing `git status` before and after.
//!
//! Linux-only: the script drives GNU `tar` and `gzip`. GNU tar and BSD tar
//! (macOS) do not write identical archives for the same inputs, so the
//! script requires GNU tar and refuses to run elsewhere rather than
//! silently producing a platform-dependent artifact. Linux is the release
//! platform — the `reproducible-build` CI workflow and the release itself
//! run on ubuntu.

#![cfg(target_os = "linux")]
// A test file is not a parser of untrusted input: a failed assertion is
// exactly the signal it is meant to raise. Same posture as the sibling
// `measurement_shape.rs`.
#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use elang_conformance::binary_vectors::{hex, sha256};

/// Repository root, derived from this crate's manifest directory
/// (`crates/elang-conformance` → two levels up).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root is resolvable")
}

fn run_script(root: &Path, out_dir: &Path) {
    let script = root.join("scripts").join("build-release-artifacts.sh");
    assert!(
        script.is_file(),
        "missing {}: the release-artifact script must exist",
        script.display()
    );

    let output = Command::new("bash")
        .arg(&script)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--version")
        .arg("0.1.0")
        .current_dir(root)
        .output()
        .expect("build-release-artifacts.sh is runnable");

    assert!(
        output.status.success(),
        "build-release-artifacts.sh failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn artifact_path(out_dir: &Path) -> PathBuf {
    out_dir.join("elang-v0.1.0.tar.gz")
}

fn sha256_of(path: &Path) -> String {
    let bytes = fs::read(path).expect("artifact is readable");
    hex(&sha256(&bytes))
}

/// Every file the release claims to ship must be in the artifact. If a
/// future change moves the license, the lockfile digest or the release
/// notes out of the tree, this fails rather than shipping an artifact
/// that is missing part of itself.
#[test]
fn the_artifact_contains_every_file_the_release_claims_to_ship() {
    let root = repo_root();
    let out_dir = root
        .join("target")
        .join("release-packaging-test")
        .join("contains");
    run_script(&root, &out_dir);

    let listing = Command::new("tar")
        .arg("-tzf")
        .arg(artifact_path(&out_dir))
        .current_dir(&root)
        .output()
        .expect("tar can list the artifact");
    assert!(
        listing.status.success(),
        "tar -tzf failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );

    let names: Vec<String> = String::from_utf8_lossy(&listing.stdout)
        .lines()
        .map(str::trim_end)
        .map(str::to_string)
        .collect();

    for required in [
        "LICENSE",
        "README.md",
        "Cargo.toml",
        "Cargo.lock",
        "Cargo.lock.sha256",
        "CHANGELOG.md",
        "SECURITY.md",
        "docs/release/v0.1.0/README.md",
        "docs/release/v0.1.0/ARCHIVE.md",
        "docs/release/v0.1.0/review-record.md",
        "docs/release/v0.1.0/security-checklist.md",
        "specs/registry/normative-manifest.json",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "release artifact is missing {required}"
        );
    }

    // The build output directory must not have leaked into the artifact.
    //
    // Positive control first: `target/` has to actually be git-ignored,
    // otherwise the exclusion assertion below would pass vacuously — for
    // example when the build output lives elsewhere (`CARGO_TARGET_DIR`) and
    // was never a candidate for the file list in the first place.
    let ignored = Command::new("git")
        .args(["check-ignore", "--no-index", "-q"])
        .arg("target/release-packaging-test/contains/elang-v0.1.0.tar.gz")
        .current_dir(&root)
        .status()
        .expect("git check-ignore runs");
    assert!(
        ignored.success(),
        "target/ must be git-ignored for the exclusion check to be meaningful"
    );
    assert!(
        !names.iter().any(|n| n.starts_with("target/")),
        "release artifact contains build output"
    );
}

/// Two builds of the same tree must produce the same bytes. This is the
/// property the reproducible-build CI workflow checks for the canonical
/// artifacts; here it is checked for the release archive itself.
#[test]
fn two_builds_of_the_same_tree_are_byte_identical() {
    let root = repo_root();
    let scratch = root.join("target").join("release-packaging-test");
    let a = scratch.join("a");
    let b = scratch.join("b");

    run_script(&root, &a);
    run_script(&root, &b);

    let bytes_a = fs::read(artifact_path(&a)).expect("artifact A is readable");
    let bytes_b = fs::read(artifact_path(&b)).expect("artifact B is readable");

    assert_eq!(
        sha256_of(&a.join("elang-v0.1.0.tar.gz")),
        sha256_of(&b.join("elang-v0.1.0.tar.gz")),
        "two builds of the same tree produced different artifacts"
    );
    assert_eq!(
        bytes_a, bytes_b,
        "release artifact is not byte-reproducible"
    );

    // The sidecar must agree with the artifact it describes.
    let sidecar =
        fs::read_to_string(a.join("elang-v0.1.0.tar.gz.sha256")).expect("sidecar is present");
    let recorded = sidecar
        .split_whitespace()
        .next()
        .expect("sidecar carries a digest");
    assert_eq!(recorded, sha256_of(&artifact_path(&a)));
}

/// The script must not touch the working tree: building an artifact is a
/// read of the tree, not a write to it.
#[test]
fn building_the_artifact_does_not_modify_the_working_tree() {
    let root = repo_root();

    let status_of = || {
        let out = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&root)
            .output()
            .expect("git status runs");
        assert!(out.status.success(), "git status failed");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    let before = status_of();
    run_script(
        &root,
        &root
            .join("target")
            .join("release-packaging-test")
            .join("clean"),
    );
    let after = status_of();

    assert_eq!(
        before, after,
        "building the release artifact modified the working tree"
    );
}
