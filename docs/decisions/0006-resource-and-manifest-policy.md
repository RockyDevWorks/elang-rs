# ADR 0006: Extra indexed resources and manifest coverage in v0.1.0

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-PKG-005
- **Related tasks:** T0121 (resolves open artifact OA-15)

## Context

Steering §2 lists the required logical resources and says other resources "may
exist only if allowed by finalized resource/manifest policy" and that we must
"not invent module semantics." §12 forbids inventing modules. OA-15 asks for
the exact v0.1.0 policy on extra indexed resources and manifest coverage.

## Decision

For v0.1.0 (phonemes-only):

1. **Required resources** (exactly these logical roles must be present):
   - `Header/format.version`
   - `mimetype`
   - `package.epf`
   - `META-INF/integrity.kdl`
   - exactly one `.eld` phoneme resource at the path declared by
     `manifest { phonemes "..." }`.
2. **No invented modules.** EPF `manifest` has exactly one `phonemes` entry and
   no `modules` node (locked, steering §5). No other manifest entry type exists
   in v0.1.0.
3. **Extra indexed resources are rejected in v0.1.0.** The archive index MUST
   contain only the five required resources above. Any additional indexed
   resource causes validation failure. This keeps the format closed and
   unambiguous until a future module defines additional roles.
4. **Integrity coverage** (per ADR 0004) covers every indexed resource except
   `integrity.kdl` itself. With rule 3, that is exactly the four non-manifest
   resources.
5. The generic "undeclared resource default max 1 GiB" ceiling in steering §3.5
   remains defined for defensive decoding, but since rule 3 rejects undeclared
   resources, it functions only as a decode-time guard, not an acceptance path.

## Consequences

- The v0.1.0 package shape is fully enumerated; `validate` success has one
  possible resource set.
- A future version that adds modules must supersede this ADR and define the new
  manifest entries and resource roles.
- Prevents silently accepting unknown payloads that integrity would still hash
  but that carry no defined meaning.

## Alternatives considered

- **Allow arbitrary extra resources as long as integrity covers them:**
  rejected for v0.1.0 — accepting payloads with no defined role invites
  ambiguity and would need module semantics that are explicitly out of scope.
