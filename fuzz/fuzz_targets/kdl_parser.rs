// Fuzz target for the KDL 2.0 parser and AST construction (Phase 15,
// T1504).
//
// Drives arbitrary byte sequences through `elang_kdl::parse_with_budgets`, the
// hand-rolled recursive-descent parser that every higher-layer
// document format (EPF, ELD, integrity manifest) is built on.
//
// The parser is the widest attack surface in the workspace: it runs
// before any schema validation, so every malformed byte in an archive
// reaches it. Its contract is:
//
// - UTF-8 is validated up front (`validate_utf8`), so the lexer never
//   sees a partial scalar.
// - Every budget in `elang_core::limits` is enforced *during* parsing
//   (nesting depth, encoded token bytes, decoded string bytes, comment
//   bytes, node count), not after allocation.
// - Every rejectable condition produces a structured `ParseError`
//   carrying a registry-aligned `ErrorCode`.
// - No input may cause a panic, an unbounded allocation, or a read past
//   the end of the input slice.
//
// The AST construction half matters as much as the lexer half: a
// malformed document must not be able to produce a `KdlDocument` with
// inconsistent node counts, dangling spans, or a node whose children
// disagree with its own span.
//
// See `docs/security-testing.md` for budget and seed corpus policy.

#![no_main]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fuzz harness drives adversarial inputs through the KDL parser; \
              panics are the failure signal that surfaces robustness defects in the \
              lexer, parser and AST builder under test"
)]

use elang_kdl::{parse_with_budgets, BudgetTracker};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // A fresh tracker per call: the archive-wide counters are the
    // reader's to manage, and a fuzz input must not be able to leak
    // budget state into the next input.
    let mut budgets = BudgetTracker::new();

    // `parse_with_budgets` is the public entry point: UTF-8 validation → lex →
    // parse → AST. A structured `ParseError` is the expected outcome
    // for adversarial input; a panic is a defect.
    let Ok(doc) = parse_with_budgets(data, &mut budgets) else {
        return;
    };
    if !doc.nodes.is_empty() {
        debug_assert!(budgets.nodes_archive().current() > 0);
    }

    // The AST was built. Walk it so the fuzzer also exercises the
    // traversal helpers that every consumer (EPF, ELD, integrity
    // manifest) depends on — a panic in an accessor is just as much a
    // defect as one in the parser.
    let mut nodes = 0usize;
    let mut pending: Vec<_> = doc.nodes.iter().collect();
    while let Some(node) = pending.pop() {
        nodes = nodes.saturating_add(1);
        // `name`/`args`/`props`/`children` are the accessors the
        // schema validators call first.
        let _ = node.name();
        let _ = node.args().count();
        let _ = node.props().count();
        if let Some(children) = node.children() {
            pending.extend(children.iter());
        }
    }
    // The node count must be bounded by the per-resource budget the
    // parser enforced; a value above the ceiling would mean the
    // budget was checked after allocation.
    debug_assert!(nodes <= elang_core::limits::MAX_NODES_PER_RESOURCE as usize);
});
