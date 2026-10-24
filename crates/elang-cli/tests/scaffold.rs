//! Scaffold smoke test (Phase 13 stub coverage).

#[test]
#[allow(clippy::expect_used, reason = "test asserts the CLI scaffold runs")]
fn elang_cli_scaffold_exits_two() {
    let bin = env!("CARGO_BIN_EXE_elang");
    let out = std::process::Command::new(bin).output().expect("run elang");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("elang"),
        "stderr should contain version info"
    );
}
