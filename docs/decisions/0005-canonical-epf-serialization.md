# ADR 0005: Canonical EPF KDL serializer profile

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-EPF-013, REQ-CANON-001
- **Related tasks:** T0104 (resolves open artifact OA-7)

## Context

Steering §5 and §12.4 require the canonical EPF whitespace/escaping/field-order
profile to be **decided and published** as an explicit artifact, and warns that
the locked canonical-ELD rules (§9) do **not** automatically apply to EPF
without a documented decision. This ADR makes that decision. It only governs
the bytes the writer emits; EPF *parsing* already accepts arbitrary KDL 2.0
whitespace/comments (steering §5).

## Decision

The canonical serialization of `package.epf` is:

1. **Node order** (top level): `epf`, `eld`, `language-version`, `metadata`,
   `manifest` (the locked order in steering §5).
2. **`metadata` child order:** `identifier`, `name`, then `description` if
   present.
3. **`manifest` child order:** `phonemes` (the only permitted entry).
4. **Indentation:** exactly **4 ASCII spaces** per nesting level; no tabs.
5. **Layout:** one declaration per line; **no blank lines**; **no comments**;
   **LF** line endings; **exactly one** final LF.
6. **Strings:** standard double-quoted; escape embedded `"` and `\`; preserve
   permitted Unicode directly as UTF-8 (values are already validated NFC and
   free of controls/Cf/Co/Cn); never silently normalize.
7. **No properties and no extra arguments** are emitted (EPF has none).

This is intentionally the same whitespace/line/string discipline as canonical
ELD (§9), chosen by explicit decision here — not inherited implicitly. The field
order follows the locked EPF grammar rather than any sort.

## Consequences

- Canonical EPF output is byte-deterministic and reproducible, enabling golden
  fixtures for the pack path.
- Locked: changing this profile requires a superseding ADR.
- Published as
  [`specs/canonical/epf-canonical-serialization.md`](../../specs/canonical/epf-canonical-serialization.md).

## Alternatives considered

- **Alphabetical child sort inside `metadata`:** rejected — the grammar's
  logical order (`identifier`, `name`, `description`) is clearer and the field
  set is fixed and tiny, so a sort adds nothing.
- **Allowing comments / original formatting passthrough:** rejected — canonical
  output must be reproducible and independent of source formatting.
