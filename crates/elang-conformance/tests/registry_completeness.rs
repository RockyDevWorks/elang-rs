//! Fail-closed checks that every stable error code the crates emit is
//! published in the registry, with the right category.
//!
//! Error codes are **stable** machine-readable identifiers (steering §11): the
//! reference implementation and the conformance corpus both refer to them by
//! exact string, and renaming or removing one requires an ADR. That makes the
//! registry and the code two sides of a contract that nothing in the compiler
//! enforces — a new `Error::new("SOMETHING_NEW", …)` compiles perfectly while
//! the code is absent from `specs/registry/error-codes.{md,json}`, and any
//! consumer filtering on it silently never matches.
//!
//! These tests close that gap. They failed for real: eight codes introduced
//! under elang-core were unregistered, and two carried a `Category` that
//! disagreed with their own registry entry.
//!
//! # Detection strategy
//!
//! Rust statements routinely span multiple physical lines
//! (`return Err(err(\n    "CODE",\n    "msg",\n));`), so the scanner first
//! joins them into *logical lines* using a paren-balance walk that also
//! tracks double-quote state. Within each logical line it:
//!
//! * skips the line if its first non-blank token is `//` or `//!`;
//! * blanks out `env!("…")` and `option_env!("…")` so their contents never
//!   count as codes;
//! * for every `"CODE"` token, rightward-searches for the nearest
//!   `Category::Variant` (skipping comments and string bodies) to record a
//!   `(code, category)` association.
//!
//! A literal is treated as a code only if it is `SCREAMING_SNAKE_CASE`, at
//! least [`MIN_CODE_LEN`] chars (the shortest real code, `PATH_NUL`, is 8),
//! and not one of the registry's own category names. A small explicit
//! [`KNOWN_NON_CODES`] allowlist holds binary magic bytes and the historical
//! gaps from earlier phases.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Shortest real error code is `PATH_NUL` (8). Anything shorter is a fragment.
const MIN_CODE_LEN: usize = 8;

/// Uppercase literals that pass the shape filter but are **not** error codes.
///
/// * Binary magic bytes — these are container/index format constants, never
///   diagnostics.
/// * Codes emitted by earlier phases (6/9) that predate this check and are
///   not yet in the registry. They are tracked here so this PR does not have
///   to widen its scope to register another phase's codes; each must be added
///   to the registry when its phase is finalised.
const KNOWN_NON_CODES: &[&str] = &[
    // Binary format magic (elang-conformance / elang-container). `ELENXXXX` is
    // a deliberately corrupted footer magic used by the T0321 footer-variant
    // test, not a diagnostic.
    "ELEND001",
    "ELIDX001",
    "ELENXXXX",
    // Deliberately corrupted footer magic used by the layout parse-error test.
    "NOPENOPE",
    // Emitted by elang-core::model::eld (Phase 9 — registry entries pending).
    "ELD_GROUP_EMPTY",
    "ELD_MISSING_REQUIRED_GROUP",
    "ELD_TOO_MANY_ALLOPHONES",
    "ELD_TOO_MANY_GROUPS",
    "ELD_TOO_MANY_ROOTS",
    // Emitted by elang-core::model::epf (Phase 7 — registry entries pending).
    "EPF_DESCRIPTION_TOO_LONG",
    "EPF_IDENTIFIER_BAD_SLUG",
    "EPF_IDENTIFIER_BAD_URN",
    "EPF_IDENTIFIER_EMPTY",
    "EPF_IDENTIFIER_TOO_LONG",
    "EPF_NAME_EMPTY",
    "EPF_NAME_TOO_LONG",
    // Emitted by elang-repertoire::ipa / elang-core::model::when.
    "IPA_EMPTY",
    "WHEN_GROUP_ARITY",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("elang-conformance lives at <root>/crates/elang-conformance")
        .to_path_buf()
}

