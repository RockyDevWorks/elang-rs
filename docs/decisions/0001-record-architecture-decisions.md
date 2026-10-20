# ADR 0001: Record architecture decisions

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-GOV-ADR-001
- **Related tasks:** T0006, T0015

## Context

ELANG is a deterministic, byte-exact format. Many of its rules are *locked*: the
binary layout, the EPF/ELD grammars, the IPA/Unicode handling and the
canonicalization algorithm. Changing any of these can silently break
interoperability. We need a durable, reviewable record of why decisions were
made and which ones are binding.

## Decision

We use lightweight Architecture Decision Records stored under
`docs/decisions/`, numbered sequentially, with the statuses `Proposed`,
`Approved`, `Superseded` and `Rejected`. Any change to a locked normative rule
or to a pinned dependency that affects canonical output requires an `Approved`
ADR. ADRs are immutable once approved; a decision is changed by superseding it.

## Consequences

- Every locked-rule change is traceable to a reviewed rationale.
- Reviewers can reject PRs that alter locked behavior without an ADR.
- A small amount of process overhead is added to locked-surface changes; this is
  intentional.

## Alternatives considered

- **Changelog only:** insufficient; it records *what* changed but not the
  binding rationale, and does not gate locked-rule changes.
- **Wiki/external doc:** not versioned with the code and easy to desync.
