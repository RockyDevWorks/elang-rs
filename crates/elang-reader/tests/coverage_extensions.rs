//! Coverage-extension integration tests (T1215).
//!
//! Exercises every public getter on `Inspected` / `Verified` / `Validated`,
//! the remaining error-paths in `stages.rs` / `bootstrap.rs` that the
//! happy-path tests don't reach (UTF-8 errors, non-digit patch, missing
//! manifest entries, malformed KDL, unindexed EPF phoneme path), and the
//! `Package::archive` / `Package::listing` accessors.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests assert exact bytes/codes; a panic signals a regression"
)]

mod common;

use common::{build_canonical, build_with_manifest, build_with_swapped};
use elang_core::error::Category;
use elang_integrity::manifest::Algorithm;
use elang_reader::Package;
use sha2::{Digest, Sha256};

// ---- Inspected surface ----------------------------------------------------

#[test]
fn inspected_exposes_listing_and_len() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let inspected = pkg.inspect();
    assert_eq!(inspected.len(), 5);
    assert!(!inspected.is_empty());
    let listing = inspected.listing();
    assert_eq!(listing.header().major(), 0);
    assert_eq!(listing.header().minor(), 1);
    let (major, minor) = inspected.header_version();
    assert_eq!((major, minor), (0, 1));
    assert!(inspected.total_uncompressed() > 0);
    assert_eq!(inspected.paths().len(), 5);
}

// ---- Verified surface ----------------------------------------------------

#[test]
fn verified_exposes_manifest_epf_and_resources() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");

    assert_eq!(verified.resource_count(), 5);
    let manifest = verified.manifest();
    assert_eq!(manifest.algorithm, Algorithm::Sha256);
    assert_eq!(manifest.resources.len(), 4); // integrity.kdl self-excluded
    let _epf = verified.epf();

    // get_resource: every indexed path is decodable
    let mimetype = verified
        .get_resource("Header/mimetype")
        .expect("mimetype decoded");
    assert_eq!(mimetype, b"application/vnd.elang");

    // resources() iterator preserves index order
    let collected: Vec<&str> = verified.resources().map(|(p, _)| p).collect();
    assert_eq!(collected.len(), 5);
    assert!(collected.contains(&"Header/mimetype"));

    // manifest_entry: indexed resource has a matching manifest entry
    let entry = verified
        .manifest_entry("Header/mimetype")
        .expect("entry present");
    assert_eq!(entry.path.as_str(), "Header/mimetype");
    assert_eq!(entry.digest.len(), 32);

    // as_inspected clones the listing and preserves len
    let downgraded = verified.as_inspected();
    assert_eq!(downgraded.len(), 5);
}

// ---- Validated surface ----------------------------------------------------

#[test]
fn validated_exposes_inventory_metadata_and_getters() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let validated = pkg.validate().expect("stages 8-12");

    // metadata
    let metadata = validated.metadata();
    assert_eq!(
        metadata.identifier().as_str(),
        "urn:elang:language:rocky:test:1.0.0"
    );
    assert_eq!(metadata.name(), "Reader Test");

    // inventory groups
    let groups = validated.groups();
    assert_eq!(groups.len(), 2); // consonants + vowels

    // group_by_name
    let consonants = validated
        .group_by_name("consonants")
        .expect("consonants group");
    assert_eq!(consonants.roots().len(), 1);
    assert!(validated.group_by_name("nope").is_none());

    // root_count
    assert_eq!(validated.root_count(), 2);

    // find_root + roots()
    let p_root = validated.find_root("p").expect("root p");
    assert_eq!(p_root.ipa().as_str(), "p");
    assert!(validated.find_root("zz").is_none());

    let all_ipas: Vec<&str> = validated.roots().map(|r| r.ipa().as_str()).collect();
    assert_eq!(all_ipas.len(), 2);

    // condition_for: canonical ELD has no `when` blocks
    assert!(validated.condition_for("p").is_none());

    // allophone_when: canonical ELD has no allophones
    assert!(validated.allophone_when("p", "ph").is_none());

    // canonical_phoneme_bytes: deterministic, non-empty
    let canon = validated.canonical_phoneme_bytes();
    assert!(!canon.is_empty());
    // Calling twice must yield the same bytes (Phase 11 byte-determinism).
    let again = validated.canonical_phoneme_bytes();
    assert_eq!(canon, again);

    // Validated.manifest / .inventory / .epf return the cached values
    assert_eq!(validated.manifest().resources.len(), 4);
    assert_eq!(validated.inventory().groups().len(), 2);
    assert_eq!(
        validated.epf().manifest().phonemes().as_str(),
        "Linguistics/phonemes.eld"
    );
}

#[test]
fn validated_downgrades_to_inspected_while_verified_keeps_resources() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    let verified = pkg.verify().expect("stages 8-11");
    let validated = Package::open(&archive.bytes)
        .expect("stages 1-7")
        .validate()
        .expect("stages 8-12");

    // as_inspected: cheap clone
    let downgraded_inspected = validated.as_inspected();
    assert_eq!(downgraded_inspected.len(), 5);

    // Keep the original Verified when decoded resource access is needed.
    assert_eq!(verified.resource_count(), 5);
    assert_eq!(
        verified.get_resource("Header/mimetype"),
        Some(common::MIMETYPE_PAYLOAD)
    );
    assert_eq!(verified.manifest().resources.len(), 4);
}

// ---- Package accessors ----------------------------------------------------

