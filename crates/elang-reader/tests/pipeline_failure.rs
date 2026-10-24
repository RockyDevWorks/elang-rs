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

fn assert_diagnostic(
    error: elang_core::Error,
    code: &str,
    category: Category,
    stage: u8,
    path: &str,
) {
    let diag = error.diagnostic;
    assert_eq!(diag.code, code, "{diag:?}");
    assert_eq!(diag.category, category, "{diag:?}");
    assert_eq!(diag.stage, Some(stage), "{diag:?}");
    assert_eq!(diag.resource_path.as_deref(), Some(path), "{diag:?}");
}

fn resource_specs() -> Vec<common::Spec> {
    vec![
        common::Spec::new("Header/mimetype", common::MIMETYPE_PAYLOAD),
        common::Spec::new("Header/format.version", common::FORMAT_VERSION_PAYLOAD),
        common::Spec::new("package.epf", common::EPF_PAYLOAD),
        common::Spec::new("Linguistics/phonemes.eld", common::ELD_PAYLOAD),
    ]
}

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
    assert_eq!(diag.stage, Some(1));
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

#[test]
fn bootstrap_decode_budget_failure_is_preserved() {
    let archive = build_with_swapped("Header/format.version", vec![b'0'; 65]);
    let err = Package::open(&archive.bytes).unwrap().verify().unwrap_err();
    assert_diagnostic(
        err,
        "DECOMPRESSION_BUDGET_EXCEEDED",
        Category::Frame,
        8,
        "Header/format.version",
    );
}

#[test]
fn integrity_coverage_failures_report_stage_9_and_manifest_path() {
    for (code, extra) in [
        ("INTEGRITY_MISSING_ENTRY", false),
        ("INTEGRITY_EXTRA_ENTRY", true),
    ] {
        let mut resources = build_canonical().resources;
        if extra {
            resources.push(("extra.eld".to_string(), Vec::new()));
        } else {
            resources.retain(|(path, _)| path != "Linguistics/phonemes.eld");
        }
        let manifest = common::manifest_for(&resources);
        let archive = build_with_manifest(&resource_specs(), &manifest);
        let err = Package::open(&archive.bytes).unwrap().verify().unwrap_err();
        assert_diagnostic(err, code, Category::Integrity, 9, "META-INF/integrity.kdl");
    }
}

#[test]
fn epf_failures_report_stage_10_and_resource_path() {
    for (payload, code, path) in [
        (
            b"epf \"0.1.0\"\n".to_vec(),
            "EPF_MISSING_NODE",
            "package.epf",
        ),
        (
            String::from_utf8(common::EPF_PAYLOAD.to_vec())
                .unwrap()
                .replace("Linguistics/phonemes.eld", "Linguistics/missing.eld")
                .into_bytes(),
            "EPF_MANIFEST_PATH_NOT_INDEXED",
            "Linguistics/missing.eld",
        ),
    ] {
        let archive = build_with_swapped("package.epf", payload);
        let err = Package::open(&archive.bytes).unwrap().verify().unwrap_err();
        assert_diagnostic(err, code, Category::EpfSchema, 10, path);
    }
}

#[test]
fn eld_failures_report_stage_12_and_phoneme_path() {
    for (payload, code, category) in [
        (
            b"unexpected\n".to_vec(),
            "ELD_TOPLEVEL_INVALID",
            Category::EldSchema,
        ),
        (
            String::from_utf8(common::ELD_PAYLOAD.to_vec())
                .unwrap()
                .replace("phoneme \"p\"", "phoneme \"☃\"")
                .into_bytes(),
            "IPA_NOT_IN_REPERTOIRE",
            Category::Ipa,
        ),
    ] {
        let archive = build_with_swapped("Linguistics/phonemes.eld", payload);
        let err = Package::open(&archive.bytes)
            .unwrap()
            .validate()
            .unwrap_err();
        assert_diagnostic(err, code, category, 12, "Linguistics/phonemes.eld");
    }
}

#[test]
fn missing_stage_resources_use_resource_path_category_and_callers_stage() {
    for (stage, path) in [
        (8, "Header/format.version"),
        (9, "META-INF/integrity.kdl"),
        (10, "package.epf"),
        (12, "Linguistics/phonemes.eld"),
    ] {
        let mut specs = resource_specs();
        specs.retain(|spec| spec.path != path);
        let archive = common::build(&specs);
        let pkg = Package::open(&archive.bytes).unwrap();
        let view =
            elang_reader::ArchiveView::new(pkg.listing().index(), pkg.listing().layout()).unwrap();
        let epf = elang_epf::parse(common::EPF_PAYLOAD).unwrap();
        // These public stage functions can also be called independently.
        let err = match stage {
            8 => elang_reader::bootstrap::run(&view, &archive.bytes, "0.1").unwrap_err(),
            9 => elang_reader::stages::run_integrity_structure(&view, &archive.bytes).unwrap_err(),
            10 => elang_reader::stages::run_epf(&view, &archive.bytes).unwrap_err(),
            12 => elang_reader::stages::run_eld_semantics(&view, &archive.bytes, &epf).unwrap_err(),
            _ => unreachable!(),
        };
        assert_diagnostic(err, "RESOURCE_MISSING", Category::ResourcePath, stage, path);
    }
}

#[test]
fn frame_failures_preserve_their_category_code_and_callers_stage() {
    let archive = build_canonical();
    let pkg = Package::open(&archive.bytes).unwrap();
    let view =
        elang_reader::ArchiveView::new(pkg.listing().index(), pkg.listing().layout()).unwrap();
    let manifest = elang_reader::stages::run_integrity_structure(&view, &archive.bytes).unwrap();
    let epf = elang_epf::parse(common::EPF_PAYLOAD).unwrap();

    for (stage, path) in [
        (8, "Header/format.version"),
        (9, "META-INF/integrity.kdl"),
        (10, "package.epf"),
        (11, "Header/format.version"),
        (12, "Linguistics/phonemes.eld"),
    ] {
        // A listing can be reused with bytes that were truncated or corrupted
        // after parsing. Test both the read_span and decode failure paths.
        let mut corrupt = archive.bytes.clone();
        let span = view.lookup_str(path).unwrap();
        let checksum_end =
            usize::try_from(span.offset.checked_add(span.compressed_size).unwrap()).unwrap();
        *corrupt
            .get_mut(checksum_end.checked_sub(1).unwrap())
            .unwrap() ^= 1;
        for (bytes, code) in [
            (&[][..], "FRAME_TRUNCATED"),
            (corrupt.as_slice(), "FRAME_CHECKSUM_MISMATCH"),
        ] {
            let err = match stage {
                8 => elang_reader::bootstrap::run(&view, bytes, "0.1").unwrap_err(),
                9 => elang_reader::stages::run_integrity_structure(&view, bytes).unwrap_err(),
                10 => elang_reader::stages::run_epf(&view, bytes).unwrap_err(),
                11 => elang_reader::stages::run_resource_integrity(&view, bytes, &manifest)
                    .unwrap_err(),
                12 => elang_reader::stages::run_eld_semantics(&view, bytes, &epf).unwrap_err(),
                _ => unreachable!(),
            };
            assert_diagnostic(err, code, Category::Frame, stage, path);
        }
    }
}
