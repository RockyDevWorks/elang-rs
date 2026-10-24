# Architecture Decision Records (ADRs)

This directory records significant decisions for the ELANG reference
implementation, especially any that touch **locked** normative rules.

## Rules

- Every change to a locked binary layout, schema grammar, IPA/Unicode rule,
  canonicalization rule, or pinned dependency that affects canonical bytes
  **requires an approved ADR** (steering §0.4, §12).
- ADRs are immutable once `Approved`. To change a decision, add a new ADR that
  `Supersedes` the old one and set the old one to `Superseded`.
- Number ADRs sequentially: `NNNN-short-title.md`.

## Statuses

| Status | Meaning |
|---|---|
| `Proposed` | Under discussion; not yet binding |
| `Approved` | Binding decision |
| `Superseded` | Replaced by a later ADR (link it) |
| `Rejected` | Considered and declined |

## Template

Copy [`0000-template.md`](0000-template.md) to a new numbered file.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Approved |
| [0002](0002-crate-decomposition.md) | Nine-crate workspace decomposition | Approved |
| [0003](0003-open-normative-artifacts.md) | Open normative artifacts are blockers | Approved |
| [0004](0004-integrity-manifest-grammar.md) | `META-INF/integrity.kdl` grammar + canonical serialization | Approved |
| [0005](0005-canonical-epf-serialization.md) | Canonical EPF KDL serializer profile | Approved |
| [0006](0006-resource-and-manifest-policy.md) | Extra-resource / manifest-coverage policy (v0.1.0) | Approved |
| [0007](0007-unicode-engine-icu4x.md) | Unicode 18.0.0 validation via first-party `elang-unicode` | Approved |
| [0008](0008-ipa-repertoire-v1.md) | ELANG IPA Repertoire v1 — source, generation and digest | Approved |
| [0009](0009-residual-eld-canonicalization-edge-cases.md) | Residual ELD canonical serialization edge cases | Approved |
| [0010](0010-kdl-parser-hand-roll.md) | Hand-rolled KDL 2.0 parser for `elang-kdl` | Approved |
| [0011](0011-normative-manifest-and-spec-checksum.md) | Normative artifact manifest and pinned spec checksum (OA-17 mechanism) | Approved |
| [0012](0012-reader-diagnostic-stage-metadata.md) | Reader diagnostic stage metadata | Approved |
