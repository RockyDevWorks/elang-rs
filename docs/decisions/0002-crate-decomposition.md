# ADR 0002: Nine-crate workspace decomposition

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-GOV-WORKSPACE-001, REQ-GOV-WORKSPACE-002
- **Related tasks:** T0001, T0002

## Context

The steering contract (§1.1) recommends a crate layout but explicitly labels it
a recommendation, not a ratified naming contract. We must pick a concrete
decomposition and commit to it so downstream tasks have stable crate boundaries,
while keeping parsing, semantic validation and serialization separable.

## Decision

Adopt a Cargo workspace with nine library/binary crates:

| Crate | Responsibility |
|---|---|
| `elang-core` | IDs, errors, limits, validated model primitives (no I/O) |
| `elang-container` | header/index/footer, frame I/O, reader/writer |
| `elang-kdl` | constrained KDL 2.0 parsing and canonical emission |
| `elang-epf` | `package.epf` schema and validation |
| `elang-eld` | phoneme model, IPA, `when` AST, canonical ELD |
| `elang-repertoire` | pinned Unicode/IPA allowlist and verification |
| `elang-integrity` | integrity manifest and SHA-256 verification |
| `elang-conformance` | fixture runner and interoperability checks |
| `elang-cli` | `inspect`/`verify`/`validate`/`pack`/`unpack`/`canonicalize` |

Dependency direction flows toward `elang-core`; no cycles. The CLI depends on the
library crates and contains no validation logic of its own.

## Consequences

- Clear ownership boundaries; parsing is separable from semantic validation and
  from serialization.
- Crate **names** are now a stable interface; renaming requires a superseding
  ADR.
- Pinned dependency versions are centralized in the workspace `Cargo.toml`.

## Alternatives considered

- **Single crate with modules:** simpler initially but blurs the untrusted-input
  boundary and makes independent fuzzing/benchmarking harder.
- **Finer decomposition (per-layer sub-crates):** more overhead than value for
  v0.1.0.