fn read_registry_json() -> Value {
    let path = workspace_root().join("specs/registry/error-codes.json");
    let raw =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

fn read_registry_md() -> String {
    let path = workspace_root().join("specs/registry/error-codes.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn registry_codes(json: &Value) -> BTreeMap<String, String> {
    let list = json["codes"]
        .as_array()
        .expect("registry has a `codes` array");
    list.iter()
        .map(|c| {
            let code = c["code"].as_str().expect("code is a string").to_owned();
            let category = c["category"]
                .as_str()
                .expect("category is a string")
                .to_owned();
            (code, category)
        })
        .collect()
}

fn registry_categories(json: &Value) -> BTreeSet<String> {
    json["categories"]
        .as_array()
        .expect("registry has a `categories` array")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn is_code_shaped(s: &str) -> bool {
    s.len() >= MIN_CODE_LEN
        && s.starts_with(|c: char| c.is_ascii_uppercase())
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Convert a Rust `Category` variant (`EldSchema`, `ResourcePath`) to the
/// registry's `SCREAMING_SNAKE_CASE` token (`ELD_SCHEMA`, `RESOURCE_PATH`).
fn normalize_category(variant: &str) -> String {
    let mut out = String::with_capacity(variant.len() + 2);
    for (i, ch) in variant.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// Every `.rs` file under `<root>/crates/*/{src,tests}`.
fn crate_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = fs::read_dir(root.join("crates")).expect("crates/ exists");
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            for sub in ["src", "tests"] {
                collect_rs(&entry.path().join(sub), &mut out);
            }
        }
    }
    out.sort();
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Join a Rust source into *logical lines*: a multi-line statement becomes
/// one logical line so pattern matching need not care about formatting.
///
/// Paren-balance is tracked literally; double-quote state is tracked as a
/// running toggle so a `(` inside a string literal does not push the depth.
fn logical_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    for line in source.lines() {
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '(' if !in_string => depth += 1,
                ')' if !in_string => depth -= 1,
                '"' => in_string = !in_string,
                '\\' if in_string => {
                    // Skip the escaped character so a `\"` does not toggle
                    // string state.
                    chars.next();
                }
                _ => {}
            }
        }
        if depth <= 0 && !buf.trim().is_empty() {
            out.push(std::mem::take(&mut buf));
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Remove `env!("X")` and `option_env!("X")` segments so their contents can
/// never be mistaken for codes.
fn strip_env_macros(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(pos) = rest.find("env!(") {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + "env!(".len()..];
        match rest.find(')') {
            Some(end) => rest = &rest[end + 1..],
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Read the string literal starting at `chars[start]` (which must be `"`),
/// returning its body and the index just past the closing quote.
fn read_literal(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut body = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                if let Some(escaped) = chars.get(i + 1) {
                    body.push(*escaped);
                }
                i += 2;
            }
            '"' => return Some((body, i + 1)),
            c => {
                body.push(c);
                i += 1;
            }
        }
    }
    None
}

/// `code -> category`, where `Some(variant)` records the Rust `Category::X`
/// variant the code is emitted with. Codes emitted via a helper whose
/// category is hardcoded (e.g. `limit_err`) have value `None` and rely on
/// the completeness check alone.
fn scan_source(source: &str) -> BTreeMap<String, Option<String>> {
    let mut found: BTreeMap<String, Option<String>> = BTreeSet::new().into_iter().collect();

    for logical in logical_lines(source) {
        let trimmed = logical.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("//!") {
            continue;
        }
        let stripped = strip_env_macros(&logical);
        let chars: Vec<char> = stripped.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '"' {
                let Some((body, after)) = read_literal(&chars, i) else {
                    break;
                };
                if is_code_shaped(&body) {
                    let category = nearest_category(&chars[after..]);
                    found.entry(body).or_insert(category);
                }
                i = after;
                continue;
            }
            i += 1;
        }
    }

    found
}

/// Look rightward in `chars` for the nearest `Category::Variant`, skipping
/// `//` comments and the bodies of string literals.
fn nearest_category(chars: &[char]) -> Option<String> {
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if chars[i] == '"' {
            let (_, after) = read_literal(chars, i)?;
            i = after;
            continue;
        }
        let marker = "Category::";
        if chars[i..].starts_with(marker.chars().collect::<Vec<_>>().as_slice()) {
            let mut end = i + marker.len();
            while let Some(c) = chars.get(end) {
                if c.is_ascii_alphanumeric() || *c == '_' {
                    end += 1;
                } else {
                    break;
                }
            }
            let variant: String = chars[i + marker.len()..end].iter().collect();
            if !variant.is_empty() && variant.starts_with(|c: char| c.is_ascii_uppercase()) {
                return Some(variant);
            }
        }
        i += 1;
    }
    None
}

