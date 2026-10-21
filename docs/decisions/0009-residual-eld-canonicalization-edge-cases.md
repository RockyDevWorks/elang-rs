# ADR 0009: Residual ELD canonical serialization edge cases

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-CANON-002, REQ-CANON-004, REQ-CANON-005, REQ-CANON-006, REQ-CANON-008, REQ-IPA-012, REQ-IPA-013
- **Related tasks:** T0122 (resolves open artifact OA-16)

## Context

The canonical ELD serialization rules are **locked** in steering
`.kiro/steering/elang.md` §9 and the Canonical Condition Sorting Key v1 (§9.1).
ADR 0004 and ADR 0005 ratified the canonical **EPF** and **integrity** profiles;
OA-16 / T0122 asks for the *residual* canonicalization edge cases to be ratified
so the canonical ELD writer (Phase 11, T1121 / OA-14) is unambiguous and its
golden fixtures are reproducible.

This ADR does **not** change any locked §9 rule. It records the only reasonable
reading of each residual point where §9's prose, taken together with the IPA
rules in §7, admits more than one literal implementation. Where §9 is already
unambiguous, nothing new is decided here. If any item below were found to
contradict §9, §9 wins and this ADR must be superseded (steering §0.4).

The residual points all stem from one fact: §7 gives an IPA identifier **two**
byte views — the *authored* NFC bytes (preserved verbatim) and a *comparison
key* that folds the lower tie bar U+035C onto the upper U+0361 for uniqueness.
§9 refers to "NFC IPA identifier UTF-8 bytewise" for ordering and to the
"exact validated value" for condition keys, without restating which view each
means. The tie-bar test vectors (`specs/registry/ipa-tie-bar-vectors.json`,
T0117) already distinguish `original` from `comparison_key`; this ADR aligns the
canonical writer with that distinction.

## Decision

The following are **locked** for the canonical ELD writer. Each is the direct
reading of §9/§7; none introduces a new normative rule.

### E-1 — Ordering uses authored NFC bytes, uniqueness uses the comparison key

Group-internal **root** ordering and root-internal **allophone** ordering
(steering §9, REQ-CANON-002) sort by the identifier's **authored NFC UTF-8
bytes** — the same bytes preserved in output (`original` in the tie-bar
vectors), **not** the tie-bar-folded comparison key.

- Rationale: §9 says "sorted by NFC IPA identifier UTF-8 bytewise" and also
  says the authored tie-bar variant is preserved in output (§7, REQ-IPA-013).
  Sorting by the preserved bytes keeps order and output consistent.
- No collision is possible: two identifiers that differ **only** by tie-bar
  variant share a comparison key and are therefore rejected as non-unique
  before serialization (§7, REQ-IPA-012, constraint C-09). So the authored-byte
  sort is always total over the surviving (unique) identifiers.
- The comparison key is used **only** for the uniqueness check, never for
  ordering or emission.

### E-2 — Condition Sorting Key v1 value bytes are the authored NFC bytes

In the Canonical Condition Sorting Key v1 (§9.1), the `atomic_key` field
`exact_validated_value_utf8` for `previous-phoneme` / `next-phoneme` is the
**authored NFC bytes** of the referenced identifier as written in the source,
consistent with E-1 and with §9.1's wording "exact validated value". The writer
does **not** substitute the owning root's identifier and does **not** fold the
tie bar when building the key.

- `word-position` / `syllable-position` values are their fixed ASCII enum
  literals, used verbatim.
- This preserves §9's rule that canonical output keeps "authored IPA refs" and
  does not rewrite references to owning roots.

### E-3 — String escaping is exactly `"` and `\`, and nothing else

Canonical ELD strings are standard double-quoted. The writer escapes **only**
the double quote (`\"`) and the backslash (`\\`). It emits **no** other escape
sequence — no `\n`, `\t`, `\r`, `\/`, `\b`, `\f`, nor any `\u{...}` form.

- This is sound because every ELD string value is validated NFC and free of
  controls (`Cc`), `Cf`, `Co`, `Cn` and all noncharacters (§7, REQ-UNICODE-004).
  No character that would otherwise need escaping can appear, so §9's "escape
  embedded `"` and `\`" is exhaustive for valid input.
- All other permitted Unicode scalar values are emitted **directly** as UTF-8,
  never as an escape. No silent normalization (§9, REQ-CANON-008).

### E-4 — Bare vs block emission, re-stated for the writer

The writer emits (restating §6/§9 so the two are read together):

- A **fieldless allophone** as a bare leaf `allophone "x"` on its own line,
  **never** `allophone "x" {}` (REQ-CANON-004, C-10).
- An allophone **with** a `roman` and/or `when` as `allophone "x" {` … `}` with
  a nonempty block; child order `roman` (if present) then `when` (if present).
- A **root phoneme** always has a block (it always has exactly one `roman`);
  child order `roman`, then `when` (if present), then allophones in E-1 order.
- An opening brace sits on the node's own line after a single space
  (`name "arg" {`); the matching `}` is on its own line at the parent's indent.
  One declaration per line (§9); the brace lines are not "declarations" and do
  not violate the one-per-line rule.

### E-5 — `roman` value is emitted verbatim with case preserved

The `roman` scalar is emitted as `roman "<value>"` with the authored ASCII case
preserved exactly (§9, REQ-CANON-006). A `roman` value is always `[A-Za-z]{1,8}`
(C-14), so E-3 escaping never applies to it. Inherited (omitted) allophone
`roman` is **not** emitted — inheritance is implicit, so a bare/`when`-only
allophone carries no `roman` line.

### E-6 — Layout invariants (no residual ambiguity, pinned for completeness)

Exactly as §9: **4 ASCII spaces** per nesting level (no tabs); LF line endings;
no blank lines; no comments; **exactly one** final LF. A valid ELD always has at
least `phonemes { consonants {…} vowels {…} }`, so the "empty document" case
cannot arise and needs no special handling; an empty/whitespace/comment-only
input is rejected at parse (C-01/C-02), never canonicalized.

## Consequences

- The canonical ELD writer (T1121) and its golden fixtures (OA-14) now have an
  unambiguous spec for every byte they emit; OA-16 is ratified.
- E-1/E-2 make ordering and condition keys depend only on the **preserved**
  bytes, so canonical output is self-consistent: re-reading canonical output and
  re-canonicalizing is a fixpoint (REQ-CANON-009 idempotence).
- These points are now **locked**; changing any of them requires a superseding
  ADR (steering §0.4).
- Tests to add in Phase 11/14: an ordering vector where two roots differ only in
  a tied vs untied unit (distinct keys, authored-byte order), a condition-key
  vector for `previous-phoneme` with a lower tie bar (authored bytes in the key),
  and a round-trip/idempotence check. These complement the existing tie-bar
  vectors (T0117).

## Alternatives considered

- **Order roots/allophones by the tie-bar-folded comparison key (E-1):**
  rejected — it would diverge from the preserved output bytes and from §9's
  literal "NFC IPA identifier" wording, and it buys nothing because
  tie-only-variant collisions are already rejected by uniqueness.
- **Resolve `previous-phoneme`/`next-phoneme` to the owning root in the sort key
  (E-2):** rejected — §9 requires canonical output to preserve authored
  references; folding them into roots would rewrite authored data.
- **Emit `\u{...}` escapes for non-ASCII IPA (E-3):** rejected — valid ELD
  strings need no escaping beyond `"`/`\`, and direct UTF-8 is what §9 mandates
  and what keeps bytes reproducible.