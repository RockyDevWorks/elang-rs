//! Integration tests for the CLI's `--json` diagnostic mode (T1609).
//!
//! The contract these tests pin:
//!
//! * `--json` emits exactly one JSON object on stdout and nothing else
//!   (no extra objects, no trailing bytes, no human-readable text).
//! * The object always carries `ok`, `command`, and `exit_code`.
//! * On failure it carries an `error` object with `code` and `message`.
//! * `code` is the stable OA-10 error-code registry code when the
//!   failure came from the reader, the writer, or another library
//!   boundary; `null` is reserved for CLI usage errors.
//! * Human-mode output is never interleaved into the JSON stream.
//!
//! These tests parse stdout with `serde_json` rather than a hand-rolled
//! brace counter: a brace counter accepts two concatenated objects as
//! "one envelope", which silently masks regressions where a stray
//! `eprintln!` writes after the envelope has been emitted.

// Integration tests legitimately panic on setup failure and on
// arithmetic that cannot overflow in this domain.
#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_elang"))
}

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        p.pop() && p.pop(),
        "crate is not under a two-level parent (CARGO_MANIFEST_DIR={})",
        p.display()
    );
    p
}

fn example_archive() -> PathBuf {
    repo_root().join("examples/hello-en/hello-en.elang")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().expect("run elang")
}

/// Look up a nested field without panicking on a missing key.
///
/// `serde_json::Value`'s `Index` impl panics when the key is absent, which
/// would turn a malformed envelope into a confusing panic instead of a
/// readable assertion failure.
#[track_caller]
fn field<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    let mut cur = v;
    for key in path {
        cur = cur
            .get(*key)
            .unwrap_or_else(|| panic!("envelope is missing field {key:?}: {v}"));
    }
    cur
}

/// Parse `stdout` as exactly one JSON object with no trailing bytes.
///
/// A real JSON parser catches the regressions a brace counter misses —
/// two concatenated objects, a trailing log line, or a stray character
/// after the closing `}`.
fn parse_single_envelope(stdout: &[u8]) -> Value {
    let mut iter = serde_json::Deserializer::from_slice(stdout).into_iter::<Value>();
    let v = iter
        .next()
        .expect("stdout was empty; expected one JSON envelope")
        .expect("stdout was not valid JSON; expected one JSON envelope");
    assert!(
        iter.next().is_none(),
        "stdout contained more than one JSON value; expected exactly one envelope: {}",
        String::from_utf8_lossy(stdout)
    );
    let pos = iter.byte_offset();
    assert_eq!(
        pos,
        stdout.len(),
        "stdout had trailing bytes after the JSON envelope (offset {pos} of {}): {}",
        stdout.len(),
        String::from_utf8_lossy(stdout)
    );
    v
}

#[track_caller]
fn require_envelope(out: &std::process::Output) -> Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "command failed: stderr: {:?}",
        out.stderr
    );
    // The envelope is pretty-printed, so it spans several lines; what
    // matters is that it parses as exactly one JSON object with no
    // trailing bytes, which `parse_single_envelope` enforces.
    parse_single_envelope(&out.stdout)
}

#[track_caller]
fn require_envelope_with_exit(out: &std::process::Output, expected_exit: i32) -> Value {
    assert_eq!(
        out.status.code(),
        Some(expected_exit),
        "wrong exit code; stderr: {:?}",
        out.stderr
    );
    parse_single_envelope(&out.stdout)
}

#[test]
fn json_verify_success_emits_ok_envelope() {
    let out = run(&["--json", "verify", &example_archive().to_string_lossy()]);
    let v = require_envelope_with_exit(&out, 0);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(true));
    assert_eq!(field(&v, &["command"]), &Value::String("verify".into()));
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(0u8.into()));
    assert!(
        v.get("error").is_none(),
        "success envelope must not carry error: {v}"
    );
}

