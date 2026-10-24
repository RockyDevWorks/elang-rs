//! Scaffold smoke tests for the `elang` binary (Phase 13).
//!
//! These tests exercise the CLI's argv-dispatch surface end-to-end by
//! running the compiled binary and asserting on its exit code and stdout.
//! The full happy-path `pack` (EPF + ELD → valid `.elang` archive) is
//! covered here too, so a CLI regression that breaks the writer dispatch
//! fails fast before the integration tests in `elang-writer` get a chance.

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang no args prints help and exits zero`.
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
/// `elang help prints help and exits zero`.
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
/// `elang unknown subcommand exits two`.
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
/// `elang pack missing required flags exits two`.
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
/// `elang subcommand help exits zero before validating arguments`.
fn elang_subcommand_help_exits_zero_before_validating_arguments() {
    let bin = env!("CARGO_BIN_EXE_elang");
    for subcommand in [
        "pack",
        "inspect",
        "verify",
        "validate",
        "unpack",
        "canonicalize-eld",
        "repertoire",
    ] {
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
                } else if subcommand == "canonicalize-eld" {
                    assert!(stdout.contains("<ELD-file>"));
                } else if subcommand == "unpack" {
                    assert!(stdout.contains("--output"));
                    assert!(stdout.contains("--force"));
                } else {
                    // inspect / verify / validate: usage line is enough.
                }
            }
        }
    }
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang canonicalize eld requires exactly one operand`.
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
/// `elang canonicalize eld accepts one operand`.
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
/// `elang pack with unknown profile exits two`.
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

/// `--format-version` followed by no value (last positional) must surface
/// the pico-args `OptionWithoutAValue` error to stderr and exit 2. The
/// previous `.ok().flatten().unwrap_or_else(...)` swallowed it and
/// silently fell back to the default `"0.1.0"`.
#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang pack with bare format version flag exits two`.
fn elang_pack_with_bare_format_version_flag_exits_two() {
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
        .output()
        .expect("run elang pack --format-version (no value)");
    assert_eq!(
        result.status.code(),
        Some(2),
        "bare --format-version should exit 2, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("--format-version"),
        "stderr must name --format-version: {stderr}"
    );
}

