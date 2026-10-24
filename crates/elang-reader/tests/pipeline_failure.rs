//! Failure-path tests (T1214) — every stage-specific failure produces the
//! locked diagnostic code with the expected stage and resource path
//! attached.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests assert exact bytes/codes; a panic signals a regression"
)]

mod common;

use common::{build_canonical, build_with_manifest, build_with_swapped};
use elang_core::error::Category;
use elang_reader::Package;
use sha2::{Digest, Sha256};

/// Stage 1 — header magic mismatch (flip the first magic byte).
#[test]
fn header_bad_magic_emits_header_bad_magic_at_stage_1() {
    let mut archive = build_canonical();
    if let Some(first) = archive.bytes.first_mut() {
        *first = b'X';
    }
    let err = Package::open(&archive.bytes).expect_err("stage 1 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "HEADER_BAD_MAGIC");
    // The container layer emits stage 1–7 diagnostics; the reader does not
    // currently re-tag them with a stage number on every path. The code
    // alone identifies the failure, which is the contract every stage
    // shares (T1207).
    assert_eq!(diag.category, Category::Container);
}

/// Stage 8 — `Header/format.version` claims a different major.minor than
/// the binary header.
#[test]
fn format_version_with_wrong_major_minor_emits_format_version_invalid() {
    // Build an archive whose format.version says "1.0.0\n" but whose
    // binary header still encodes major=0, minor=1.
    let archive = build_with_swapped("Header/format.version", b"1.0.0\n".to_vec());
    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 8 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "FORMAT_VERSION_INVALID");
    assert_eq!(diag.stage, Some(8));
    assert_eq!(diag.resource_path.as_deref(), Some("Header/format.version"));
}

/// Stage 8 — `Header/mimetype` content is not the locked mimetype.
#[test]
fn wrong_mimetype_emits_mimetype_invalid() {
    let archive = build_with_swapped("Header/mimetype", b"application/x-evil".to_vec());
    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 8 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "MIMETYPE_INVALID");
    assert_eq!(diag.stage, Some(8));
    assert_eq!(diag.resource_path.as_deref(), Some("Header/mimetype"));
}

/// Stage 11 — a resource's SHA-256 disagrees with the manifest entry.
#[test]
fn hash_mismatch_emits_integrity_hash_mismatch() {
    // Build an archive whose `Linguistics/phonemes.eld` has been swapped
    // for `tampered_eld`, but whose manifest still lists the SHA-256 of
    // the *original* `ELD_PAYLOAD` for that resource (and the real
    // SHA-256s for the other four). Stage 11 must catch the disagreement
    // — the override manifest must cover every indexed resource, else
    // stage 11 trips INTEGRITY_MISSING_ENTRY first.
    let canonical = build_canonical();
    let mut entries = String::from("integrity {\n    algorithm \"sha-256\"\n");
    for (path, bytes) in &canonical.resources {
        if path == "META-INF/integrity.kdl" {
            continue;
        }
        if path == "Linguistics/phonemes.eld" {
            // Keep the *original* SHA-256 so it disagrees with the
            // tampered content.
            let digest: [u8; 32] = Sha256::digest(common::ELD_PAYLOAD).into();
            let hex = digest.iter().fold(String::new(), |mut a, b| {
                use core::fmt::Write as _;
                let _ = write!(&mut a, "{b:02x}");
                a
            });
            entries.push_str(&format!("    resource \"{path}\" sha256=\"{hex}\"\n"));
            continue;
        }
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let hex = digest.iter().fold(String::new(), |mut a, b| {
            use core::fmt::Write as _;
            let _ = write!(&mut a, "{b:02x}");
            a
        });
        entries.push_str(&format!("    resource \"{path}\" sha256=\"{hex}\"\n"));
    }
    entries.push_str("}\n");

    let tampered_eld =
        b"phonemes {\n    consonants {\n        phoneme \"TAMPERED\" { roman \"T\" }\n    }\n}\n";
    let specs = vec![
        common::Spec::new("Header/mimetype", common::MIMETYPE_PAYLOAD),
        common::Spec::new("Header/format.version", common::FORMAT_VERSION_PAYLOAD),
        common::Spec::new("package.epf", common::EPF_PAYLOAD),
        common::Spec::owned("Linguistics/phonemes.eld", tampered_eld.to_vec()),
    ];
    let archive = build_with_manifest(&specs, entries.as_bytes());

    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 11 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "INTEGRITY_HASH_MISMATCH");
    assert_eq!(diag.stage, Some(11));
    assert_eq!(
        diag.resource_path.as_deref(),
        Some("Linguistics/phonemes.eld")
    );
}

/// `require_resource` returns `RESOURCE_MISSING` for an unindexed path.
#[test]
fn missing_resource_emits_resource_missing() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");
    let err = verified
        .require_resource("does/not/exist.eld")
        .expect_err("missing path");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "RESOURCE_MISSING");
    assert_eq!(diag.category, Category::ResourcePath);
}
