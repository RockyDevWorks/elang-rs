# Architecture

The reference implementation (`elang-rs`) is a Rust workspace split into small,
focused crates with a strict acyclic dependency graph rooted at **two** root
crates — `elang-core` and `elang-unicode` — neither of which depends on any
other crate in the workspace. Parsing, semantic validation, and serialization
are kept separate so each can be fuzzed, benchmarked, and reviewed independently.

## Crate graph

Approximate layering (the exact edges are listed below):

```text
  application layer:   elang-cli
  tooling layer:       elang-conformance
  pipeline layer:      elang-reader        elang-writer
  format layer:        elang-container   elang-epf   elang-eld   elang-integrity
  parser layer:        elang-kdl         elang-repertoire
  root crates:         elang-core        elang-unicode
```

The authoritative dependency listing, read from each crate's `Cargo.toml`
(`[dependencies]` section). Edges point from a dependent crate to its
dependency:

```text
elang-cli          ──▶ elang-reader, elang-writer, elang-conformance,
                        elang-container, elang-core, elang-epf, elang-eld,
                        elang-integrity, elang-repertoire
elang-conformance  ──▶ elang-reader, elang-writer, elang-container, elang-core,
                        elang-epf, elang-eld, elang-integrity, elang-kdl,
                        elang-repertoire, elang-unicode
elang-reader       ──▶ elang-container, elang-core, elang-epf, elang-eld,
                        elang-integrity, elang-kdl, elang-repertoire
elang-writer       ──▶ elang-container, elang-core, elang-epf, elang-eld,
                        elang-integrity, elang-unicode
elang-eld          ──▶ elang-core, elang-kdl, elang-repertoire
elang-epf          ──▶ elang-core, elang-kdl, elang-unicode
elang-integrity    ──▶ elang-core, elang-kdl, elang-unicode
elang-container    ──▶ elang-core, elang-unicode
elang-repertoire   ──▶ elang-core, elang-unicode
elang-kdl          ──▶ elang-core
elang-core         ──▶ (none)
elang-unicode      ──▶ (none)
```

`elang-unicode` is a **second root crate**, a sibling of `elang-core` — not a
dependency of it. `elang-core` has no dependencies at all, and `elang-unicode`
has no dependencies at all. Several crates depend on both: `elang-container`,
`elang-repertoire`, `elang-epf`, `elang-integrity`, and `elang-writer`.

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
| `elang-reader` | Integrated 12-stage `inspect`/`verify`/`validate` pipeline, bounded resource retrieval, read-only validated-data accessors. |
| `elang-writer` | Builds `.elang` archives from a validated EPF, a validated ELD, and additional permitted resources. |
| `elang-conformance` | Fixture runner and golden comparisons. |
| `elang-cli` | CLI commands, diagnostics output, safe unpack. |

## Design principles

- **Validated newtypes.** Resource paths, IPA identifiers, romanizations, group
  names, and language identifiers are types whose construction is fallible —
  "valid but unrepresentable-if-invalid." Holding the type means the value passed
  validation.
- **Separation of concerns.** Parsing, semantic validation, and serialization
  are distinct phases and distinct crates.
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