/// `--profile` followed by no value (last positional) must exit 2 with
/// the pico-args error. The previous `.ok().flatten()` silently
/// defaulted to `Profile::Fast` when the value was missing.
#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang pack with bare profile flag exits two`.
fn elang_pack_with_bare_profile_flag_exits_two() {
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
        .output()
        .expect("run elang pack --profile (no value)");
    assert_eq!(
        result.status.code(),
        Some(2),
        "bare --profile should exit 2, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("--profile"),
        "stderr must name --profile: {stderr}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang pack with leftover positional exits two`.
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
/// `elang pack with missing epf file exits one`.
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
/// `elang pack with malformed epf exits one`.
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
/// `elang pack happy path exits zero and produces archive`.
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
/// `elang canonicalize eld missing arg exits two`.
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
/// `elang canonicalize eld with missing file exits one`.
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
/// `elang canonicalize eld happy path exits zero and prints canonical`.
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
/// `elang pack with compact profile exits zero`.
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
/// `elang pack with canonical profile exits zero`.
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
/// `elang pack with explicit format version exits zero`.
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
/// `elang pack with missing eld file exits one`.
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
/// `elang pack with malformed eld exits one`.
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
/// `elang pack with writer error surfaces code and exits one`.
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
/// `elang canonicalize eld with malformed input exits one`.
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
// Reader-side subcommands: inspect / verify / validate / unpack
// ---------------------------------------------------------------------------

/// Pack the fixture EPF + ELD into a valid `.elang` archive and return its
/// path.
#[allow(clippy::expect_used, reason = "test scaffolding")]
/// `build archive`.
fn build_archive(tmp: &TempDir) -> std::path::PathBuf {
    let bin = env!("CARGO_BIN_EXE_elang");
    let epf = tmp.child("build-epf.bin");
    let eld = tmp.child("build-eld.bin");
    let out = tmp.child("build.elang");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let status = std::process::Command::new(bin)
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack to build the fixture archive");
    assert_eq!(
        status.status.code(),
        Some(0),
        "fixture pack should succeed, stderr={}",
        String::from_utf8_lossy(&status.stderr)
    );
    out
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang inspect prints resource listing`.
fn elang_inspect_prints_resource_listing() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let result = std::process::Command::new(bin)
        .args(["inspect"])
        .arg(&archive)
        .output()
        .expect("run elang inspect");
    assert_eq!(
        result.status.code(),
        Some(0),
        "inspect should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("format version: 0.1"),
        "stdout should report the header version: {stdout}"
    );
    assert!(
        stdout.contains("resource count:"),
        "stdout should report the resource count: {stdout}"
    );
    assert!(
        stdout.contains("Header/mimetype"),
        "stdout should list Header/mimetype: {stdout}"
    );
    assert!(
        stdout.contains("Linguistics/phonemes.eld"),
        "stdout should list the ELD resource: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang inspect requires exactly one operand`.
fn elang_inspect_requires_exactly_one_operand() {
    let bin = env!("CARGO_BIN_EXE_elang");
    for operands in [vec![], vec!["a.elang", "b.elang"]] {
        let result = std::process::Command::new(bin)
            .args(["inspect"])
            .args(operands)
            .output()
            .expect("run elang inspect with wrong operand count");
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("usage: elang inspect"));
    }
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang inspect on non archive exits one`.
fn elang_inspect_on_non_archive_exits_one() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let junk = tmp.child("junk.elang");
    std::fs::write(&junk, b"not an archive at all").expect("write junk");
    let result = std::process::Command::new(bin)
        .args(["inspect"])
        .arg(&junk)
        .output()
        .expect("run elang inspect on junk");
    assert_eq!(
        result.status.code(),
        Some(1),
        "inspect on junk should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang inspect on missing file exits one`.
fn elang_inspect_on_missing_file_exits_one() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let result = std::process::Command::new(bin)
        .args(["inspect"])
        .arg(tmp.child("absent.elang"))
        .output()
        .expect("run elang inspect on missing file");
    assert_eq!(
        result.status.code(),
        Some(1),
        "inspect on missing file should exit 1, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang verify passes for valid archive`.
fn elang_verify_passes_for_valid_archive() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let result = std::process::Command::new(bin)
        .args(["verify"])
        .arg(&archive)
        .output()
        .expect("run elang verify");
    assert_eq!(
        result.status.code(),
        Some(0),
        "verify should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("integrity: OK"),
        "stdout should report integrity OK: {stdout}"
    );
    assert!(
        stdout.contains("manifest resources:"),
        "stdout should report the manifest size: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang verify fails for tampered archive`.
fn elang_verify_fails_for_tampered_archive() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    // Corrupt a byte in the middle of the archive: the index/footer digest
    // check or the per-resource SHA-256 must reject it.
    let mut bytes = std::fs::read(&archive).expect("read archive");
    let mid = bytes.len() / 2;
    let byte = bytes.get_mut(mid).expect("archive is not empty");
    *byte = byte.wrapping_add(1);
    let tampered = tmp.child("tampered.elang");
    std::fs::write(&tampered, &bytes).expect("write tampered archive");
    let result = std::process::Command::new(bin)
        .args(["verify"])
        .arg(&tampered)
        .output()
        .expect("run elang verify on tampered archive");
    assert_ne!(
        result.status.code(),
        Some(0),
        "verify must not pass a tampered archive, stdout={}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang verify requires exactly one operand`.
fn elang_verify_requires_exactly_one_operand() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let result = std::process::Command::new(bin)
        .args(["verify"])
        .output()
        .expect("run elang verify with no operand");
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("usage: elang verify"));
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang validate passes for valid archive`.
fn elang_validate_passes_for_valid_archive() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let result = std::process::Command::new(bin)
        .args(["validate"])
        .arg(&archive)
        .output()
        .expect("run elang validate");
    assert_eq!(
        result.status.code(),
        Some(0),
        "validate should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("validation: OK"),
        "stdout should report validation OK: {stdout}"
    );
    assert!(
        stdout.contains("language: Writer Test"),
        "stdout should report the EPF language name: {stdout}"
    );
    assert!(
        stdout.contains("identifier: urn:elang:language:rocky:test:1.0.0"),
        "stdout should report the language identifier: {stdout}"
    );
    assert!(
        stdout.contains("root phonemes:"),
        "stdout should report the root phoneme count: {stdout}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang validate fails for tampered archive`.
fn elang_validate_fails_for_tampered_archive() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let mut bytes = std::fs::read(&archive).expect("read archive");
    let mid = bytes.len() / 2;
    let byte = bytes.get_mut(mid).expect("archive is not empty");
    *byte = byte.wrapping_add(1);
    let tampered = tmp.child("tampered.elang");
    std::fs::write(&tampered, &bytes).expect("write tampered archive");
    let result = std::process::Command::new(bin)
        .args(["validate"])
        .arg(&tampered)
        .output()
        .expect("run elang validate on tampered archive");
    assert_ne!(
        result.status.code(),
        Some(0),
        "validate must not pass a tampered archive"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang validate requires exactly one operand`.
fn elang_validate_requires_exactly_one_operand() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let result = std::process::Command::new(bin)
        .args(["validate"])
        .output()
        .expect("run elang validate with no operand");
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("usage: elang validate"));
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack extracts every resource`.
fn elang_unpack_extracts_every_resource() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = tmp.child("extracted");
    let result = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .output()
        .expect("run elang unpack");
    assert_eq!(
        result.status.code(),
        Some(0),
        "unpack should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("extracted resources:"),
        "stdout should report the extracted count: {stdout}"
    );
    assert!(
        out.join("Header/mimetype").is_file(),
        "unpack should write Header/mimetype"
    );
    assert!(
        out.join("Linguistics/phonemes.eld").is_file(),
        "unpack should write the nested ELD resource"
    );
    assert!(
        out.join("package.epf").is_file(),
        "unpack should write package.epf"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack refuses to overwrite without force`.
fn elang_unpack_refuses_to_overwrite_without_force() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = tmp.child("extracted");
    let first = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .output()
        .expect("run elang unpack (first)");
    assert_eq!(first.status.code(), Some(0));
    // Second run over the same directory must refuse rather than clobber.
    let second = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .output()
        .expect("run elang unpack (second)");
    assert_ne!(
        second.status.code(),
        Some(0),
        "unpack must refuse to overwrite without --force"
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("refusing to overwrite"),
        "stderr must explain the refusal: {stderr}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack with force overwrites existing files`.
fn elang_unpack_with_force_overwrites_existing_files() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = tmp.child("extracted");
    let first = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .output()
        .expect("run elang unpack (first)");
    assert_eq!(first.status.code(), Some(0));
    let second = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .arg("--force")
        .output()
        .expect("run elang unpack --force");
    assert_eq!(
        second.status.code(),
        Some(0),
        "unpack --force should exit 0, stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack without output exits two`.
fn elang_unpack_without_output_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let result = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .output()
        .expect("run elang unpack without --output");
    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("--output"),
        "stderr must name --output: {stderr}"
    );
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack without archive exits two`.
fn elang_unpack_without_archive_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let result = std::process::Command::new(bin)
        .args(["unpack"])
        .args(["--output"])
        .arg(tmp.child("out"))
        .output()
        .expect("run elang unpack without an archive");
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("usage: elang unpack"));
}

/// `elang unpack` refuses to write through a symlink that points outside the
/// output directory.
#[test]
#[cfg(unix)]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack refuses symlink escape`.
fn elang_unpack_refuses_symlink_escape() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = tmp.child("extracted");
    let outside = tmp.child("outside");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    std::fs::create_dir_all(&out).expect("create out dir");
    // `Header` is a real resource directory in the archive; pre-create
    // it as a symlink pointing outside the output tree.
    std::os::unix::fs::symlink(&outside, out.join("Header")).expect("create symlink");
    let result = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .output()
        .expect("run elang unpack with an escaping symlink");
    assert_ne!(
        result.status.code(),
        Some(0),
        "unpack must refuse to follow a symlink out of the output tree"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsafe extraction"),
        "stderr must report the unsafe extraction: {stderr}"
    );
    assert!(
        !outside.join("mimetype").exists(),
        "nothing may be written through the escaping symlink"
    );
}

/// `elang unpack` refuses to write through a symlinked target file even with
/// `--force`.
#[test]
#[cfg(unix)]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang unpack refuses symlinked target file`.
fn elang_unpack_refuses_symlinked_target_file() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = tmp.child("extracted");
    let outside = tmp.child("outside.bin");
    std::fs::write(&outside, b"precious").expect("write outside file");
    std::fs::create_dir_all(&out).expect("create out dir");
    // `package.epf` sits at the archive root, so it has no parent dir to
    // create — the symlink lands exactly on the target file.
    std::os::unix::fs::symlink(&outside, out.join("package.epf")).expect("create symlink");
    let result = std::process::Command::new(bin)
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(&out)
        .arg("--force")
        .output()
        .expect("run elang unpack over a symlinked target");
    assert_ne!(
        result.status.code(),
        Some(0),
        "unpack must refuse to write through a symlinked target"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("symlink"),
        "stderr must report the symlink refusal: {stderr}"
    );
    assert_eq!(
        std::fs::read(&outside).expect("read outside file"),
        b"precious",
        "the symlink target must be untouched"
    );
}

// ---------------------------------------------------------------------------
// `repertoire info`
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang repertoire info prints metadata`.
fn elang_repertoire_info_prints_metadata() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let result = std::process::Command::new(bin)
        .args(["repertoire", "info"])
        .output()
        .expect("run elang repertoire info");
    assert_eq!(
        result.status.code(),
        Some(0),
        "repertoire info should exit 0, stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    for expected in [
        "repertoire version:",
        "unicode version:",
        "ipa chart:",
        "base symbols:",
        "combining diacritics:",
        "spacing modifiers:",
    ] {
        assert!(
            stdout.contains(expected),
            "stdout should contain {expected:?}: {stdout}"
        );
    }
}

#[test]
#[allow(clippy::expect_used, reason = "test invokes the compiled binary")]
/// `elang repertoire without info exits two`.
fn elang_repertoire_without_info_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    for operands in [vec![], vec!["bogus"]] {
        let result = std::process::Command::new(bin)
            .args(["repertoire"])
            .args(operands)
            .output()
            .expect("run elang repertoire without info");
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("usage: elang repertoire info"));
    }
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
    /// `child`.
    fn child(&self, name: &str) -> std::path::PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    /// `drop`.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[allow(clippy::expect_used, reason = "test scaffolding")]
/// `tempdir`.
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
