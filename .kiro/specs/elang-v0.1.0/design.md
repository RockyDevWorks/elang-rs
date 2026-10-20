# ELANG v0.1.0 — Design

> Companion to [`requirements.md`](requirements.md). This document describes the
> architecture: crate graph, data flow through the 12 validation stages, the
> threat model, ownership, and the key type contracts. It does not restate
> normative rules — those live in the steering contract and `requirements.md`.

## 1. Architecture overview

ELANG is split into nine crates with a strict acyclic dependency graph rooted at
`elang-core`. Parsing, semantic validation and serialization are kept separate
so each can be fuzzed, benchmarked and reviewed independently
([ADR 0002](../../../docs/decisions/0002-crate-decomposition.md)).

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

Edges point from dependent to dependency. `elang-cli` depends on the library
crates and the conformance runner; it holds no validation logic itself.

| Crate | Owns | Depends on |
|---|---|---|
| `elang-core` | identifiers, error model + codes, limits, checked arithmetic, bounded reader, validated model primitives | — |
| `elang-kdl` | constrained KDL 2.0 parse + canonical emit, parser budgets | core |
| `elang-container` | header/index/footer, path validation, frame I/O, Zstd reader/encoder, reader state machine, writer | core |
| `elang-repertoire` | pinned Unicode 18.0.0 tables, IPA allowlist, JCS + SHA-256 verify | core |
| `elang-epf` | `package.epf` schema | core, kdl |
| `elang-integrity` | integrity manifest parse + SHA-256 verify | core, kdl |
| `elang-eld` | phoneme model, IPA parser, `when` AST/evaluator, canonical ELD writer | core, kdl, repertoire |
| `elang-conformance` | fixture runner, golden comparisons | core, container, epf, eld, integrity, repertoire |
| `elang-cli` | CLI commands, diagnostics output, safe unpack | all library crates + conformance |

## 2. Data flow — the 12 validation stages

The reader is a fail-closed state machine (steering §4). Each stage has typed
inputs/outputs and produces stage-tagged errors. `inspect` stops after 7,
`verify` after 11, `validate` runs all 12.

```text
bytes ─▶ (1) preflight: size/bounds/minimal structure           [container]
      ─▶ (2) header decode + validate (magic, versions, flags)   [container]
      ─▶ (3) footer decode + offset consistency (checked)        [container]
      ─▶ (4) SHA-256 of exact index bytes == footer hash         [container+integrity]
      ─▶ (5) index parse: entries, paths (NFC, order, dup)       [container]
      ─▶ (6) physical frame layout: contiguity, no gaps/overlap  [container]
      ─▶ (7) per-frame header inspect: size/window/checksum/dict [container]
      ───────────── inspect ends here ─────────────
      ─▶ (8) decode+validate Header/format.version & mimetype    [container]
      ─▶ (9) parse+validate integrity manifest structure         [integrity+kdl]
      ─▶ (10) parse+validate EPF                                 [epf+kdl]
      ─▶ (11) decompress ALL resources (bounded) + SHA-256 verify[container+integrity]
      ───────────── verify ends here ─────────────
      ─▶ (12) ELD semantic validation                            [eld+repertoire+kdl]
      ───────────── validate ends here ─────────────
```

Stage 11 is the only stage that decompresses every resource. `inspect`
deliberately does **not** verify integrity (REQ-READER-003); the public API
returns distinct result types so callers cannot mistake inspection for
verification (REQ-CORE-MODEL-007).

### ELD validation pipeline (stage 12 detail)

```text
KDL AST (budgeted) ─▶ structural schema (groups/roots/allophones)
                   ─▶ IPA identifier parse + repertoire allowlist check
                   ─▶ romanization rules (regex, case-insensitive uniqueness, inheritance)
                   ─▶ when AST build (arity/depth/predicate budgets)
                   ─▶ reference resolution (forward/self/allophone→root)
                   ─▶ global uniqueness (IPA incl. tie-bar aliases)
                   ─▶ validated immutable model  ─▶ (optional) canonical ELD bytes
```

The condition **evaluator** is a separate, pure function over a supplied context
(word position, generator-assigned syllable roles, neighbors). It returns an
eligibility set, never a probability or priority (REQ-WHEN-015).

## 3. Writer / canonical pipeline

```text
validated EPF + validated ELD (+ permitted resources)
  ─▶ canonicalize each resource (ELD profile locked; EPF/integrity profiles are open artifacts)
  ─▶ compute integrity manifest over exact uncompressed bytes (excluding itself)
  ─▶ sort resources by canonical UTF-8 path
  ─▶ encode one independent Zstd frame per resource (profile: Fast/Compact/Canonical)
  ─▶ lay out: [32B header][frames...][16B index header][entries][64B footer]
  ─▶ footer carries SHA-256 of exact index bytes
```

