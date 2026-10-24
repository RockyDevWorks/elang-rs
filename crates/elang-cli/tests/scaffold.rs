//! Scaffold smoke tests for the `elang` binary (Phase 13).
//!
//! These tests exercise the CLI's argv-dispatch surface end-to-end by
//! running the compiled binary and asserting on its exit code and stdout.

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
