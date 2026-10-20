# `META-INF/integrity.kdl` v0.1.0 — grammar and canonical serialization

> **Tasks:** T0103, T0105 (open artifact OA-6). **Ratified by:**
> [ADR 0004](../../docs/decisions/0004-integrity-manifest-grammar.md).
> **Normative source:** steering `.kiro/steering/elang.md` §2, §3.3, §12.3.
> **Requirements:** REQ-INTEGRITY-001..008.
> Companion ABNF: [`integrity-0.1.0.abnf`](integrity-0.1.0.abnf).

## 1. Base format

- KDL **2.0**, strict UTF-8, enforced NFC; ≤ **16 MiB** uncompressed.
- Generic KDL budgets (steering §10) apply before any schema check.

## 2. Shape

```kdl
integrity {
    algorithm "sha-256"
    resource "Header/format.version" sha256="<64 lowercase hex>"
    resource "Linguistics/phonemes.eld" sha256="<64 lowercase hex>"
    resource "mimetype" sha256="<64 lowercase hex>"
    resource "package.epf" sha256="<64 lowercase hex>"
}
```

## 3. Constraints (`[C-nn]`, all normative)

| ID | Constraint | Reject when violated |
|---|---|---|
| C-01 | Exactly one top-level `integrity` node: zero args, zero properties, mandatory nonempty block; nothing else at top level. | Extra top-level node; args/props; empty/missing block. |
| C-02 | `algorithm`: exactly once, one string arg `"sha-256"`, no properties, no block. | Other value; duplicate; property; block. |
| C-03 | `resource`: one string path arg (steering §3.4 path rules) + exactly one property `sha256` matching `[0-9a-f]{64}`; no block; no other arg/property. | Bad path; uppercase/short/long hex; missing/extra property; block. |
| C-04 | Coverage is exact: one `resource` per indexed resource **except** `META-INF/integrity.kdl` (self-exclusion). | Entry for a non-indexed path; missing entry for an indexed resource; entry for `integrity.kdl`. |
| C-05 | No duplicate `resource` paths; no duplicate `algorithm`; no unknown nodes/properties/args. | Any duplicate or unknown field. |
| C-06 | Each `sha256` compared to SHA-256 over the resource's **exact uncompressed bytes** (reader stage 11). The footer index hash (§3.3) is verified separately and is not an `integrity` entry. | Digest mismatch (per-resource failure). |

## 4. Canonical serialization (writer)

- Children order: `algorithm`, then `resource` entries.
- `resource` entries sorted by path argument, **raw UTF-8 bytewise ascending**
  (matches container index order).
- 4 ASCII spaces per level; one declaration per line; no tabs; no blank lines;
  no comments; LF endings; exactly one final LF.
- Standard double-quoted strings; escape `"` and `\`; `sha256` value lowercase.
- Writer excludes `integrity.kdl` from its own coverage (C-04).

## 5. Non-goal

A matching `sha256` proves **integrity**, not publisher authenticity
(steering §0.8, REQ-INTEGRITY-008). This manifest is not a signature.
