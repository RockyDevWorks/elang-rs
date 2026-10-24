//! Meta-tests for the conformance corpus.
//!
//! The conformance corpus's coverage table
//! ([`crate::requirement_coverage_data`]) references test functions
//! across the workspace. The acceptance criterion is "Every normative MUST
//! requirement is mapped to at least one positive or negative conformance
//! test" — and that claim is only as strong as the weakest named test. A
//! coverage row that points at a test that does not exist is a fabrication.
//!
//! This module is the anti-fabrication check. It scans every crate's source
//! tree at test time and asserts the named test exists, then runs the
//! "meta" rows (governance / process requirements that are not exercised by
//! Rust code).
//!
//! # What lives here, what lives in tests/
//!
//! - `meta_tests.rs` (this file) — tests that run inside the lib target so
//!   they participate in the lib test count, plus the source-tree scan.
//! - `tests/requirement_completeness.rs` — the bidirectional spec ↔
//!   coverage check.
//!
//! Splitting the two means a regression that drops a named test fails the
//! test suite immediately rather than silently passing the completeness
//! check.

// The whole module is test-only: it exists to assert things about the
// workspace's test tree, which is meaningless outside a test build.
#![cfg(test)]
#![allow(clippy::expect_used, reason = "tests are the spec")]

use crate::requirement_coverage_data::{rows, CoverageKind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The workspace root, found by walking up from this crate's manifest dir.
fn workspace_root() -> Option<PathBuf> {
    let mut dir: &Path = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".kiro").is_dir() && dir.join("Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Collect every `fn <name>()` from a `.rs` file.
///
/// The scan is deliberately dependency-free: the corpus is a developer
/// tool, and pulling in a regex engine to find `fn` keywords would add a
/// dependency to a crate that exists to check other crates.
///
/// The matcher walks the text looking for the `fn` keyword at a token
/// boundary (preceded by a non-identifier character), reads the
/// identifier that follows, and keeps it only when it is immediately
/// followed by an empty argument list — which is what a `#[test]`
/// function looks like.
///
/// A bare `fn name()` match is not enough: the workspace is full of
/// zero-argument *helpers* (`fn minimal_groups()`, `fn all_test_names()`)
/// that are not tests. A coverage row naming one of those would pass an
/// existence check while proving nothing, so the matcher additionally
/// requires the function to carry a test attribute — `#[test]`,
/// `#[tokio::test]`, or any other attribute whose last path segment is
/// `test`. The attribute run immediately above the `fn` is scanned
/// backwards, skipping whitespace and any other `#[...]` attributes in
/// between (for example `#[test]` followed by `#[ignore = "..."]`).
fn collect_fn_names(file: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let bytes = text.as_bytes();
    let mut names = Vec::new();
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] != b'f' || bytes[i + 1] != b'n' {
            i += 1;
            continue;
        }
        // `fn` must be a whole word: the preceding byte must not be an
        // identifier character.
        if i > 0 && is_ident_byte(bytes[i - 1]) {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        // Skip whitespace between `fn` and the name.
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        // Read the identifier.
        let name_start = j;
        while j < bytes.len() && is_ident_byte(bytes[j]) {
            j += 1;
        }
        if j == name_start {
            i += 2;
            continue;
        }
        let name = &text[name_start..j];
        // Skip whitespace between the name and the argument list.
        let mut k = j;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        let is_empty_args = k + 1 < bytes.len() && bytes[k] == b'(' && bytes[k + 1] == b')';
        // Only keep `#[test]` functions that are not `#[ignore]`d: an
        // ignored test never runs and cannot prove a requirement.
        if is_empty_args
            && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && matches!(classify_attributes(&text, i), TestAttr::Test)
        {
            names.push(name.to_string());
        }
        i = j;
    }
    names
}

/// What the attribute run above a function says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestAttr {
    /// No test attribute: the function is a plain helper.
    NotATest,
    /// A test attribute is present and the test is not ignored.
    Test,
    /// A test attribute is present but the test carries `#[ignore]`,
    /// so it never runs and cannot prove a requirement.
    IgnoredTest,
}

