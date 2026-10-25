//! Machine-readable diagnostic output for the `elang` CLI (T1609).
//!
//! The CLI's default output is human-readable text on stdout (results)
//! and stderr (diagnostics). With the global `--json` flag the CLI
//! emits a single JSON object on **stdout** instead, carrying the
//! command, the exit code, and — when the failure came from the
//! reader or the writer — the **stable error code** from the OA-10
//! error-code registry.
//!
//! # Why the stable code matters
//!
//! The human-readable message is not a stable interface: it can be
//! reworded, it embeds absolute paths, and it is localised in a future
//! release. The error code (`INTEGRITY_HASH_MISMATCH`,
//! `HEADER_BAD_MAGIC`, …) *is* stable — it is published in
//! `specs/registry/error-codes.json` and every conformance fixture
//! asserts it. A machine consumer should key off the code and treat
//! the message as display text.
//!
//! # Schema
//!
//! ```json
//! {
//!   "ok": false,
//!   "command": "verify",
//!   "exit_code": 1,
//!   "error": {
//!     "code": "INTEGRITY_HASH_MISMATCH",
//!     "message": "verify /tmp/x.elang: resource digest mismatch"
//!   }
//! }
//! ```
//!
//! On success:
//!
//! ```json
//! {
//!   "ok": true,
//!   "command": "verify",
//!   "exit_code": 0
//! }
//! ```
//!
//! `error` is absent on success. `code` is absent when the failure was
//! a CLI usage error (bad flags, missing operand) rather than a
//! reader/writer diagnostic — those exit with code `2` and are the
//! caller's fault, not the package's.
//!
//! # Note on what this is *not*
//!
//! This is a **diagnostic** format, not an authenticity mechanism. A
//! JSON envelope proves that the CLI produced the output; it says
//! nothing about who produced the package. See
//! `docs/release/v0.1.0/README.md` for the integrity-vs-authenticity
//! distinction.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Whether `--json` was requested.
static JSON_MODE: AtomicBool = AtomicBool::new(false);

/// The buffered diagnostics, captured while JSON mode is on.
static BUFFER: Mutex<Option<String>> = Mutex::new(None);

/// The stable error code for the failure, when one is known.
static ERROR_CODE: Mutex<Option<String>> = Mutex::new(None);

/// Turn JSON mode on or off.
///
/// Must be called before any diagnostic is emitted.
pub fn set_json_mode(on: bool) {
    JSON_MODE.store(on, Ordering::Relaxed);
    if on {
        let mut buf = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        if buf.is_none() {
            *buf = Some(String::new());
        }
    }
}

/// Whether JSON mode is on.
pub fn json_mode() -> bool {
    JSON_MODE.load(Ordering::Relaxed)
}

/// Record a diagnostic line.
///
/// In human mode this goes straight to stderr. In JSON mode it is
/// buffered and folded into the `message` field of the envelope.
pub fn diagnostic(text: &str) {
    if json_mode() {
        let mut buf = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(b) = buf.as_mut() {
            if !b.is_empty() {
                b.push('\n');
            }
            b.push_str(text);
        }
    } else {
        eprintln!("{text}");
    }
}

/// Record a diagnostic with a stable error code.
///
/// The code is what a machine consumer should key off; the text is
/// display-only.
pub fn diagnostic_with_code(code: &str, text: &str) {
    {
        let mut slot = ERROR_CODE.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = Some(code.to_string());
        }
    }
    diagnostic(text);
}

/// Emit the JSON envelope on stdout.
///
/// Call exactly once, after the subcommand has returned.
pub fn emit_envelope(command: &str, exit_code: u8) {
    let message = {
        let mut buf = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        buf.take().unwrap_or_default()
    };
    let code = {
        let mut slot = ERROR_CODE.lock().unwrap_or_else(|e| e.into_inner());
        slot.take()
    };
    let ok = exit_code == 0;
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"ok\": {},\n", ok));
    out.push_str(&format!("  \"command\": {},\n", json_string(command)));
    out.push_str(&format!("  \"exit_code\": {exit_code}"));
    if !ok {
        out.push_str(",\n  \"error\": {\n");
        match code {
            Some(c) => {
                out.push_str(&format!("    \"code\": {},\n", json_string(&c)));
            }
            None => {
                out.push_str("    \"code\": null,\n");
            }
        }
        out.push_str(&format!("    \"message\": {}\n", json_string(&message)));
        out.push_str("  }");
    }
    out.push_str("\n}\n");
    print!("{out}");
}

/// Escape a string as a JSON string literal.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len().saturating_add(2));
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