#[test]
fn json_mode_does_not_leak_human_output_into_stdout() {
    let out = run(&["--json", "inspect", &example_archive().to_string_lossy()]);
    // The envelope is the *only* thing on stdout; parse it as such.
    let v = parse_single_envelope(&out.stdout);
    assert_eq!(field(&v, &["command"]), &Value::String("inspect".into()));
    assert_eq!(field(&v, &["ok"]), &Value::Bool(true));
}

#[test]
fn json_failure_carries_stable_error_code() {
    let bad = repo_root().join("target/tmp/json-bad-magic.elang");
    if let Some(parent) = bad.parent() {
        std::fs::create_dir_all(parent).expect("create tmp dir");
    }
    std::fs::write(&bad, b"NOT-AN-ELANG-ARCHIVE-000000000").expect("write bad archive");

    let out = run(&["--json", "verify", &bad.to_string_lossy()]);
    let v = require_envelope_with_exit(&out, 1);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(false));
    assert_eq!(field(&v, &["command"]), &Value::String("verify".into()));
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(1u8.into()));
    // Strict equality, not a substring match: future regressions where
    // the code stops being `HEADER_BAD_MAGIC` must fail this test.
    assert_eq!(
        field(&v, &["error", "code"]),
        &Value::String("HEADER_BAD_MAGIC".into())
    );
    let msg = field(&v, &["error", "message"])
        .as_str()
        .expect("message is a string");
    assert!(!msg.is_empty(), "error.message must not be empty");

    let _ = std::fs::remove_file(&bad);
}

#[test]
fn json_pack_failure_carries_stable_error_code() {
    // `pack` requires --epf, --eld and --output. A bad EPF exercises the
    // reader-failure path that the reviewer flagged as silently emitting
    // `code = null`.
    let bad_epf = repo_root().join("target/tmp/json-pack-bad.epf");
    if let Some(parent) = bad_epf.parent() {
        std::fs::create_dir_all(parent).expect("create tmp dir");
    }
    std::fs::write(&bad_epf, b"not-a-valid-epf").expect("write bad epf");
    let good_eld = repo_root().join("examples/hello-en/Linguistics/phonemes.eld");
    let out_path = repo_root().join("target/tmp/json-pack-out.elang");

    let out = run(&[
        "--json",
        "pack",
        "--epf",
        &bad_epf.to_string_lossy(),
        "--eld",
        &good_eld.to_string_lossy(),
        "--output",
        &out_path.to_string_lossy(),
    ]);
    let v = require_envelope_with_exit(&out, 1);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(false));
    assert_eq!(field(&v, &["command"]), &Value::String("pack".into()));
    let code = field(&v, &["error", "code"])
        .as_str()
        .expect("pack reader failure must carry a non-null OA-10 code");
    assert!(
        !code.is_empty() && code != "null",
        "pack reader failure must carry a non-null code, got {v}"
    );

    let _ = std::fs::remove_file(&bad_epf);
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn json_canonicalize_eld_failure_carries_stable_error_code() {
    let bad = repo_root().join("target/tmp/json-canon-bad.eld");
    if let Some(parent) = bad.parent() {
        std::fs::create_dir_all(parent).expect("create tmp dir");
    }
    std::fs::write(&bad, b"not-a-valid-eld").expect("write bad eld");

    let out = run(&["--json", "canonicalize-eld", &bad.to_string_lossy()]);
    let v = require_envelope_with_exit(&out, 1);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(false));
    assert_eq!(
        field(&v, &["command"]),
        &Value::String("canonicalize-eld".into())
    );
    let code = field(&v, &["error", "code"])
        .as_str()
        .expect("canonicalize-eld reader failure must carry a non-null OA-10 code");
    assert!(
        !code.is_empty() && code != "null",
        "canonicalize-eld reader failure must carry a non-null code, got {v}"
    );

    let _ = std::fs::remove_file(&bad);
}