/// Classify the attribute run above the function whose `fn` keyword
/// starts at `fn_start`.
///
/// Scans backwards from the `fn` keyword over the contiguous run of
/// `#[...]` attributes and `//` doc comments above it, and classifies
/// the run.
///
/// The doc-comment skip matters: this workspace writes its test
/// docstrings *between* the attribute and the function
/// (`#[test]` / ``/// `accepts nfc path`.`` / `fn accepts_nfc_path()`),
/// so a scan that stops at the first non-`]` byte above the `fn` would
/// miss most of the workspace's tests.
///
/// `#[ignore]` matters for a different reason: an ignored test never
/// runs, so naming one in a coverage row would claim coverage that is
/// never exercised. The whole attribute run is collected — `#[test]`
/// and `#[ignore]` may appear in either order — before the verdict is
/// decided.
fn classify_attributes(text: &str, fn_start: usize) -> TestAttr {
    let bytes = text.as_bytes();
    // `cursor` points just below the next thing to inspect. We walk it
    // backwards over whitespace, an optional `async` keyword, doc
    // comments and attributes.
    let mut cursor = fn_start;
    let mut has_test = false;
    let mut has_ignore = false;

    loop {
        // Skip whitespace backwards (this crosses newlines too).
        while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
            cursor -= 1;
        }
        // An `async` keyword may sit between the attribute run and
        // `fn`. Skip it so `#[tokio::test] async fn an_async_test()` is
        // recognised the same way as the sync form. `continue` so the
        // whitespace scan above runs again and lands on the attribute.
        if cursor >= 5 && &bytes[cursor - 5..cursor] == b"async" {
            let before = cursor.saturating_sub(6);
            if before == 0 || !is_ident_byte(bytes[before]) {
                cursor -= 5;
                continue;
            }
        }
        if cursor == 0 {
            break;
        }

        // A `//` or `///` comment line above the function? Skip the
        // whole line and keep looking.
        let line_start = line_start_of(bytes, cursor - 1);
        let line = text[line_start..cursor].trim_start();
        if line.starts_with("//") {
            cursor = line_start;
            continue;
        }

        // The next thing above must be the closing `]` of an attribute.
        if bytes[cursor - 1] != b']' {
            break;
        }
        let close = cursor - 1;

        // Find the `[` that opens this attribute. We track the position
        // OF the `[` itself, so the body is `text[open + 1..close]`.
        let mut open = close;
        while open > 0 && bytes[open] != b'[' {
            open -= 1;
        }
        if open == 0 || bytes[open] != b'[' {
            break;
        }
        if open == 0 || bytes[open - 1] != b'#' {
            break;
        }

        // The attribute body is everything between `[` and `]`.
        let body = &text[open + 1..close];
        let body = body.trim();
        // The attribute *path* is what comes before any `= value`
        // part: `ignore = "reason"` has the path `ignore`.
        let path = body.split('=').next().unwrap_or(body).trim();
        // `test`, `tokio::test`, `rstest`, … — the last path segment
        // must be `test`. `cfg(test)` has no `::`, so its last segment
        // is the whole path and does NOT match.
        let last_segment = path.rsplit("::").next().unwrap_or(path).trim();
        if last_segment == "test" {
            has_test = true;
        } else if last_segment == "ignore" {
            has_ignore = true;
        }
        // Keep walking: another attribute may sit above this one.
        cursor = open.saturating_sub(2);
    }

    if !has_test {
        TestAttr::NotATest
    } else if has_ignore {
        TestAttr::IgnoredTest
    } else {
        TestAttr::Test
    }
}

