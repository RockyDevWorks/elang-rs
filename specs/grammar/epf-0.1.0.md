# EPF `package.epf` v0.1.0 — machine-checkable grammar and constraints

> **Task:** T0101. **Normative source:** steering `.kiro/steering/elang.md` §5
> (LOCKED). Companion ABNF: [`epf-0.1.0.abnf`](epf-0.1.0.abnf).
> **Requirements:** REQ-EPF-001..014.
>
> This document is a derivation of already-locked rules, not a new decision.
> It exists so a validator and the conformance suite can check EPF documents
> against an exact, enumerated contract. Where ABNF cannot express a count,
> uniqueness or cross-field equality, the `[C-nn]` constraints below are
> normative and MUST be enforced during parse/validation.

## 1. Base format

- Encoding: KDL **2.0**, strict UTF-8, enforced NFC for all string values.
- Resource budget: `package.epf` ≤ **1 MiB** uncompressed (steering §2).
- All generic KDL parser budgets (steering §10) apply before any schema check.

## 2. Top-level structure

Exactly **five** top-level nodes, each present **exactly once**, in any source
order. The canonical order (for serialization, T0104) is:

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

## 3. Constraints (`[C-nn]`, all normative)

| ID | Constraint | Reject when violated |
|---|---|---|
| C-01 | Top level is exactly the set `{epf, eld, language-version, metadata, manifest}`, each once. | Unknown top-level node; duplicate top-level node; missing any of the five; any top-level argument or property. |
| C-02 | `epf` value == `"0.1.0"`; `eld` value == `"0.1.0"`. | Any other version string. |
| C-03 | `epf`, `eld`, `language-version` each have exactly one string argument and **no** child block. | Missing/extra argument; non-string argument; any property; a child block. |
| C-04 | `metadata` and `manifest` each have **zero** arguments, **zero** properties and a **mandatory nonempty** child block. | Any argument/property; missing block; empty block. |
| C-05 | `language-version` is strict `MAJOR.MINOR.PATCH`, decimal, **no leading zeros** (except a lone `0` component). | Leading zeros; missing components; extra components; non-numeric; prerelease/build suffixes. |
| C-06 | Inside `metadata`: `identifier` exactly once, `name` exactly once, `description` at most once; no other child nodes; no properties on `metadata`. | Unknown child; duplicate `identifier`/`name`; >1 `description`; nested blocks on these children. |
| C-07 | `metadata.identifier`: nonempty, ≤ **512** UTF-8 bytes, exactly `urn:elang:language:<namespace>:<language-name>:<version>`. `<namespace>` and `<language-name>` are ASCII lowercase slugs matching `[a-z0-9]+(?:-[a-z0-9]+)*`, beginning and ending alphanumeric. `<version>` is strict `MAJOR.MINOR.PATCH` (see C-05). | Wrong scheme/NID; uppercase; leading/trailing hyphen; empty segment; wrong field count; > 512 bytes. |
| C-08 | `metadata.name`: exactly one string arg, nonempty, ≤ **256** UTF-8 bytes, no block. | Empty; > 256 bytes; block present. |
| C-09 | `metadata.description`: exactly one string arg, ≤ **4096** UTF-8 bytes, **may be empty**, no block. | > 4096 bytes; block present; non-string arg. |
| C-10 | `language-version` value **equals** the `<version>` suffix of `metadata.identifier` exactly. | Any mismatch. |
| C-11 | Inside `manifest`: exactly one `phonemes` child with one nonempty string argument and no block; **no** `modules` node; **no** other child node; no properties on `manifest`. | Missing/duplicate `phonemes`; `modules` present; any other child; extra args; block on `phonemes`. |
| C-12 | `manifest.phonemes` value is a safe package-relative resource path (steering §3.4), ends with `.eld`, and **exists** in the archive index. | Absolute/`..`/backslash/NUL/empty-segment path; not ending `.eld`; path not indexed. |

## 4. Validation ordering note

Structural constraints (C-01..C-11) are checked during EPF parse (stage 10 of
the reader). C-12's "path exists in index" cross-check uses the already-parsed
index from stage 5; it is a semantic link, not a syntactic property of the EPF
text. No EPF declaration may ever be silently ignored (steering §0.5).
