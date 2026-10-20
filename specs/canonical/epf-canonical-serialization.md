# Canonical EPF KDL serialization profile v0.1.0

> **Task:** T0104 (open artifact OA-7). **Ratified by:**
> [ADR 0005](../../docs/decisions/0005-canonical-epf-serialization.md).
> **Normative source:** steering `.kiro/steering/elang.md` §5, §12.4.
> **Requirements:** REQ-EPF-013, REQ-CANON-001.

The canonical serialization of a validated `package.epf` is byte-deterministic.
A writer MUST emit exactly these bytes; a conformance test compares them to a
golden file.

## Rules

1. **Top-level node order:** `epf`, `eld`, `language-version`, `metadata`,
   `manifest`.
2. **`metadata` children:** `identifier`, `name`, then `description` (only if
   present).
3. **`manifest` children:** `phonemes` (the sole permitted entry).
4. **Indent:** exactly 4 ASCII spaces per nesting level; no tabs.
5. **Layout:** one declaration per line; no blank lines; no comments; LF line
   endings; exactly one trailing LF.
6. **Strings:** standard double-quoted; escape embedded `"` and `\`; preserve
   validated NFC Unicode directly as UTF-8; no silent normalization.
7. No properties and no extra positional arguments are emitted.

## Canonical example

```kdl
epf "0.1.0"
eld "0.1.0"
language-version "1.0.0"
metadata {
    identifier "urn:elang:language:rocky:aelith:1.0.0"
    name "Aelith"
    description "A small constructed language."
}
manifest {
    phonemes "Linguistics/phonemes.eld"
}
```

The byte stream ends with a single LF after the final `}`. When `description`
is absent, its line is omitted entirely (no empty `description ""` is emitted
unless the source actually declared an empty description).
