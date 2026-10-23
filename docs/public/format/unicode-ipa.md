# Unicode & IPA

ELANG identifies sounds with IPA strings, and it is strict about exactly which
characters are allowed and how they combine. Behavior is pinned so it does not
drift with the host environment.

## Pinned sources

- **Unicode:** pinned to **18.0.0** for all NFC and General-Category behavior. The
  reference implementation generates its tables from the official UCD 18.0.0 and
  does not use host-OS Unicode behavior.
- **IPA chart:** pinned to the IPA **2026 chart issue**.
- **IPA Repertoire v1:** a narrow, explicit **allowlist** of approved base
  symbols, combining diacritics, and spacing modifiers. Anything outside the
  allowlist is rejected.

The repertoire is distributed as strict UTF-8 JSON (no BOM), canonicalized with
RFC 8785 JCS, and verified against a pinned SHA-256 digest shipped with the
validator.

## Rejected characters

Every ELD string must be **NFC**; non-NFC input is rejected rather than
normalized. In addition, the following are rejected in every ELD string:

- Unicode control characters (`Cc`) and format characters (`Cf`),
- private-use characters (`Co`),
- unassigned code points (`Cn`),
- all Unicode noncharacters (including `U+FDD0`–`U+FDEF` and the plane-end
  `FFFE`/`FFFF` code points),
- surrogates and any invalid UTF-8.

Standalone stress marks, syllable-boundary markers, and tone notation are
excluded from phoneme and allophone identifiers; they are reserved for future
prosody features.

## Logical units

An IPA identifier is measured two ways, both enforced independently:

- **1–8 Unicode scalar values**, and
- **at most 4 logical IPA base units**.

A combining diacritic or a spacing modifier must attach to a valid base symbol.
A standalone modifier, or a combining mark with no preceding base, is invalid.

Each logical base unit allows **at most one modification** — either one combining
diacritic **or** one spacing modifier, never both. NFC-precomposed characters are
interpreted by their canonical decomposition when counting base-plus-modifier
units.

### Precomposed forms

A precomposed character is interpreted by its **full canonical decomposition**
when counting base-plus-modifier units:

- If the decomposition is exactly **one approved base plus one approved
  combining mark**, the character is an approved **composition pair**. It
  counts as **one** logical base unit carrying **one** modification — so `ã`
  is one modified unit, and `ã̃` is invalid (two modifications on one base).
- If the decomposition carries **two or more** combining marks, it is **not** a
  composition pair: it is *too many modifications* (e.g. `ṏ` = `o` + U+0303 +
  U+0308).
- A character that **is** an approved **base** is interpreted through its own
  decomposition for counting. `ç` is an approved base, and its inherited
  cedilla occupies the single modification slot, so `çʰ` is invalid.
- Decomposition never turns an inherited mark into an approved one. U+0327
  (combining cedilla) is not an approved combining diacritic, so an authored
  cedilla is rejected.

The set of approved composition pairs is **derived** from the ratified
inventories and the pinned Unicode 18.0.0 decomposition table, not stored as
separate inventory data. It currently contains **27** precomposed characters.

## Tie bars

The tie bars **U+0361** (upper) and **U+035C** (lower) join **exactly two** IPA
base symbols into **one** logical base unit. Tie bars cannot overlap or chain. A
modifier for a tied unit may appear **only after the second base**. Multiple
independent tied units, and a mix of tied and untied units, are allowed.

### Identity and uniqueness

- Untied `tʃ` and tied `t͡ʃ` are **distinct** IPA identities.
- `t͡ʃ` (upper tie bar) and `t͜ʃ` (lower tie bar) are **equivalent for
  uniqueness** — they count as the same identifier.

The authored tie-bar code point is **preserved** in the source and in canonical
output; the upper/lower distinction is folded only when computing comparison
keys. There is no other phonetic equivalence beyond Unicode canonical equivalence
under strict NFC and this explicit tie-bar equivalence. IPA uniqueness is global
across all root and allophone declarations, and there is **no case folding** for
IPA.

## Examples

These outcomes assume the final approved allowlist; they illustrate the
structural rules.

| Identifier | Outcome |
|------------|---------|
| `tʃ` | untied — 2 logical units |
| `t͡ʃ` | tied — 1 logical unit |
| `t͜ʃ` | tie-equivalent to `t͡ʃ` for uniqueness |
| `t͡ʃa` | mixed — 2 logical units |
| `t͡ʃd͡ʒ` | two independent tied units — 2 logical units |
| `t͡ʃʰ` | modifier correctly after the tied second base — valid |
| `tʰ͡ʃ` | invalid — modifier placed before the tie |
| `tʰʷ` | invalid — two modifications on one base |
| `ʰ` | invalid — standalone modifier |
| isolated `U+0303` | invalid — standalone combining mark |
| `ã` | valid — composition pair (`a` + U+0303): 1 unit, 1 modification |
| `ã̃` | invalid — the pair already carries one modification |
| `ç` | valid — approved base; inherited cedilla counts as the modification |
| `çʰ` | invalid — the cedilla already occupies the modification slot |
| `ṏ` | invalid — two combining marks over one base |

!!! info "Repertoire status"
    The IPA inventory has been generated, cross-checked, and pinned (107 base
    symbols, 27 combining diacritics, 9 spacing modifiers in the current data),
    with the canonical JCS SHA-256 pinned and asserted by test. The second
    human review of the diacritic and spacing-modifier inventories
    **completed 2026-10-23** (outcome match; `specs/sources/SOURCES.md` §7).
    OA-1 and OA-4 are **Ratified**. The IPA side of the repertoire is fully
    ratified; the only outstanding v0.1.0 freeze blocker is now OA-14. See
    [Specification status](../spec-status.md).
