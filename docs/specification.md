# ELANG Specification (reference index)

> **Specification release:** `0.0.1-1` · **Container/EPF/ELD:** `0.1.0`
>
> This document is the navigation index into the ELANG normative rules. The
> single source of truth for engineering decisions is the steering contract at
> [`.kiro/steering/elang.md`](../.kiro/steering/elang.md). The locked
> requirements with stable IDs live in
> [`.kiro/specs/elang-v0.1.0/requirements.md`](../.kiro/specs/elang-v0.1.0/requirements.md),
> and the architecture in
> [`.kiro/specs/elang-v0.1.0/design.md`](../.kiro/specs/elang-v0.1.0/design.md).

## How the normative material is organized

| Topic | Normative source | Requirement group |
|---|---|---|
| Version domains | steering §1 | REQ-CORE-VERSION-* |
| Package logical resources | steering §2 | REQ-PKG-* |
| Binary container (header/index/footer/paths/sizes/frames) | steering §3 | REQ-BIN-*, REQ-FRAME-* |
| Reader modes & 12 validation stages | steering §4 | REQ-READER-* |
| EPF `package.epf` schema | steering §5 | REQ-EPF-* |
| ELD phoneme grammar & cardinalities | steering §6 | REQ-ELD-* |
| Unicode 18.0.0 & IPA repertoire v1 | steering §7 | REQ-UNICODE-*, REQ-IPA-* |
| `when` condition language | steering §8 | REQ-WHEN-* |
| Canonical ELD serialization | steering §9 | REQ-CANON-* |
| Parser budgets & defensive limits | steering §10 | REQ-LIMIT-* |
| Errors & conformance | steering §11 | REQ-ERR-*, REQ-CONF-* |
| Open artifacts & release gates | steering §12 | REQ-GOV-OPENART-001 |

## Locked vs open

**Locked** (change only via an approved ADR): the 32-byte header, index-entry
layout, 64-byte footer, resource-path rules, size ceilings, the Zstd one-frame
contract, the canonical Zstd profile `elang-canonical-zstd-v1`, the EPF
five-node grammar, the ELD group/root/allophone grammar and cardinalities, the
IPA tie-bar/modifier/length rules, the `when` grammar and semantics, the
canonical ELD serialization and the Canonical Condition Sorting Key v1.

**Open artifacts** (must be produced and reviewed before the conformance freeze;
see [ADR 0003](decisions/0003-open-normative-artifacts.md)):

1. Exact approved IPA allowlist (base symbols, combining diacritics, spacing
   modifiers) for the pinned 2026 IPA chart, with classification and
   canonical-decomposition handling.
2. The canonical (RFC 8785 JCS) SHA-256 digest of the IPA repertoire JSON.
3. The complete `META-INF/integrity.kdl` normative grammar and its canonical
   serialization.
4. The canonical EPF and integrity KDL serializer profiles.
5. Machine-checkable ELD/EPF grammars and JSON Schema.
6. Reference golden fixtures, reproducibility hashes and the error-code
   registry.

Until each is ratified, the corresponding code path must not embed guessed
inventories or fabricated hashes.

## Reproducibility and pinning

- **Unicode:** pinned to **18.0.0** (not runtime-dependent tables).
- **IPA chart:** pinned to the IPA **2026** chart issue.
- **libzstd:** pinned to **1.5.7** for canonical frames.
- **Toolchain/dependencies:** pinned via `rust-toolchain.toml` and exact
  versions in the workspace `Cargo.toml` + `Cargo.lock`.
