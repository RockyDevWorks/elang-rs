# ELD — Linguistic Definition

`.eld` is the **internal KDL 2.0 extension** ELANG uses for linguistic
inventories. Each linguistic resource a language definition needs — phonemes
in v0.1.0, and other linguistic types (the lexicon, morphology, phonotactics,
syntax, prosody, etc.) in future versions — is carried as a distinct top-level
node within an `.eld` document.

v0.1.0 defines **only the `phonemes` module**. Every other module is reserved
for future versions and is not part of this release. The shape of those future
modules will be defined by later specifications.

Within the v0.1.0 phonemes module, an ELD document describes a language's
sounds as **groups** of **root phonemes**, each with a romanization, optional
**allophones**, and optional conditional realization rules.

## Example

```kdl
phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "ɾ"
            allophone "tʰ" {
                roman "th"
                when {
                    word-position "initial"
                }
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    diphthongs {
        phoneme "aɪ" {
            roman "ai"
        }
    }
}
```

## Top level

An ELD document contains **exactly one** top-level `phonemes` node with an
explicit, nonempty block and zero arguments or properties. Empty, whitespace-only,
or comment-only files are rejected, as are any additional top-level nodes.

## Groups

```kdl
phonemes {
    consonants { /* ... */ }
    vowels     { /* ... */ }
    // optional custom groups:
    diphthongs { /* ... */ }
}
```

- Exactly one `consonants` group and one `vowels` group, each with **≥ 1** root
  phoneme.
- Up to **16 groups total** (so up to 14 custom groups); every custom group must
  be nonempty.
- Group names are unique. Custom group names match the regex
  `[a-z]+(?:-[a-z]+)*` and are 1–32 ASCII characters. `consonants` and `vowels`
  are reserved.
- Group nodes take zero arguments/properties and require a child block whose only
  direct children are `phoneme` nodes.
- Group source order carries no meaning. The canonical order is `consonants`,
  `vowels`, then custom groups in ASCII bytewise order.

## Root phonemes

```kdl
phoneme "<IPA>" {
    roman "<spelling>"
    // optional:
    when { /* ... */ }
    // zero or more:
    allophone "<IPA>"
}
```

- `phoneme` takes exactly **one IPA string argument**, zero properties, and a
  mandatory nonempty block.
- Exactly **one** `roman` child; **optional** zero-or-one `when`; **zero to 16**
  direct `allophone` children.
- At most **256 root phonemes** across all groups.
- Root IPA identifiers are **globally unique** across every group and allophone.
  You cannot "reuse" a sound by placing it in another group.
- Only `roman`, `when`, and `allophone` children are allowed. `frequency`,
  `distribution`, `description`, and any other field are rejected.
- Canonical child order: `roman`, then optional `when`, then allophones sorted by
  IPA. Source declaration order carries no priority.

## Allophones

An allophone is a conditional surface pronunciation of its root.

```kdl
// Fieldless: a bare leaf (no braces).
allophone "ɾ"

// With fields: a nonempty block.
allophone "tʰ" {
    roman "th"
    when { word-position "initial" }
}
```

- `allophone` takes exactly **one IPA string argument** and zero properties, and
  must be a **direct child of a root phoneme** (never nested under another
  allophone).
- Optional zero-or-one `roman`; optional zero-or-one `when`.
- A **fieldless allophone is a bare leaf** — `allophone "ɾ"`. Writing
  `allophone "ɾ" {}` with an empty block is **invalid**. When fields are present,
  the block must be nonempty.
- An omitted `roman` **inherits** the parent root's exact-case romanization.
  Inherited romanizations do not count as explicit duplicates.
- An omitted `when` means unconditional eligibility. An explicit empty `when {}`
  is invalid.

### Independence of conditions

A root's `when` and an allophone's `when` are evaluated **independently** — the
child does not inherit the parent's condition. Multiple pronunciations can be
eligible at once; the generator selects among them with no weighting or
declaration priority. If none is eligible in a given context, the inventory is
still valid — the generator simply avoids that placement or chooses a different
root. ELANG defines no fallback pronunciation.

## Romanization

- `roman` is a scalar leaf: exactly one string argument, no properties, no block.
- An explicit value matches `[A-Za-z]{1,8}` — 1 to 8 ASCII letters, preserving
  case exactly in canonical output.
- Every **explicit** `roman` across all roots and allophones is **globally
  unique using ASCII case-insensitive comparison**. For example, `th` and `TH`
  collide. Inherited romanizations are exempt.

!!! note
    There is no `frequency` field anywhere in ELD v0.1.0. Statistical weighting
    is out of scope for this release.

## IPA identifiers

Every `phoneme` and `allophone` IPA string is validated against the pinned IPA
Repertoire v1 and a strict set of Unicode and structural rules (logical units,
modifiers, tie bars, length). Those rules are detailed in
[Unicode & IPA](unicode-ipa.md).

## Canonical form

The [canonical ELD serializer](canonical.md) fixes group/root/allophone ordering,
child ordering, condition ordering, indentation, line endings, and string
quoting, so that semantically equal inventories always produce identical bytes.