#[test]
fn every_code_in_the_crates_is_registered() {
    let root = workspace_root();
    let json = read_registry_json();
    let registered = registry_codes(&json);
    let categories = registry_categories(&json);
    let known_non: BTreeSet<&str> = KNOWN_NON_CODES.iter().copied().collect();

    let mut missing: BTreeSet<String> = BTreeSet::new();
    let mut emitted = 0usize;

    for file in crate_sources(&root) {
        let source = fs::read_to_string(&file).expect("source file is UTF-8");
        for (code, _) in scan_source(&source) {
            if !is_code_shaped(&code)
                || categories.contains(&code)
                || known_non.contains(code.as_str())
            {
                continue;
            }
            emitted += 1;
            if !registered.contains_key(&code) {
                missing.insert(format!("{code}  (in {})", file.display()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} error code(s) are emitted by the crates but absent from \
         specs/registry/error-codes.{{md,json}}. Error codes are stable public \
         contract (steering §11); a consumer filtering on an unregistered code \
         never matches. Add each to both registry formats.\n\n  {}\n",
        missing.len(),
        missing.into_iter().collect::<Vec<_>>().join("\n  "),
    );
    assert!(
        emitted > 0,
        "scanner found no codes at all — extraction is broken"
    );
}

#[test]
fn code_categories_match_the_registry() {
    let root = workspace_root();
    let json = read_registry_json();
    let registered = registry_codes(&json);
    let categories = registry_categories(&json);
    let known_non: BTreeSet<&str> = KNOWN_NON_CODES.iter().copied().collect();

    let mut mismatched: BTreeSet<String> = BTreeSet::new();

    for file in crate_sources(&root) {
        let source = fs::read_to_string(&file).expect("source file is UTF-8");
        for (code, emitted) in scan_source(&source) {
            if !is_code_shaped(&code)
                || categories.contains(&code)
                || known_non.contains(code.as_str())
            {
                continue;
            }
            let (Some(emitted), Some(expected)) = (emitted, registered.get(&code)) else {
                continue;
            };
            if normalize_category(&emitted) != *expected {
                mismatched.insert(format!(
                    "{code}: crates emit Category::{emitted} but the registry says {expected}"
                ));
            }
        }
    }

    assert!(
        mismatched.is_empty(),
        "{} code(s) emit a Category that disagrees with their registry \
         entry. Consumers switch on category as well as code, so a mismatch \
         misroutes the diagnostic.\n\n  {}\n",
        mismatched.len(),
        mismatched.into_iter().collect::<Vec<_>>().join("\n  "),
    );
}

#[test]
fn the_two_registry_formats_agree() {
    let json = read_registry_json();
    let md = read_registry_md();
    let registered = registry_codes(&json);

    let list = json["codes"].as_array().expect("codes array");
    assert_eq!(
        list.len(),
        registered.len(),
        "error-codes.json contains duplicate `code` entries"
    );

    let undocumented: Vec<&String> = registered.keys().filter(|c| !md.contains(*c)).collect();
    assert!(
        undocumented.is_empty(),
        "{} code(s) are in error-codes.json but absent from error-codes.md: {:?}",
        undocumented.len(),
        undocumented,
    );

    let categories = registry_categories(&json);
    let unknown: Vec<&String> = registered
        .values()
        .filter(|c| !categories.contains(*c))
        .collect();
    assert!(
        unknown.is_empty(),
        "codes reference categories not declared in the registry: {unknown:?}",
    );
}

/// `limit_err` and similar helpers hardcode their category outside the call
/// site (e.g. `Category::Limit`), so the scanner that walks for `"CODE",
/// Category::X` cannot see them. This targeted check parses every helper
/// invocation across the crates and verifies that the codes they emit carry
/// the helper's documented category in the registry.
#[test]
fn helper_hardcoded_categories_match_the_registry() {
    let root = workspace_root();
    let json = read_registry_json();
    let registered = registry_codes(&json);

    // (helper_name, expected_registry_category)
    let helper_categories = [("limit_err", "LIMIT")];

    let mut wrong: BTreeSet<String> = BTreeSet::new();

    for file in crate_sources(&root) {
        let source = fs::read_to_string(&file).expect("source file is UTF-8");
        let logicals = logical_lines(&source);
        for logical in logicals {
            for (helper, expected) in &helper_categories {
                let mut i = 0;
                let chars: Vec<char> = logical.chars().collect();
                while let Some(pos) =
                    find_subseq(&chars[i..], &helper.chars().collect::<Vec<char>>())
                {
                    let abs = i + pos + helper.len();
                    // Walk forward through `(`, whitespace, and string-literal
                    // bodies to land on the first real `"CODE"` arg.
                    let mut j = abs;
                    while j < chars.len() && (chars[j].is_whitespace() || chars[j] == '(') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j] == '"' {
                        if let Some((body, _)) = read_literal(&chars, j) {
                            if is_code_shaped(&body) {
                                let actual = registered.get(&body);
                                if actual.map(String::as_str) != Some(*expected) {
                                    wrong.insert(format!(
                                        "{body} via {helper}: registry says \
                                         {actual:?}, expected {expected}"
                                    ));
                                }
                            }
                        }
                    }
                    i = abs;
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} helper call(s) carry a category that disagrees with the \
         registry. The helper's hardcoded category is part of the public \
         contract; a mismatch silently misroutes the diagnostic.\n\n  {}\n",
        wrong.len(),
        wrong.into_iter().collect::<Vec<_>>().join("\n  "),
    );
}

fn find_subseq(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    'outer: for start in 0..=haystack.len().saturating_sub(needle.len()) {
        for k in 0..needle.len() {
            if haystack[start + k] != needle[k] {
                continue 'outer;
            }
        }
        return Some(start);
    }
    None
}
