# Architecture

The reference implementation (`elang-rs`) is a Rust workspace split into small,
focused crates with a strict acyclic dependency graph rooted at `elang-core`.
Parsing, semantic validation, and serialization are kept separate so each can be
fuzzed, benchmarked, and reviewed independently.

## Crate graph

```text
                         elang-core
            (IDs, errors, limits, checked math, models)
                              ▲
     ┌───────────┬────────────┼───────────┬──────────────┐
     │           │            │           │              │
 elang-kdl  elang-container  elang-repertoire            │
     ▲           ▲            ▲           ▲               │
     │           │            │           │               │
 ┌───┴───┬───────┘        ┌───┴────┐      │               │
 │       │                │        │      │               │
elang-epf elang-integrity elang-eld┘──────┘               │
     ▲       ▲                ▲                            │
     └───────┴────────┬───────┘                           │
                      │                                    │
               elang-conformance                           │
                      ▲                                    │
                      └──────────── elang-cli ─────────────┘
```

Edges point from a dependent crate to its dependency.

## Crates

| Crate | Owns |
|-------|------|
| `elang-core` | Identifiers, error model and stable codes, limits, checked arithmetic, bounded reader, validated model primitives. |
| `elang-unicode` | Pinned Unicode 18.0.0 General_Category + NFC + noncharacter checks, generated from the official UCD. |
| `elang-kdl` | Constrained KDL 2.0 parsing and canonical emission; parser budgets. |
| `elang-container` | Header/index/footer, path validation, frame I/O, Zstd reader/encoder, reader state machine, writer. |
| `elang-repertoire` | IPA allowlist, IPA identifier validation, JCS + SHA-256 verification. |
| `elang-epf` | `package.epf` schema and validation. |
| `elang-integrity` | Integrity manifest parsing and SHA-256 verification. |
| `elang-eld` | Phoneme model, IPA parser, `when` AST/evaluator, canonical ELD writer. |
| `elang-conformance` | Fixture runner and golden comparisons. |
| `elang-cli` | CLI commands, diagnostics output, safe unpack. |

## Design principles

- **Validated newtypes.** Resource paths, IPA identifiers, romanizations, group
  names, and language identifiers are types whose construction is fallible —
  "valid but unrepresentable-if-invalid." Holding the type means the value passed
  validation.
- **Separation of concerns.** Parsing, semantic validation, and serialization are
  distinct phases and distinct crates.
- **One source of truth for limits.** A single `limits` module defines every
  budget; parsers read from it.
- **Typed, stable errors.** A category enum plus stable string codes, propagated
  with `Result`, carrying resource path, byte offset, and KDL span where
  available.
- **Distinct result types per mode.** `inspect`, `verify`, and `validate` return
  different types so partially validated state can never be mistaken for a fully
  validated package.

## Build, test, CI

- Pinned `stable` toolchain via `rust-toolchain.toml`; MSRV 1.90 enforced in CI.
- CI gates: `cargo fmt --check`, `cargo clippy` (deny warnings), `cargo test`,
  a docs build, a dependency/license audit, and cross-platform builds on Linux,
  Windows, and macOS.
- Reproducible release builds (`lto=thin`, `codegen-units=1`, pinned deps,
  committed lock file) so canonical golden hashes reproduce bit-for-bit.

!!! note
    This documentation site is built and deployed by a separate GitHub Actions
    workflow (`docs.yml`) and does not touch the crate's CI (`ci.yml`) or
    security (`security.yml`) workflows.