#[test]
fn package_exposes_archive_bytes_and_listing() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).expect("stages 1-7");
    // The Package owns the bytes (cheap clone of the slice is fine for tests).
    assert_eq!(pkg.archive(), archive.bytes.as_slice());
    let listing = pkg.listing();
    assert_eq!(listing.header().major(), 0);
    assert_eq!(listing.index().entries().len(), 5);
}

// ---- Stage 8 — bootstrap extra paths --------------------------------------

#[test]
fn format_version_non_utf8_emits_format_version_invalid() {
    // Three bytes that are not valid UTF-8 anywhere.
    let archive = build_with_swapped("Header/format.version", vec![0xFF, 0xFE, 0xFD]);
    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 8 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "FORMAT_VERSION_INVALID");
    assert_eq!(diag.stage, Some(8));
    assert_eq!(diag.resource_path.as_deref(), Some("Header/format.version"));
}

#[test]
fn format_version_non_digit_patch_emits_format_version_invalid() {
    // Major.minor prefix "0.1." matches the binary header, but the patch
    // component is "x" — not a non-empty ASCII integer.
    let archive = build_with_swapped("Header/format.version", b"0.1.x\n".to_vec());
    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 8 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "FORMAT_VERSION_INVALID");
    assert_eq!(diag.stage, Some(8));
    assert_eq!(diag.resource_path.as_deref(), Some("Header/format.version"));
}

// ---- Stage 9 — integrity structure extra paths ---------------------------

#[test]
fn integrity_structure_invalid_emits_integrity_structure_invalid() {
    // Inject a manifest that's not valid KDL — the KDL parser fails at stage 9.
    let specs = vec![
        common::Spec::new("Header/mimetype", common::MIMETYPE_PAYLOAD),
        common::Spec::new("Header/format.version", common::FORMAT_VERSION_PAYLOAD),
        common::Spec::new("package.epf", common::EPF_PAYLOAD),
        common::Spec::new("Linguistics/phonemes.eld", common::ELD_PAYLOAD),
    ];
    // Deliberately malformed: missing closing brace, illegal node syntax.
    let bad_manifest = b"integrity { algorithm \"sha-256\" garbage-no-quotes\n".to_vec();
    let archive = build_with_manifest(&specs, &bad_manifest);

    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 9 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "INTEGRITY_STRUCTURE_INVALID");
    assert_eq!(diag.stage, Some(9));
    assert_eq!(diag.category, Category::Integrity);
}

#[test]
fn integrity_missing_entry_emits_integrity_missing_entry() {
    // Manifest covers every indexed resource except one. Stage 9 must catch
    // the missing entry via `verify_coverage`.
    let canonical = build_canonical();
    let mut entries = String::from("integrity {\n    algorithm \"sha-256\"\n");
    let mut omitted_one = false;
    for (path, bytes) in &canonical.resources {
        if path == "META-INF/integrity.kdl" {
            continue;
        }
        // Drop exactly one entry — `Linguistics/phonemes.eld` is convenient
        // because it's the last one we iterate.
        if path == "Linguistics/phonemes.eld" && !omitted_one {
            omitted_one = true;
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

    let specs = vec![
        common::Spec::new("Header/mimetype", common::MIMETYPE_PAYLOAD),
        common::Spec::new("Header/format.version", common::FORMAT_VERSION_PAYLOAD),
        common::Spec::new("package.epf", common::EPF_PAYLOAD),
        common::Spec::new("Linguistics/phonemes.eld", common::ELD_PAYLOAD),
    ];
    let archive = build_with_manifest(&specs, entries.as_bytes());

    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 9 fails");
    let diag = &err.diagnostic;
    assert_eq!(diag.code, "INTEGRITY_MISSING_ENTRY");
    assert_eq!(diag.stage, Some(9));
    assert_eq!(diag.category, Category::Integrity);
    // The reader attaches the manifest path; the message identifies its
    // missing entry.
    assert_eq!(
        diag.resource_path.as_deref(),
        Some("META-INF/integrity.kdl")
    );
    assert!(
        diag.message.contains("Linguistics/phonemes.eld"),
        "message should name the missing entry, got: {}",
        diag.message,
    );
}

// ---- Stage 10 — EPF phoneme path not in index -----------------------------

#[test]
fn epf_phoneme_path_unindexed_emits_epf_phonemes_not_indexed() {
    // Replace package.epf with one that declares a phoneme path the
    // archive's index does not contain. The builder recomputes the manifest
    // from the swapped bytes, so stage 9 passes; stage 10 then fails
    // because the declared phoneme path is not in the index.
    let epf_with_missing_phonemes = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Reader Test\"\n}\nmanifest {\n    phonemes \"Linguistics/does_not_exist.eld\"\n}\n";
    let archive = build_with_swapped("package.epf", epf_with_missing_phonemes.to_vec());

    let err = Package::open(&archive.bytes)
        .and_then(|pkg| pkg.verify())
        .expect_err("stage 10 fails");
    let diag = &err.diagnostic;
    // `resolve_phonemes` emits `EPF_MANIFEST_PATH_NOT_INDEXED` when the
    // declared phoneme path is missing from the index. The adapter
    // attaches the declared path to the diagnostic's `resource_path`.
    assert_eq!(diag.code, "EPF_MANIFEST_PATH_NOT_INDEXED");
    assert_eq!(diag.stage, Some(10));
    assert_eq!(
        diag.resource_path.as_deref(),
        Some("Linguistics/does_not_exist.eld")
    );
}
