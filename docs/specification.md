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
see [ADR 0003](decisions/0003-open-normative-artifacts.md) and the live tracker
in [`open-artifacts.md`](open-artifacts.md)). Machine-checkable artifacts land
under the top-level [`specs/`](../specs/) directory.

Still open — **blocked on external pinned source data or later phases**,
must not be fabricated:

1. **IPA diacritic/modifier inventories (T0106–T0110)** — the allowlist has
   been generated, cross-checked against an independent chart transcription
   and pinned (`specs/schema/ipa-v1.json`, 107 base / 27 combining / 9
   spacing, ADR 0008). The second human review of the diacritic and modifier
   inventories **completed 2026-10-23** (outcome match, `specs/sources/SOURCES.md`
   §7); OA-1/OA-4 are Ratified and this gate is closed.
2. **Canonical ELD golden files + SHA-256 (OA-14)** — produced by the Phase 11
   canonical writer (T1121). The methodology (JCS, deterministic EPF
   ordering) is published; the fixtures themselves depend on the writer
   landing.
3. **Formal freeze + spec checksum/release tag (OA-17)** — depends on (1) and
   (2). Until the freeze lands, no schema or repertoire version is final.

Ratified in Phase 1 (decision-only / contract-derived artifacts):

4. The complete `META-INF/integrity.kdl` grammar and canonical serialization —
   ADR 0004, `specs/grammar/integrity-0.1.0.md`.
5. The canonical EPF serializer profile — ADR 0005,
   `specs/canonical/epf-canonical-serialization.md`.
6. Machine-checkable ELD/EPF grammars and the repertoire JSON Schema —
   `specs/grammar/{epf,eld}-0.1.0.md` and
   `specs/schema/elang-ipa-repertoire-v1.schema.json`.
7. The error-code registry — `specs/registry/error-codes.md` +
   `error-codes.json`. Reference golden fixtures and reproducibility hashes
   still depend on the IPA/Unicode data and remain open.
8. **Binary header/index/footer reference vectors and offsets (T0118, OA-11)**
   — `specs/registry/binary-reference-vectors.json` +
   `fixtures/valid/container/minimal-archive/`. All offsets, sizes and
   SHA-256 digests are computed by the deterministic
   `elang-conformance::binary_vectors` builder and asserted by tests.
9. **Tie-bar equivalence and IPA comparison-key test vectors (T0117, OA-12)**
   — 12 published vectors
   (`specs/registry/ipa-tie-bar-vectors.json`) across positive,
   equivalence and negative cases, driven by the `elang-repertoire`
   integration test `tie_bar_vectors.rs`.
10. **Residual KDL canonicalization edge cases (T0122, OA-16)** — ADR 0009
    pins the authored-byte vs comparison-key views for ordering (E-1),
    the condition sort key (E-2), the exact `"`/`\` escaping set (E-3),
    bare-vs-block allophone emission (E-4/E-5) and layout invariants (E-6).
11. **Unicode 18.0.0 data (T0116, OA-13)** — first-party `elang-unicode`
    generated from the pinned UCD 18.0.0; NFC passes the official
    `NormalizationTest.txt` (20,171 rows); ADR 0007.

Until each open item is ratified, the corresponding code path must not embed
guessed inventories or fabricated hashes.

## Reproducibility and pinning

- **Unicode:** pinned to **18.0.0** (not runtime-dependent tables).
- **IPA chart:** pinned to the IPA **2026** chart issue.
- **libzstd:** pinned to **1.5.7** for canonical frames.
- **Toolchain/dependencies:** pinned via `rust-toolchain.toml` and exact
  versions in the workspace `Cargo.toml` + `Cargo.lock`.
