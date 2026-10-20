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
