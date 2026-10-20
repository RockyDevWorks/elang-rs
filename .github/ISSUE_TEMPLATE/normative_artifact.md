---
name: Normative / open artifact
about: Track an open normative artifact that must be produced and reviewed
title: "[artifact] "
labels: ["normative", "blocker"]
---

## Artifact

Which open artifact does this cover? (steering §12)

- [ ] Exact approved IPA allowlist (base / combining / spacing)
- [ ] Canonical IPA JSON SHA-256 digest
- [ ] `META-INF/integrity.kdl` normative grammar + canonical serialization
- [ ] Canonical EPF / integrity KDL serializer profiles
- [ ] Machine-checkable ELD/EPF grammars + JSON Schema
- [ ] Reference golden fixtures / reproducibility hashes
- [ ] Error-code registry
- [ ] Other:

## Why it blocks

Which phases/tasks are blocked until this is frozen? (e.g. Phase 7 integrity,
Phase 8 IPA, conformance freeze.)

## Required deliverables

- [ ] The artifact file(s), committed under `specs/`.
- [ ] Independent review against the pinned source (IPA 2026 chart / Unicode
      18.0.0) where applicable.
- [ ] Reproducible generation procedure documented.
- [ ] Test vectors.
- [ ] ADR recording the decision.

## Acceptance

Artifact exists, is reviewed, is reproducible, and has independent test vectors.
No placeholder inventories or fabricated hashes.
