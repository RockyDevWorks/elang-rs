# ADR 0003: Open normative artifacts are release blockers

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-GOV-OPENART-001
- **Related tasks:** T0016, T0101–T0123

## Context

The steering contract (§3, §5, §7, §12) enumerates normative artifacts that are
**not yet finalized**: the exact approved IPA allowlist and its canonical
SHA-256, the `META-INF/integrity.kdl` grammar and its canonical serialization,
the canonical EPF serializer profile, and the machine-checkable grammars/JSON
Schemas. The contract forbids fabricating these and calling them ratified.

## Decision

Each open artifact is tracked as a `blocker`-labelled issue and listed in the
requirements document under REQ-GOV-OPENART-001. No code path may embed a
guessed IPA inventory, a placeholder/fake hash, or an inferred integrity grammar
as if it were normative. Phases that depend on an open artifact (notably Phase 7
integrity, Phase 8 IPA, and the conformance freeze) are **blocked** until the
corresponding artifact exists, is independently reviewed against its pinned
source, is reproducible and has test vectors. Freezing an artifact requires an
ADR recording the final bytes/rules.

## Consequences

- Phases 2–6 and the container/KDL layers can proceed, since they do not depend
  on the open artifacts' *content*.
- The IPA validator, integrity reader/writer, canonical IPA JSON digest and the
  conformance corpus cannot be finalized until their artifacts are ratified.
- Prevents shipping a deterministic format on top of non-deterministic or
  invented normative data.

## Alternatives considered

- **Placeholder data now, real data later:** rejected — a placeholder digest or
  inventory can leak into fixtures and release artifacts and create false
  conformance signals.
