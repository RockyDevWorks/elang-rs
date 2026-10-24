//! Happy-path integration tests (T1204) — every operation runs the full
//! state machine and reaches its terminal state without errors. These
//! tests exercise the **real** binary container, **real** Zstandard
//! compression, **real** KDL parsing, **real** ELD schema validation and
//! **real** IPA repertoire lookups in one pass.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert exact values; unwrap on known-good input"
)]

mod common;

use common::{build_canonical, EPF_PAYLOAD};
use elang_reader::Package;

#[test]
fn inspect_runs_stages_1_through_7_and_lists_all_resources() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let inspected = pkg.inspect();

    // The archive carries five resources; inspect must list them all.
    assert_eq!(inspected.len(), 5);
    let paths = inspected.paths();
    assert!(paths.contains(&"Header/mimetype"));
    assert!(paths.contains(&"Header/format.version"));
    assert!(paths.contains(&"META-INF/integrity.kdl"));
    assert!(paths.contains(&"package.epf"));
    assert!(paths.contains(&"Linguistics/phonemes.eld"));
}

#[test]
fn verify_runs_stages_1_through_11_and_loads_manifest() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");

    // Stage 11 verified every resource against the manifest, so all five
    // resources should be available by path.
    assert!(verified.require_resource("Header/mimetype").is_ok());
    assert!(verified.require_resource("Header/format.version").is_ok());
    assert!(verified.require_resource("META-INF/integrity.kdl").is_ok());
    assert!(verified.require_resource("package.epf").is_ok());
    assert!(verified
        .require_resource("Linguistics/phonemes.eld")
        .is_ok());

    // The manifest must expose at least one entry (T0706).
    assert!(!verified.manifest().resources.is_empty());
    assert_eq!(
        verified.manifest().algorithm,
        elang_integrity::Algorithm::Sha256
    );
}

#[test]
fn validate_runs_all_twelve_stages_and_exposes_root_phonemes() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let validated = pkg.validate().expect("stages 8-12");

    // T1209: validated inventory exposes language metadata.
    let metadata = validated.metadata();
    assert_eq!(
        metadata.identifier().as_str(),
        "urn:elang:language:rocky:test:1.0.0"
    );
    assert_eq!(metadata.name(), "Reader Test");

    // The canonical fixture carries two root phonemes ('p' and 'a').
    assert_eq!(validated.root_count(), 2);
    let romans: Vec<&str> = validated.roots().map(|p| p.roman().as_str()).collect();
    assert!(romans.contains(&"p"));
    assert!(romans.contains(&"a"));
}

#[test]
fn canonical_eld_round_trips_through_canonical_serialization() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let validated = pkg.validate().expect("stages 8-12");

    // T1210: canonical ELD bytes are deterministic and re-parseable.
    let serialized = validated.canonical_phoneme_bytes();
    assert!(!serialized.is_empty());
    let _doc = elang_kdl::parse(&serialized).expect("canonical bytes parse as KDL");
}

#[test]
fn epf_payload_round_trips_through_verified() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");

    // The verify path exposes the EPF document via the validated layer,
    // but the verified cache itself stores the raw bytes.
    let bytes = verified
        .require_resource("package.epf")
        .expect("epf present");
    assert_eq!(bytes, EPF_PAYLOAD);
}

#[test]
fn bootstrap_resources_agree_with_binary_header() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");

    let mimetype = verified
        .require_resource("Header/mimetype")
        .expect("mimetype present");
    let format_version = verified
        .require_resource("Header/format.version")
        .expect("format.version present");
    assert_eq!(mimetype, b"application/vnd.elang");
    assert_eq!(format_version, b"0.1.0\n");
}
