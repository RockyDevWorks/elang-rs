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
