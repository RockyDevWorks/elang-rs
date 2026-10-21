# The `when` condition language

A root phoneme or allophone may carry an optional `when` block describing the
phonetic context in which it is eligible. The language is small and explicit: a
few atomic predicates combined with implicit AND and a binary OR.

## Shape

A `when` block must have explicit braces, at least one child expression, and zero
arguments or properties. A root and an allophone each have at most one `when`,
and the two are evaluated **independently** — there is no inheritance between
them.

```kdl
when {
    or {
        group {
            word-position "final"
            syllable-position "coda"
        }
        previous-phoneme "s"
    }
}
```

## Atomic predicates

Each predicate takes exactly one string argument, with no block and no
properties.

| Predicate | Allowed values |
|-----------|----------------|
| `word-position` | `initial`, `medial`, `final` |
| `syllable-position` | `onset`, `nucleus`, `coda` |
| `previous-phoneme` | a declared root or allophone IPA identifier |
| `next-phoneme` | a declared root or allophone IPA identifier |

## Combining predicates

- **`when { ... }`** and **`group { ... }`** implicitly **AND** their direct
  children.
- **`or { a b }`** takes **exactly two** direct expressions and means **OR**.
  Nested `or` is allowed, including cross-category alternatives.
- `group` requires **≥ 2** direct expressions.
- There is **no explicit `and` or `not`** node. Unknown nodes are rejected.
- Repeated predicates are allowed — even contradictory ones. The structure is
  preserved; nothing is simplified.

### Limits

- At most **16 atomic predicates** per `when`, across all branches.
- At most **8 nested `group`/`or` levels** along any path (the root `when` and
  the leaves are not counted).
- The generic KDL parser independently caps nesting depth at 64.

## Semantics

### Word position

Word position is defined over the sequence of underlying **root** phonemes in a
word:

- `initial` — the first root phoneme.
- `final` — the last root phoneme.
- `medial` — neither first nor last.

A single-root-phoneme word is **both initial and final**, never medial.

### Syllable position

`onset`, `nucleus`, and `coda` refer to the **generator-assigned** syllable
components, not something inferred from the consonant/vowel group. Every phoneme
inside a multi-phoneme component matches that component's position.

!!! note "The syllabifier is not part of v0.1.0"
    `when` predicates may refer to generator-assigned syllable positions, but
    ELANG v0.1.0 does not define the syllabifier or the generator itself.

### Neighbors

`previous-phoneme` and `next-phoneme` refer to the immediately adjacent
underlying **root identity**. They look across syllable boundaries but **not
across word boundaries**:

- A missing neighbor (for example, no previous phoneme at the start of a word)
  makes the predicate **false** — this is not a validation error.
- A reference to an allophone resolves to its owning root.
- Forward and self references are allowed, resolved after the whole inventory is
  collected. There is no circularity, because references are to underlying roots,
  not surface pronunciations.

### Eligibility, not probability

The evaluator is a pure function over a supplied context (word position,
generator-assigned syllable roles, neighbors). It returns an **eligibility
set** — never a probability, weight, or priority. If no pronunciation is
eligible in a context, the set is empty and there is **no fallback**; the
inventory is still valid.

## Canonical ordering

Inside `when`, `group`, and `or`, the direct child expressions are sorted
recursively using the **Canonical Condition Sorting Key v1**. Tree structure and
duplicate predicates are preserved; there is no boolean simplification,
flattening, or reference rewriting. See [Canonical serialization](canonical.md).
