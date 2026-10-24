//! Scaffold smoke tests for the `elang` binary (Phase 13).
//!
//! These tests exercise the CLI's argv-dispatch surface end-to-end by
//! running the compiled binary and asserting on its exit code and stdout.
//! The full happy-path `pack` (EPF + ELD → valid `.elang` archive) is
//! covered here too, so a CLI regression that breaks the writer dispatch
//! fails fast before the integration tests in `elang-writer` get a chance.

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_no_args_prints_help_and_exits_zero() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let out = std::process::Command::new(bin).output().expect("run elang");
    assert_eq!(out.status.code(), Some(0), "no-args should exit 0 (help)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Subcommands"),
        "stdout should print help banner: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_help_prints_help_and_exits_zero() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let out = std::process::Command::new(bin)
        .arg("help")
        .output()
        .expect("run elang help");
    assert_eq!(out.status.code(), Some(0), "help should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Subcommands"),
        "stdout should print help banner: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_unknown_subcommand_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let out = std::process::Command::new(bin)
        .arg("bogus-subcommand")
        .output()
        .expect("run elang bogus-subcommand");
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown subcommand should exit 2"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_missing_required_flags_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .output()
        .expect("run elang pack (no flags)");
    assert_eq!(
        out.status.code(),
        Some(2),
        "pack with missing --epf/--eld/--output should exit 2"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_subcommand_help_exits_zero_before_validating_arguments() {
    let bin = env!("CARGO_BIN_EXE_elang");
    for subcommand in ["pack", "canonicalize-eld"] {
        for flag in ["-h", "--help"] {
            for operands in [
                vec![],
                vec!["--profile", "invalid", "missing.eld", "extra.eld"],
            ] {
                let out = std::process::Command::new(bin)
                    .arg(subcommand)
                    .args(operands)
                    .arg(flag)
                    .output()
                    .expect("run subcommand help");
                assert_eq!(out.status.code(), Some(0), "{subcommand} {flag}");
                let stdout = String::from_utf8_lossy(&out.stdout);
                assert!(stdout.contains(&format!("usage: elang {subcommand}")));
                assert!(out.stderr.is_empty(), "help should not print errors");
                if subcommand == "pack" {
                    for option in [
                        "--epf",
                        "--eld",
                        "--output",
                        "--profile",
                        "--format-version",
                    ] {
                        assert!(stdout.contains(option), "missing option {option}: {stdout}");
                    }
                } else {
                    assert!(stdout.contains("<ELD-file>"));
                }
            }
        }
    }
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_requires_exactly_one_operand() {
    let bin = env!("CARGO_BIN_EXE_elang");
    for operands in [vec![], vec!["missing.eld", "extra.eld"]] {
        let out = std::process::Command::new(bin)
            .arg("canonicalize-eld")
            .args(operands)
            .output()
            .expect("run canonicalize-eld with invalid operand count");
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr)
            .contains("usage: elang canonicalize-eld <ELD-file>"));
    }
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_accepts_one_operand() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/valid/canonical/minimal-inventory/input.eld");
    let out = std::process::Command::new(bin)
        .arg("canonicalize-eld")
        .arg(input)
        .output()
        .expect("run canonicalize-eld with one input");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert_eq!(
        out.stdout,
        include_bytes!("../../../fixtures/valid/canonical/minimal-inventory/golden/canonical.eld")
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_unknown_profile_exits_two() {
    // The `--profile` parser has a fourth branch (`Some(other) => exit 2`)
    // that nothing else exercises. Drive it with an unknown profile name.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("--profile")
        .arg("bogus")
        .output()
        .expect("run elang pack --profile bogus");
    assert_eq!(
        result.status.code(),
        Some(2),
        "unknown --profile should exit 2, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_leftover_positional_exits_two() {
    // The leftover-positional check (`args.clone().finish()`) is its own
    // branch — drive it with a stray positional after the known flags.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("stray-positional")
        .output()
        .expect("run elang pack with stray positional");
    assert_eq!(
        result.status.code(),
        Some(2),
        "leftover positional should exit 2, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_missing_epf_file_exits_one() {
    // `fs::read(&epf_path)` fails when the file is absent; this is the
    // `Err(e) => exit 1` branch on the EPF-read arm.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(tmp.child("missing.bin"))
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack with missing EPF");
    assert_eq!(
        result.status.code(),
        Some(1),
        "missing EPF file should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_malformed_epf_exits_one() {
    // `elang_epf::parse(&epf_bytes)` fails when the bytes don't parse; this
    // is the `Err(e) => exit 1` branch on the EPF-parse arm.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, b"this is not a valid EPF\n").expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack with malformed EPF");
    assert_eq!(
        result.status.code(),
        Some(1),
        "malformed EPF should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_happy_path_exits_zero_and_produces_archive() {
    // Full happy path: minimal EPF + ELD → writer succeeds → file exists.
    // Also exercises the default --profile (Fast) branch.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack happy path");
    assert_eq!(
        result.status.code(),
        Some(0),
        "happy pack should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(&out).expect("read produced archive");
    // Magic + version prefix — proves the writer ran and the file is
    // recognisably an ELANG archive, not an empty stub.
    assert!(
        bytes.len() > 32,
        "archive should be longer than the binary header, got {} bytes",
        bytes.len()
    );
    // `HEADER_MAGIC = [0x45, 0x4C, 0x41, 0x4E, 0x47, 0x0D, 0x0A, 0x1A]`
    // i.e. the 8-byte ASCII sequence `ELANG\r\n\x1a` (steering §3.1).
    assert_eq!(
        bytes.get(..8).expect("archive is at least 8 bytes"),
        b"ELANG\r\n\x1a" as &[u8],
        "binary header magic should be present"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_missing_arg_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let result = std::process::Command::new(bin)
        .arg("canonicalize-eld")
        .output()
        .expect("run elang canonicalize-eld (no arg)");
    assert_eq!(
        result.status.code(),
        Some(2),
        "canonicalize-eld with no input should exit 2"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_with_missing_file_exits_one() {
    // `fs::read(&input)` failure on the canonicalize-eld arm.
    let bin = env!("CARGO_BIN_EXE_elang");
    // Build a path that is guaranteed not to exist on every platform by
    // nesting a fresh tempdir name under the system tempdir. We never
    // create the leaf file, so the binary's `fs::read` returns `Err`.
    let missing = std::env::temp_dir().join(format!(
        "elang-cli-scaffold-missing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let result = std::process::Command::new(bin)
        .args(["canonicalize-eld"])
        .arg(&missing)
        .output()
        .expect("run elang canonicalize-eld missing");
    assert_eq!(
        result.status.code(),
        Some(1),
        "missing input should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_happy_path_exits_zero_and_prints_canonical() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let eld = tmp.child("eld.bin");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["canonicalize-eld"])
        .arg(&eld)
        .output()
        .expect("run elang canonicalize-eld happy path");
    assert_eq!(
        result.status.code(),
        Some(0),
        "canonicalize-eld happy path should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    // The canonical KDL for the minimal inventory starts with `phonemes {`.
    assert!(
        stdout.contains("phonemes"),
        "stdout should contain the canonical phoneme inventory: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_compact_profile_exits_zero() {
    // Drives the `--profile compact` arm of the profile parser; the happy
    // path is otherwise identical to the default-profile case but the
    // profile branch in `run_pack` is its own line of code.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("--profile")
        .arg("compact")
        .output()
        .expect("run elang pack --profile compact");
    assert_eq!(
        result.status.code(),
        Some(0),
        "--profile compact should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::fs::metadata(&out)
            .map(|m| m.len() > 32)
            .unwrap_or(false),
        "compact-profile archive should exist and be longer than the header"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_canonical_profile_exits_zero() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("--profile")
        .arg("canonical")
        .output()
        .expect("run elang pack --profile canonical");
    assert_eq!(
        result.status.code(),
        Some(0),
        "--profile canonical should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_explicit_format_version_exits_zero() {
    // Drives the `args.opt_value_from_str("--format-version")` arm. The
    // explicit version flows into the writer's `validate_format_version`
    // and the success path is otherwise identical to the default.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("--format-version")
        .arg("0.1.0")
        .output()
        .expect("run elang pack --format-version 0.1.0");
    assert_eq!(
        result.status.code(),
        Some(0),
        "--format-version should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_missing_eld_file_exits_one() {
    // `fs::read(&eld_path)` failure on the ELD-read arm — mirrors the
    // existing EPF-missing test but covers the eld branch.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(tmp.child("missing.eld"))
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack with missing ELD");
    assert_eq!(
        result.status.code(),
        Some(1),
        "missing ELD file should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_malformed_eld_exits_one() {
    // `elang_eld::parse(&eld_bytes, &rep)` failure on the ELD-parse arm.
    // The bytes below are valid UTF-8 and not a syntactically valid ELD
    // document; the parser returns a registered `ELD_*` code which the
    // CLI prints verbatim to stderr and turns into exit 1.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, b"this is not a valid ELD\n").expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack with malformed ELD");
    assert_eq!(
        result.status.code(),
        Some(1),
        "malformed ELD should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("parse"),
        "stderr should mention the parse failure: {stderr}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_pack_with_writer_error_surfaces_code_and_exits_one() {
    // The writer's `BuildError::code()` round-trips through the CLI:
    // the stderr line is `write archive: [<code>] <message>`. Pass a
    // bad `--format-version` so the writer fails with
    // `WRITER_INVALID_FORMAT_VERSION` before any byte is emitted.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    let out = tmp.child("archive.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .arg("--format-version")
        .arg("not-a-version")
        .output()
        .expect("run elang pack with bad --format-version");
    assert_eq!(
        result.status.code(),
        Some(1),
        "writer error should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("WRITER_INVALID_FORMAT_VERSION"),
        "stderr should embed the writer's diagnostic code: {stderr}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
fn elang_canonicalize_eld_with_malformed_input_exits_one() {
    // `elang_eld::parse` failure on the canonicalize-eld arm.
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let eld = tmp.child("eld.bin");
    std::fs::write(&eld, b"not a phoneme inventory\n").expect("write eld");
    let result = std::process::Command::new(bin)
        .args(["canonicalize-eld"])
        .arg(&eld)
        .output()
        .expect("run elang canonicalize-eld malformed");
    assert_eq!(
        result.status.code(),
        Some(1),
        "malformed canonicalize-eld input should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("parse"),
        "stderr should mention the parse failure: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Shared fixtures + helpers
// ---------------------------------------------------------------------------

const EPF_FIXTURE: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

const ELD_FIXTURE: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

/// A minimal unique-directory abstraction. Avoids pulling in a tempdir
/// dev-dependency just for these tests; the API matches the subset we use.
struct TempDir {
    path: std::path::PathBuf,
}

impl TempDir {
    fn child(&self, name: &str) -> std::path::PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[allow(clippy::expect_used, reason = "test scaffolding")]
fn tempdir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let base = std::env::temp_dir();
    let unique = format!(
        "elang-cli-scaffold-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = base.join(unique);
    std::fs::create_dir_all(&path).expect("create tempdir");
    TempDir { path }
}
