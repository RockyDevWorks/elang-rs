# ELD phoneme inventory v0.1.0 — machine-checkable grammar and constraints

> **Task:** T0102. **Normative source:** steering `.kiro/steering/elang.md`
> §6 (structure), §8 (`when` language), §9 (canonical serialization) — all
> LOCKED. Companion ABNF: [`eld-0.1.0.abnf`](eld-0.1.0.abnf).
> **Requirements:** REQ-ELD-001..017, REQ-WHEN-001..015.
>
> Derivation of locked rules into an enumerated, testable contract. `[C-nn]`
> constraints are normative. IPA identifier validity additionally depends on
> the approved repertoire (T0106–T0110), which is an **open artifact**
> ([ADR 0003](../../docs/decisions/0003-open-normative-artifacts.md)); a
> validator MUST NOT accept IPA values until that allowlist is ratified.

## 1. Base format

- Encoding: KDL **2.0**, strict UTF-8, enforced NFC for **every** string value.
- Resource budget: one `.eld` ≤ **1 GiB** uncompressed (steering §2, §3.5).
- Generic KDL budgets (steering §10) apply before any schema check.

## 2. Structural constraints (`[C-nn]`, all normative)

### Document and groups

| ID | Constraint | Reject when violated |
|---|---|---|
| C-01 | Exactly one top-level `phonemes` node with an explicit nonempty block, zero args, zero properties; nothing else at top level. | Extra top-level node; empty/whitespace/comment-only file; args/props on `phonemes`; empty `phonemes` block. |
| C-02 | Exactly one `consonants` group and one `vowels` group, each with **≥1** root phoneme. | Missing/duplicate `consonants` or `vowels`; empty mandatory group. |
| C-03 | At most **16** groups total (so ≤ 14 custom groups); each custom group **nonempty**. | 17th group; empty custom group. |
| C-04 | Custom group name matches `[a-z]+(?:-[a-z]+)*`, length **1–32** ASCII chars; names unique; `consonants`/`vowels` reserved. | Uppercase/digits/underscore; length 0 or >32; duplicate name; reusing a reserved name. |
| C-05 | Group nodes have zero args, zero properties, a **mandatory** child block, and only direct `phoneme` children. | Args/props on a group; missing block; any non-`phoneme` child. |

### Root phonemes

| ID | Constraint | Reject when violated |
|---|---|---|
| C-06 | `phoneme "<IPA>"`: exactly one IPA string arg, **zero** properties, mandatory **nonempty** block. | Missing/extra arg; non-string arg; any property; missing/empty block. |
| C-07 | Root children: exactly **one** `roman`; **zero or one** `when`; **0–16** `allophone`. Only these children. | 0 or ≥2 `roman`; ≥2 `when`; 17th `allophone`; unknown child (`frequency`, `distribution`, `description`, etc.); duplicate singleton. |
| C-08 | Total root phonemes across all groups ≤ **256**. | 257th root. |
| C-09 | Root IPA identifiers are globally unique across all groups **and** allophones, including tie-bar-equivalent forms (steering §7). | Any collision, incl. upper/lower tie-bar equivalents. |

### Allophones

| ID | Constraint | Reject when violated |
|---|---|---|
| C-10 | `allophone "<IPA>"`: exactly one IPA string arg, zero properties; must be a **direct** child of a root (never nested under an allophone). If no fields present, use a **bare** leaf `allophone "x"`. | Missing/extra arg; property; nesting under an allophone; `allophone "x" {}` (explicit empty block, **invalid**). |
| C-11 | If an allophone has any field, it has a **nonempty** block containing at most one `roman` and at most one `when`; no other children. | Empty block with fields expected; ≥2 `roman`; ≥2 `when`; unknown child. |
| C-12 | ≤ 16 direct allophones per root (same count as C-07). | 17th allophone. |

### Romanization

| ID | Constraint | Reject when violated |
|---|---|---|
| C-13 | `roman`: scalar leaf, exactly one string arg, no properties, no block. | Missing/extra arg; non-string arg; property; any block. |
| C-14 | Explicit `roman` value matches `[A-Za-z]{1,8}`, case preserved in canonical output. | Non-ASCII-letter; empty; >8 chars. |
| C-15 | Every **explicit** `roman` is globally unique under ASCII case-insensitive comparison (`th` collides with `TH`). Inherited romanizations are exempt. | Any case-insensitive duplicate among explicit values. |
| C-16 | An allophone that omits `roman` **inherits** the parent root's exact original-case value; inheritance does not count as an explicit duplicate for C-15. | — (defines behavior; a validator that drops inheritance is wrong). |

## 3. `when` condition language (steering §8)

| ID | Constraint | Reject when violated |
|---|---|---|
| C-20 | A root/allophone has **0 or 1** `when`; if present it has explicit braces, **≥1** child expression, zero args/properties. Root and allophone `when` are evaluated independently (no inheritance). | ≥2 `when`; empty `when {}`; args/props on `when`. |
| C-21 | Atomic predicates (`word-position`, `syllable-position`, `previous-phoneme`, `next-phoneme`): exactly one string arg, zero properties, **no** child block. | Missing/extra arg; property; child block; unknown predicate name. |
| C-22 | `word-position` ∈ {`initial`, `medial`, `final`}; `syllable-position` ∈ {`onset`, `nucleus`, `coda`}. | Any other value. |
| C-23 | `previous-phoneme`/`next-phoneme` values are declared root/allophone IPA identifiers (resolved to the owning root). | Reference to an undeclared identifier. |
| C-24 | `group`: mandatory braces, zero args/properties, **≥2** direct child expressions (implicit AND). | <2 children; args/props. |
| C-25 | `or`: mandatory braces, zero args/properties, **exactly 2** direct child expressions (OR). Nested `or` permitted. | ≠2 children; args/props. |
| C-26 | No explicit `and` or `not` node; unknown nodes invalid. Repeated/contradictory predicates allowed. | `and`/`not`/unknown node. |
| C-27 | ≤ **16** atomic predicates per `when` across all nested branches; ≤ **8** nested `group`/`or` levels along any path (root `when` and leaf atomics excluded). Generic KDL depth ≤ 64 still applies. | 17th predicate; 9th nesting level. |

## 4. Semantics (evaluator, steering §8) — reference, not syntax

These govern the pure evaluator (Phase 10), stated here so the grammar's
references are unambiguous:

- `initial`/`final`/`medial` are over the **underlying root** sequence of a
  word. A one-root word is **both** initial and final, never medial.
- Syllable positions use the **generator-assigned** onset/nucleus/coda roles;
  all phonemes inside a multi-phoneme component match that component.
- `previous-phoneme`/`next-phoneme` match the immediately adjacent underlying
  **root identity**, across syllable boundaries but **not** word boundaries; a
  missing neighbor yields **false**, not an error. Allophone references resolve
  to the parent root. Forward/self references allowed after global resolution.
- Eligibility is a **set**; there is no probability, weight, priority, or
  fallback. No eligible realization ⇒ explicit empty set.

## 5. Canonical serialization

Locked in steering §9 and the Canonical Condition Sorting Key v1 (§9.1). Not
restated here; the canonical writer (Phase 11) and its golden fixtures
(T1121 / OA-14) are the executable form.
