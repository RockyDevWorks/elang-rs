# ADR 0004: `META-INF/integrity.kdl` grammar and canonical serialization

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-INTEGRITY-001..008, REQ-PKG-003
- **Related tasks:** T0103, T0105 (resolves open artifact OA-6)

## Context

Steering §2 and §12.3 require the exact `META-INF/integrity.kdl` grammar, its
canonical serialization, and the self-exclusion rule to be **decided and
published** before the integrity reader/writer is implemented (Phase 7,
REQ-INTEGRITY-001 BLOCKER). Steering states the manifest "hashes the
uncompressed bytes of every other indexed resource, excluding itself" using
SHA-256, but leaves the concrete node shape and whether it records per-resource
sizes/algorithms open (OA-6). This ADR fixes the minimal grammar consistent with
those locked facts. It introduces no new hashing semantics; it only names the
container for them.

## Decision

### Grammar

`META-INF/integrity.kdl` is KDL 2.0 (strict UTF-8, NFC), ≤ **16 MiB**
uncompressed, with **exactly one** top-level `integrity` node:

```kdl
integrity {
    algorithm "sha-256"
    resource "Header/format.version" sha256="<64 lowercase hex>"
    resource "Linguistics/phonemes.eld" sha256="<64 lowercase hex>"
    resource "mimetype" sha256="<64 lowercase hex>"
    resource "package.epf" sha256="<64 lowercase hex>"
}
```

Rules (all MUST; violations are rejected, never repaired):

1. One top-level `integrity` node: zero args, zero properties, mandatory
   nonempty block. No other top-level node.
2. `algorithm`: exactly once, one string arg equal to the ASCII literal
   `"sha-256"`, no properties, no block. (Only SHA-256 is defined in v0.1.0;
   any other value is rejected.)
3. `resource`: one per covered resource. Exactly one string arg — a resource
   path validated by the steering §3.4 rules — and exactly one property
   `sha256` whose value is **64 lowercase hex characters** (`[0-9a-f]{64}`).
   No child block. No other property or argument.
4. **Coverage:** there is exactly one `resource` entry for **every** indexed
   resource **except `META-INF/integrity.kdl` itself** (self-exclusion). A
   `resource` entry for a path not in the index, a missing entry for an indexed
   resource, or an entry for `META-INF/integrity.kdl` is rejected.
5. No duplicate `resource` paths; no duplicate `algorithm`; no unknown
   top-level nodes, child nodes, properties, or extra arguments anywhere.
6. The declared `sha256` is compared against the SHA-256 of the resource's
   **exact uncompressed bytes** (stage 11). The footer index hash
   (steering §3.3) is a **separate** check and is not an `integrity` entry.

### Canonical serialization

When ELANG writes `integrity.kdl` (pack), it emits canonical bytes:

- Top-level `integrity` node; children emitted in this order: `algorithm`
  first, then all `resource` entries.
- `resource` entries sorted by their path argument using **raw UTF-8 bytewise
  ascending** order (same ordering as the container index, steering §3.2).
- One declaration per line; **4 ASCII spaces** per nesting level; no tabs;
  **no blank lines**; **no comments**; **LF** line endings; **exactly one**
  final LF. (Mirrors the locked canonical-ELD whitespace rules, §9, applied
  here by explicit decision.)
- Strings are standard double-quoted; escape embedded `"` and `\`. The `sha256`
  property value is lowercase hex, never uppercase.
- The writer computes each digest over the final uncompressed resource bytes,
  excluding `integrity.kdl` itself, so the manifest never hashes itself.

## Consequences

- Unblocks Phase 7 (integrity parse/verify/write) and the pack path.
- This grammar and serialization are now **locked**; changing them requires a
  superseding ADR (steering §0.4).
- Deliberately minimal: no per-resource size field and no algorithm agility
  beyond the single fixed `sha-256`, matching the v0.1.0 scope. A future
  version wanting multi-algorithm support must supersede this ADR.
- Published as [`specs/grammar/integrity-0.1.0.md`](../../specs/grammar/integrity-0.1.0.md).

## Alternatives considered

- **Record per-resource `size`/`algorithm` on each entry:** rejected for
  v0.1.0 — size is already authoritative in the index, and a single pinned
  algorithm keeps verification unambiguous. Revisit via a new ADR if agility is
  needed.
- **Positional hash arg instead of a `sha256` property:** rejected — a named
  property is self-describing and leaves room for a future algorithm property
  without reordering positional args.
