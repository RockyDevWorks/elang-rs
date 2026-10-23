//! # elang-kdl
//!
//! Constrained KDL 2.0 parsing and canonical emission used by the EPF, ELD
//! and integrity-manifest layers. Enforces the parser budgets from the
//! steering contract (§10) during parsing, not after allocation.
//!
//! Implemented in Phase 5 (Issue #15, ADR 0010).
//!
//! ## Entry points
//!
//! - [`parse`] — parse a UTF-8 byte slice as a [`KdlDocument`].
//! - [`parse_with_budgets`] — parse with a caller-supplied budget
//!   tracker (useful for parsing multiple resources sequentially).
//! - [`canon::emit_value`] — emit a canonical KDL 2.0 double-quoted
//!   string for any [`KdlValue`].
//!
//! ## Module layout
//!
//! | Module | Purpose |
//! | --- | --- |
//! | [`span`] | `SourceSpan` / `Position` byte-offset tracking |
//! | [`value`] | `KdlValue` enum (Null / Bool / Integer / Float / String) |
//! | [`ast`] | `KdlDocument`, `KdlNode`, `KdlEntry`, `KdlChildren` |
//! | [`escape`] | KDL 2.0 escape processing and canonical string encoding |
//! | [`lex`] | Token stream with budget-aware failure |
//! | [`budget`] | `BudgetTracker` enforcing steering §10 limits |
//! | [`error`] | `ParseError` + `ErrorCode` (registry-aligned) |
//! | [`parse`] | Recursive-descent parser building an AST |
//! | [`canon`] | Canonical double-quoted string emitter |
//!
//! See ADR 0010 in `docs/decisions/0010-kdl-parser-hand-roll.md` for the
//! hand-roll-vs-`kdl-rs` decision rationale.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod ast;
pub mod budget;
pub mod canon;
pub mod error;
pub mod escape;
pub mod lex;
pub mod parse;
pub mod span;
pub mod value;

pub use ast::{KdlChildren, KdlDocument, KdlEntry, KdlNode, KdlVersionPragma};
pub use budget::{Budget, BudgetTracker};
pub use error::{ErrorCode, ParseError};
pub use lex::{parse_float, parse_integer, Token};
pub use parse::{parse, parse_with_budgets, validate_utf8};
pub use span::{Position, SourceSpan};
pub use value::KdlValue;
