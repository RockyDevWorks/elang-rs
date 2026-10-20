# Locked rules policy

Implements REQ-GOV-ADR-001 / T0015: **no unreviewed changes to locked binary or
schema semantics.** Governed by
[ADR 0001](decisions/0001-record-architecture-decisions.md).

## What is locked

Changing any of the following requires an **approved ADR** and regenerated
golden hashes where applicable. A PR that touches these without an ADR must be
rejected in review.

### Binary container
- 32-byte header layout, magic `45 4C 41 4E 47 0D 0A 1A`, version/flag/reserved rules.
- Index header (`ELIDX001`), entry layout and ordering rules.
- 64-byte footer (`ELEND001`), offset-consistency rule, index SHA-256 location.
- Resource-path rules and size ceilings.
- The one-frame Zstd contract and the `elang-canonical-zstd-v1` settings.

### Schemas
- EPF five-node grammar, URN format, field cardinalities.
- ELD group/root/allophone grammar, cardinalities, bare-allophone rule.
- `when` grammar (`group`/`or` arity, atomic predicates) and its semantics.

### IPA / Unicode
- Pinned Unicode 18.0.0 and IPA 2026 chart.
- Identifier length and logical-unit limits; tie-bar and modifier rules;
  tie-bar equivalence for uniqueness.
- Rejected categories (`Cc`/`Cf`/`Co`/`Cn`/noncharacters) and non-NFC rejection.

### Canonicalization
- Canonical ELD ordering, indentation, line endings, string quoting.
- Canonical Condition Sorting Key v1 binary format and rank assignments.

### Pinned dependencies affecting bytes
- libzstd 1.5.7; the Unicode data tables; any serializer-affecting dependency.

## What is not yet locked

The open artifacts in [`open-artifacts.md`](open-artifacts.md). These are
*unfinished*, not *changeable at will*: they must be produced and reviewed per
ADR 0003 before they become locked.

## Enforcement

- The PR template requires linking an ADR for any locked-rule change.
- CI treats changes to canonical golden hashes as review-significant.
- Reviewers verify the ADR exists and is `Approved` before merging.