Canonical frames use the locked `elang-canonical-zstd-v1` settings and must be
byte-reproducible across runs and platforms (REQ-FRAME-010/011). The writer may
buffer a single resource to call `ZSTD_compress2` once; the reader stays
streaming.

## 4. Threat model (steering §3.5, §10; SECURITY.md)

All container bytes, KDL text and compressed frames are **untrusted**.

| Threat | Vector | Defense | Requirements |
|---|---|---|---|
| Decompression bomb | Huge declared/real output | Bounded streaming decode; per-resource caps; 8 GiB aggregate budget; declared-size cross-check; window ≤ 128 MiB | REQ-FRAME-004/008, REQ-PKG-004 |
| Integer overflow | Crafted offsets/sizes | Checked arithmetic helpers; `overflow-checks` on; range validation | REQ-CORE-MODEL-005, REQ-BIN-006/007 |
| OOB / panic | Truncated/garbage structures | No unchecked indexing; lint-denied `unwrap`/`panic`; fuzzing | REQ-CORE-MODEL-008, REQ-BIN-018 |
| Path traversal | `..`, absolute, symlink, backslash | Strict NFC relative-path validation; safe unpack policy | REQ-BIN-011/012, REQ-CLI-003 |
| Parser abuse | Deep nesting, huge tokens/strings/nodes | Budgets enforced during parse | REQ-LIMIT-001..010 |
| Frame abuse | Dict/skippable/concatenated/trailing | One-frame contract; reject disallowed frame features | REQ-FRAME-002/005/006 |
| False trust | Treating hash/URN as authorship | Documented non-goal; integrity ≠ signature | REQ-INTEGRITY-008 |

### Memory-safety posture

Safe Rust by default; `unsafe` lint-denied except at the audited libzstd FFI
boundary. No panics or unchecked indexing on untrusted input. Checked integer
and offset math everywhere (REQ-CORE-MODEL-005/008).

## 5. Key type contracts (elang-core)

- **Version domains** (`version_domains`): locked constants for spec/container/
  EPF/ELD/canonical-profile (implemented); plus independent typed versions and a
  strict `MAJOR.MINOR.PATCH` parser with no leading zeros (T0201/T0202).
- **Validated newtypes:** resource path, IPA identifier, romanization, group
  name, language identifier. Construction is fallible and the only way to obtain
  the type, so "valid" is unrepresentable-if-invalid (T0203/T0204).
- **Limits:** a single `limits` module with documented units is the source of
  truth for every budget in REQ-LIMIT; parsers read from it (T0206).
- **Errors:** category enum + stable string codes, `Result`-based, carrying
  resource path / byte offset / KDL span; diagnostics capped at 100 without
  masking failure (T0209–T0211).
- **Checked math:** `checked_*` helpers for the conversions and offset
  arithmetic the container layer needs (T0207).

## 6. Ownership and process

- Each crate has a primary owner (the `Owner` column in `requirements.md`).
- Locked rules change only via an approved ADR
  ([ADR 0001](../../../docs/decisions/0001-record-architecture-decisions.md)).
- Open normative artifacts are release blockers
  ([ADR 0003](../../../docs/decisions/0003-open-normative-artifacts.md)); the
  IPA validator, integrity reader/writer and conformance freeze cannot proceed
  until their artifacts are ratified.
- Every `MUST` requirement maps to at least one task and one test; the mapping is
  tracked in [`docs/traceability.md`](../../../docs/traceability.md).

## 7. Build, test, CI

- Pinned `stable` toolchain via `rust-toolchain.toml`; MSRV 1.90 enforced in CI.
- CI gates: `cargo fmt --check`, `cargo clippy` (deny warnings), `cargo test`,
  doc build, dependency/license audit, and cross-platform build on
  Linux/Windows/macOS (REQ-GOV-CI-001).
- Release builds are reproducible (`lto=thin`, `codegen-units=1`, pinned deps,
  committed `Cargo.lock`) so canonical golden hashes reproduce (REQ-FRAME-011).

## 8. Phase ordering (dependency-driven)

Phase 0 (this scaffold) unblocks everything. Phases 1 (normative artifacts) and
2 (core) run next; Phase 1's open artifacts block Phases 7 (integrity) and 8
(IPA) and the conformance freeze. Container (3) + KDL (5) feed EPF (6), integrity
(7) and ELD (8–11). The writer/CLI (13) and conformance (14) sit on top, followed
by fuzzing/hardening (15) and docs/release (16). See the dependency map at the
end of [`tasks.md`](tasks.md).
