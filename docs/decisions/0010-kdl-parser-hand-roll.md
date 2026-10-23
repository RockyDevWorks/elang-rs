# ADR 0010: Hand-rolled KDL 2.0 parser for `elang-kdl`

- **Status:** Approved
- **Date:** 2026-10-23
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-EPF-013, REQ-ELD-013, REQ-INT-013, REQ-KDL-001..010
- **Related tasks:** T0501–T0515

## Context

KDL 2.0 is the surface grammar for EPF (`package.epf`), ELD (`*.eld`) and the
integrity manifest (`META-INF/integrity.kdl`). The shared parser adapter
lives in `elang-kdl` and is consumed by `elang-epf`, `elang-eld`, and
`elang-integrity`.

The steering contract (§10) imposes a fixed set of parser budgets that must
be enforced **before unbounded allocation**, not after:

- KDL syntactic nesting depth ≤ 64
- encoded token ≤ 1 MiB, decoded string ≤ 1 MiB
- node / property name ≤ 256 UTF-8 bytes
- ≤ 256 positional args, ≤ 256 properties per node
- ≤ 2 000 000 nodes per resource, ≤ 10 000 000 nodes per archive
- ≤ 1 MiB per comment, ≤ 16 MiB comment bytes per resource
- ≤ 256 MiB decoded string bytes per resource, ≤ 1 GiB per archive
- ≤ 10 000 000 semantic references per archive, traversal depth ≤ 128

The parser must also retain exact source spans for every node, entry and
value, reject invalid UTF-8 at the input boundary, accept the full KDL 2.0
whitespace and comment surface, and emit canonical standard double-quoted
strings for the canonical serializer.

The only serious third-party option is the `kdl` crate (`kdl-rs`) on crates.io.
It is a competent KDL 1.x/2.0 parser and exposes source spans, but it does
**not** enforce the budget list above and does not provide a canonical emitter
that matches the ELANG canonicalization rules (which diverge from `kdl`'s
default formatter).

## Decision

Hand-roll the KDL 2.0 parser inside `elang-kdl` as a single-crate, zero-extra-
dependency lexer + recursive-descent parser + AST + canonical emitter + fuzz
tests. Budget rejections return a structured `ParseError` with an `ErrorCode`
whose category is `Limit`. The implementation and tests below describe the
checks currently in place; this ADR is not a proof of allocation bounds.

The implementation is split into:

| Module | Responsibility |
| --- | --- |
| `span` | `SourceSpan` / `Position` (byte offsets + line/column, UTF-8 boundary safe) |
| `value` | `KdlValue` enum (Null / Bool / Integer / Float / String) |
| `ast` | `KdlDocument`, `KdlNode`, `KdlEntry`, `KdlChildren` |
| `escape` | KDL 2.0 escape processing (`\n`, `\u{...}`, `\t`, `\"`, `\\`, …) and canonical encoding |
| `lex` | Token stream with budget-aware failure |
| `budget` | `BudgetTracker` for cumulative node, string and comment totals; other limits checked locally |
| `error` | `ParseError` + `ErrorCode` (wraps `elang_core::error::Category`) |
| `parse` | Recursive-descent parser building an AST |
| `canon` | Canonical double-quoted string emitter for `KdlValue::String` |
| `tests` | Inline unit tests for each module + KDL 2.0 spec fixtures + boundary tests |

External dependency surface: only `elang-core` (for limits, errors, ids). No
new third-party crates. `crates/elang-kdl/tests/fuzz.rs` uses a deterministic
PRNG and pathological byte patterns in ordinary `cargo test` tests. It is not
a `cargo-fuzz`/libFuzzer harness, and no separate CI fuzzing job is configured.

## Consequences

- **Budget checks have distinct scopes.** `BudgetTracker` charges cumulative
  resource/archive node and decoded-string totals and resource comment bytes.
  Nesting depth is checked by `parse_node`'s depth parameter, not charged to a
  `nesting_depth` budget. Per-node argument/property counters reject an entry
  before adding it to the entries vector (replacing the earlier implementation
  that collected properties before counting). Lexer checks bound encoded
  token, individual comment and name sizes. These checks do not establish a
  general guarantee about all allocations from attacker-controlled input.
- **Source spans are exact and stable** for every AST node because the parser
  carries a single `(start, end)` pair from tokenisation through AST
  construction. Schema layers (`elang-epf`, `elang-eld`, `elang-integrity`)
  can therefore emit diagnostics that reference the offending byte range
  with no ambiguity.
- **Canonical double-quoted emitter is owned end-to-end.** It implements the
  ELANG canonicalization rules (escape set E-3 in ADR 0009: `"` and `\`
  always escaped; control characters `\b`, `\f`, `\n`, `\r`, `\t` and U+0000
  escaped; other C0 / C1 escaped via `\u{...}`).
- **Correctness is now our problem.** The KDL 2.0 grammar is non-trivial; the
  unit + spec-fixture + boundary + fuzz test suite is the safety net.
  Representative spec fixtures and boundary tests live in
  `crates/elang-kdl/tests/spec_fixtures.rs`; they are not the full published
  reference corpus.
- **No third-party parsing dep added.** Keeps the supply-chain surface flat
  (consistent with ADR 0007 / 0008).
- **Schema-layer rejection (T0511) lives at the consumer** (`elang-epf`,
  `elang-eld`, `elang-integrity`). `elang-kdl` exposes the full AST including
  duplicates and unknown fields so the schema layer can enforce its own rules
  with exact source spans.

## Alternatives considered

- **`kdl` (`kdl-rs`) crate**: rejected. Battle-tested for KDL 2.0 but does
  not enforce the steering-§10 budget list, and its default formatter does
  not match the ELANG canonicalization rules. Wrapping it to add the budgets
  would require roughly the same amount of code as hand-rolling, and we
  would still own the canonical emitter.
- **Pest / LALRPOP grammar**: rejected. Grammar-grammar tools add a build-
  time dependency and obscure the budget-tracking code path. The error
  messages are harder to align with the per-line / per-column diagnostics
  the steering contract expects.
- **Hand-roll only the lexer, leave the parser to `kdl-rs`**: rejected for
  the same reason as the wrapped-`kdl-rs` option, plus the additional
  integration cost.

## Exit criterion

ADR 0010 is superseded if and only if (a) a published KDL 2.0 reference parser
that meets every steering-§10 budget and (b) a canonical emitter that matches
ADR 0009 E-3 are both maintained upstream and pinned in the workspace, with
the project still owning the budget-enforcement wrapper around them.