/// The index of the first byte of the line containing `pos`.
fn line_start_of(bytes: &[u8], pos: usize) -> usize {
    let mut i = pos;
    while i > 0 && bytes[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

/// Whether the byte can appear inside a Rust identifier.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Walk a directory recursively, returning every `.rs` file path.
fn walk_rs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rs(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// Every test fn name present in the workspace, keyed by owning crate.
///
/// The crate key matters. Flattening every crate into one set would
/// let a coverage row claim `crate_name: "elang-epf"` while naming a
/// test that actually lives in `elang-conformance` — the row would
/// still "pass" the existence check while pointing at the wrong crate.
/// Keying by crate makes the row's `crate_name` field part of the
/// guarantee rather than decoration.
fn all_test_names() -> BTreeMap<String, BTreeSet<String>> {
    let mut names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let Some(root) = workspace_root() else {
        return names;
    };
    for crate_dir in std::fs::read_dir(root.join("crates"))
        .expect("read crates/")
        .flatten()
        .map(|e| e.path())
    {
        let Some(crate_name) = crate_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let entry = names.entry(crate_name.to_string()).or_default();
        for sub in ["src", "tests"] {
            let p = crate_dir.join(sub);
            if !p.exists() {
                continue;
            }
            for file in walk_rs(&p) {
                for n in collect_fn_names(&file) {
                    entry.insert(n);
                }
            }
        }
    }
    names
}

#[test]
fn every_named_test_in_the_coverage_table_exists() {
    // Per-crate lookup, so a row must name a test that lives in the
    // crate it claims — not merely a test somewhere in the workspace.
    let available = all_test_names();
    let mut missing: Vec<String> = Vec::new();
    for row in rows() {
        if row.kind == CoverageKind::Meta || row.kind == CoverageKind::Doc {
            continue;
        }
        let found = available
            .get(row.crate_name)
            .is_some_and(|names| names.contains(row.test));
        if !found {
            missing.push(format!(
                "{}::{} (crate `{}`)",
                row.requirement, row.test, row.crate_name
            ));
        }
    }
    missing.sort_unstable();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "{} named test(s) do not exist in the claimed crate:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

#[test]
fn the_scanner_rejects_non_test_helpers() {
    // The scanner matches `fn name()` — but the workspace is full of
    // zero-argument helpers that are NOT tests. If the scanner accepted
    // those, a coverage row could name `fn minimal_groups()` and pass
    // the existence check while proving nothing. This pins the
    // `#[test]`-attribute requirement.
    let dir = std::env::temp_dir().join(format!("elang-scanner-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create probe dir");
    let file = dir.join("probe.rs");
    std::fs::write(
        &file,
        r#"
fn helper_without_attribute() {}

#[allow(dead_code)]
fn helper_with_other_attribute() {}

#[test]
fn a_real_test() {}

#[test]
#[ignore = "provenance probe"]
fn an_ignored_test() {}

#[ignore = "provenance probe"]
#[test]
fn an_ignored_test_reversed_order() {}

#[tokio::test]
async fn an_async_test() {}
"#,
    )
    .expect("write probe file");

    let names = collect_fn_names(&file);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        names.contains(&"a_real_test".to_string()),
        "a `#[test]` fn must be collected: {names:?}"
    );
    assert!(
        !names.contains(&"an_ignored_test".to_string()),
        "an `#[ignore]`d test never runs and must NOT be collected: {names:?}"
    );
    assert!(
        !names.contains(&"an_ignored_test_reversed_order".to_string()),
        "an `#[ignore]`d test must NOT be collected in either attribute order: {names:?}"
    );
    assert!(
        !names.contains(&"helper_without_attribute".to_string()),
        "a bare fn with no attribute must NOT be collected: {names:?}"
    );
    assert!(
        !names.contains(&"helper_with_other_attribute".to_string()),
        "a fn with a non-test attribute must NOT be collected: {names:?}"
    );
    assert!(
        names.contains(&"an_async_test".to_string()),
        "a `#[tokio::test] async fn` is a recognised test: {names:?}"
    );
}

#[test]
fn the_runner_path_never_panics_on_malformed_input() {
    // The crate-level lint allow permits `expect` / `panic!` in the
    // fixture AUTHORING helpers, where a panic is the intended
    // fail-closed mode. The runner is different: it consumes the
    // deliberately-malformed archives this crate exists to test, so a
    // panic there would crash the suite instead of reporting a
    // conformance failure.
    //
    // This pins that boundary: the production portion of `corpus.rs`
    // (everything before its `#[cfg(test)]` module) must contain no
    // `expect(`, `panic!`, or `unwrap()`.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/corpus.rs");
    let text = std::fs::read_to_string(&path).expect("corpus.rs is readable");
    let production = match text.find("#[cfg(test)]") {
        Some(idx) => &text[..idx],
        None => text.as_str(),
    };
    for needle in ["expect(", "panic!", "unwrap()", "unreachable!"] {
        assert!(
            !production.contains(needle),
            "the runner path (crates/elang-conformance/src/corpus.rs) must \
             not contain `{needle}` — it consumes untrusted-shaped bytes \
             and must report a conformance failure, not abort the suite"
        );
    }
}

#[test]
fn workspace_root_resolves() {
    assert!(
        workspace_root().is_some(),
        "could not locate the workspace root from CARGO_MANIFEST_DIR"
    );
}

#[test]
fn crates_directory_is_scannable() {
    let root = workspace_root().expect("workspace root");
    assert!(
        root.join("crates").is_dir(),
        "crates/ must exist under the workspace root"
    );
}

// ---- Governance / process meta-tests --------------------------------------
//
// These map to REQ-GOV-* requirements. They are not Rust tests that
// exercise runtime behaviour — they assert the existence of a documented
// artifact the requirement asks for. Each one lives in its own
// `#[test]` so a missing file fails a single named assertion and the
// failure message points to the requirement that broke.

fn file_present(relative: &str) -> bool {
    workspace_root()
        .expect("workspace root")
        .join(relative)
        .is_file()
}

fn dir_present(relative: &str) -> bool {
    workspace_root()
        .expect("workspace root")
        .join(relative)
        .is_dir()
}

#[test]
fn ci_matrix_runs_linux_windows_macos() {
    let text = std::fs::read_to_string(
        workspace_root()
            .expect("root")
            .join(".github/workflows/ci.yml"),
    )
    .expect("ci.yml present");
    for os in ["ubuntu-latest", "windows-latest", "macos-latest"] {
        assert!(
            text.contains(os),
            "ci.yml must run on {os} (covers REQ-CONF-005 / REQ-GOV-CI-001 / REQ-GOV-WORKSPACE-001)"
        );
    }
}

#[test]
fn crates_match_adr_0002_decomposition() {
    // ADR 0002 is the crate-decomposition decision. The conformance crate
    // is one of the crates it enumerates.
    assert!(
        file_present("docs/decisions/0002-crate-decomposition.md"),
        "ADR 0002 must exist (covers REQ-GOV-WORKSPACE-002)"
    );
    assert!(dir_present("crates/elang-conformance"));
    assert!(dir_present("crates/elang-reader"));
    assert!(dir_present("crates/elang-writer"));
}

#[test]
fn docs_present() {
    for path in [
        "README.md",
        "LICENSE",
        "CONTRIBUTING.md",
        "CODE_OF_CONDUCT.md",
        "SECURITY.md",
        "docs/specification.md",
        "docs/conformance.md",
    ] {
        assert!(file_present(path), "{path} must exist (REQ-GOV-DOCS-001)");
    }
}

#[test]
fn adrs_present() {
    assert!(
        dir_present("docs/decisions"),
        "docs/decisions/ must exist (REQ-GOV-ADR-001)"
    );
    let entries = std::fs::read_dir(workspace_root().expect("root").join("docs/decisions"))
        .expect("read docs/decisions");
    let count = entries
        .flatten()
        .filter(|e| {
            e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_string_lossy().starts_with("000")
        })
        .count();
    assert!(count >= 1, "at least one ADR must be recorded");
}

#[test]
fn traceability_present() {
    assert!(file_present("docs/traceability.md"));
}

#[test]
fn ci_workflow_present() {
    assert!(file_present(".github/workflows/ci.yml"));
}

#[test]
fn supply_chain_doc_present() {
    assert!(file_present("docs/supply-chain.md"));
}

#[test]
fn fixture_naming_convention_present() {
    // Either a README in fixtures/ or the expected-result schema.
    let present =
        file_present("fixtures/README.md") || file_present("fixtures/expected-result.schema.json");
    assert!(present, "fixture-naming convention artifact must exist");
}

#[test]
fn release_strategy_doc_present() {
    assert!(file_present("docs/release-strategy.md"));
}

#[test]
fn open_artifacts_doc_present() {
    assert!(file_present("docs/open-artifacts.md"));
}