#[test]
fn json_usage_error_exits_two_with_null_code() {
    let out = run(&["--json", "verify"]);
    let v = require_envelope_with_exit(&out, 2);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(false));
    assert_eq!(field(&v, &["command"]), &Value::String("verify".into()));
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(2u8.into()));
    assert_eq!(
        field(&v, &["error", "code"]),
        &Value::Null,
        "usage error must carry code=null, got {v}"
    );
    let message = field(&v, &["error", "message"])
        .as_str()
        .expect("usage error message");
    assert!(
        message.contains("usage:") || message.to_lowercase().contains("required"),
        "expected usage text, got {message:?}"
    );
}

#[test]
fn json_unknown_subcommand_exits_two() {
    let out = run(&["--json", "definitely-not-a-subcommand"]);
    let v = require_envelope_with_exit(&out, 2);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(false));
    // The envelope echoes the attempted command name rather than
    // substituting "help", so a caller can correlate the failure with
    // what it actually asked for.
    assert_eq!(
        field(&v, &["command"]),
        &Value::String("definitely-not-a-subcommand".into())
    );
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(2u8.into()));
}

#[test]
fn json_help_emits_one_envelope_not_human_text() {
    // The reviewer flagged that `elang --json` and `elang --json help`
    // used to print the help text to stdout instead of a JSON envelope.
    // Verify stdout is exactly one envelope.
    let out = run(&["--json"]);
    let v = require_envelope(&out);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(true));
    assert_eq!(field(&v, &["command"]), &Value::String("help".into()));
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(0u8.into()));
}

#[test]
fn json_help_subcommand_emits_one_envelope() {
    let out = run(&["--json", "help"]);
    let v = require_envelope(&out);
    assert_eq!(field(&v, &["ok"]), &Value::Bool(true));
    assert_eq!(field(&v, &["command"]), &Value::String("help".into()));
    assert_eq!(field(&v, &["exit_code"]), &Value::Number(0u8.into()));
}

#[test]
fn json_per_subcommand_help_emits_one_envelope() {
    // For every subcommand, `elang --json <sub> --help` must emit exactly
    // one JSON envelope (command=<sub>, exit_code=0) and nothing else.
    for sub in [
        "pack",
        "inspect",
        "verify",
        "validate",
        "unpack",
        "canonicalize-eld",
        "repertoire",
    ] {
        let out = run(&["--json", sub, "--help"]);
        let v = require_envelope(&out);
        assert_eq!(
            field(&v, &["command"]),
            &Value::String(sub.into()),
            "--help envelope for {sub} did not carry the right command: {v}"
        );
        assert_eq!(field(&v, &["exit_code"]), &Value::Number(0u8.into()));
        assert_eq!(field(&v, &["ok"]), &Value::Bool(true));
    }
}

#[test]
fn json_failure_envelope_round_trips_through_escaped_message() {
    // Drive a failure with a path containing characters the JSON escaper
    // has to handle, then confirm the message survives a parse/serialise
    // round-trip unchanged.
    let bad = repo_root().join("target/tmp/json-escape.elang");
    if let Some(parent) = bad.parent() {
        std::fs::create_dir_all(parent).expect("create tmp dir");
    }
    std::fs::write(&bad, b"not-an-archive \"with\" a\\back\\slash")
        .expect("write bad archive with escapes");

    let out = run(&["--json", "verify", &bad.to_string_lossy()]);
    let v = require_envelope_with_exit(&out, 1);
    let msg = field(&v, &["error", "message"])
        .as_str()
        .expect("message is a string");
    // Re-serialise the parsed value and confirm the message survives.
    let round = serde_json::to_string(&v).expect("re-serialise envelope");
    let parsed_again: Value = serde_json::from_str(&round).expect("re-parse envelope");
    assert_eq!(
        field(&parsed_again, &["error", "message"]),
        &Value::String(msg.into())
    );

    let _ = std::fs::remove_file(&bad);
}

#[test]
fn human_mode_still_prints_the_listing() {
    let out = run(&["inspect", &example_archive().to_string_lossy()]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("archive:"), "{stdout}");
    assert!(stdout.contains("Linguistics/phonemes.eld"), "{stdout}");
    assert!(
        !stdout.contains("\"ok\""),
        "human mode must not emit a JSON envelope: {stdout}"
    );
}
