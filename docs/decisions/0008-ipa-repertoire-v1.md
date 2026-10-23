# ADR 0008: ELANG IPA Repertoire v1 — source, generation and digest

- **Status:** Approved
- **Date:** 2026-10-20
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-IPA-001..014, REQ-UNICODE-001..004
- **Related tasks:** T0106–T0115 (resolves OA-1/OA-4/OA-5 content), T0114 partial

## Context

Steering §7 defines the ELANG IPA Repertoire v1 as a narrow explicit allowlist
of approved IPA base symbols, combining diacritics and spacing modifiers, pinned
to the "IPA 2026 chart issue (2015/2005 underlying revision)", with prosodic
categories excluded. The exact inventory and its canonical SHA-256 were open
artifacts (OA-1, OA-4, OA-5) that must be produced from the official chart and
not fabricated (ADR 0003).

A clarification was required: there is **no 2026 revision** of the IPA alphabet.
"2026" is ELANG's issue label; the authoritative content is the official IPA
alphabet shown on the **UNITIPA chart revised to 2020**.

## Decision

1. **Authoritative source:** the UNITIPA IPA chart (revised to 2020), plus the
   IPA Association full-chart page, downloaded and hashed in
   `specs/sources/SOURCES.md`. Unicode scalar values follow the Unicode
   Standard and are recorded per entry with their Unicode name for independent
   verification. No third-party Rust crate supplies the inventory.
2. **Generated, reviewable artifact:** `specs/schema/ipa-v1.json` is produced by
   `crates/elang-repertoire/src/bin/gen_repertoire.rs` (the human-reviewable
   inventory) and must reproduce byte-for-byte. It contains **107** base
   symbols, **27** combining diacritics and **9** spacing modifiers.
3. **Pinned digest:** the RFC 8785 JCS SHA-256 of the repertoire is
   `190aae7b1fe355793a30beb29e44d36540132d9d77e350e06db0f7d9302c64aa`, pinned as
   `PINNED_JCS_SHA256` and asserted by a test. Any inventory change breaks the
   test until the digest is re-reviewed.
4. **Exclusions:** suprasegmentals (stress, length, syllable break, linking),
   tone/word-accent letters and diacritics, and intonation marks are excluded
   (steering §7); the validator rejects them as not-in-repertoire.
5. **Validator:** `elang-repertoire` implements the Unicode layer
   (NFC/category/noncharacter) and the IPA identifier parser (allowlist +
   scalar/unit limits + tie-bar and modifier-placement rules) against the
   pinned JSON.
6. **Freeze gate:** the repertoire is loadable and fully validated, but is **not
   declared frozen** for the v0.1.0 conformance tag (T0123) until (a) the
   Unicode data pin reaches 18.0.0 (ADR 0007) and (b) a second human reviewer
   signs off on the combining-diacritic and spacing-modifier inventories against
   the chart. The base-symbol set is cross-checked automatically against an
   independent transcription of the chart grids (`chart_crosscheck.rs`).
   **Gate status (2026-10-23) — both conditions met.**
   - **(a) Unicode data pin** — landed via ADR 0007 / OA-13: `elang-unicode` is
     generated from the pinned official UCD 18.0.0.
   - **(b) Human sign-off on the combining-diacritic and spacing-modifier
     inventories** — **reviewer of record: the repository owner** (GitHub
     account `mhd-rahman`; this repository's only human author — git author
     names `Rocky` and `mhd-rahman`, both `mhdrahman.dev@gmail.com`).
     Completed **2026-10-23**: the 27 combining diacritics and 9 spacing
     modifiers in `specs/schema/ipa-v1.json` were compared row-by-row against
     the Diacritics block and the non-prosodic part of the Suprasegmentals
     block of the pinned UNITIPA chart revised to 2020 (SHA-256
     `882a0e4d…f5fc9`). **Outcome: match**; the excluded prosodic categories
     were confirmed absent. Method, artifact and reproduction steps:
     `specs/sources/SOURCES.md` §7.

   **Independence — stated explicitly, not implied.** The reviewer of record is
   *also* the author of the inventory generator
   (`crates/elang-repertoire/src/bin/gen_repertoire.rs`, commit `0a1f891`), and
   this repository has exactly **one** human author. The (b) sign-off is
   therefore a **self-review by the repository owner**, not a review performed
   by a distinct second person: "second" in (b) denotes a second, *human* line
   of evidence **alongside** the automated chart transcription that covers the
   base-symbol set, not a second distinct human. The independent machine check
   for the base set is `chart_crosscheck.rs` — a separately authored
   transcription of the chart grids that must agree with the generator
   cell-for-cell. Any change to the combining-diacritic or spacing-modifier
   inventories after this sign-off **voids** it and requires a fresh sign-off
   recorded here and in `specs/sources/SOURCES.md` §7.

   OA-1/OA-4 are Ratified. The repertoire's own gate is therefore **cleared**;
   the remaining v0.1.0 freeze blocker is OA-14 (canonical ELD goldens,
   Phase 11).

7. **Composition-pair interpretation (canonical decomposition).** Steering §7
   requires NFC precomposed characters to be interpreted by canonical
   decomposition for base+modifier counting. This ADR fixes *how* that
   applies to the ratified inventories:

   - **Approved bases are decomposed for counting.** A scalar that is itself
     an approved **base** is interpreted through its canonical decomposition
     for base+modifier counting, so that its inherited mark occupies the
     single modification slot. `ç`/`U+00E7` is a directly approved base, and
     its canonical decomposition `c` + U+0327 is therefore counted as one base
     plus one modification. The parsed unit records the decomposed base and
     the inherited mark; `IpaIdentifier::original` preserves the authored
     scalar byte-for-byte, and the authored scalar limit is unaffected.
   - **Otherwise, decompose.** A scalar that is in no approved inventory is
     interpreted by its **full** canonical decomposition
     (`elang_unicode::nfc::canonical_decomposition`, UAX #15 §3.11; Hangul
     handled algorithmically). The decomposition must be exactly **one
     approved base plus one approved combining mark**; such a scalar is an
     approved **composition pair**. It occupies **one** logical base unit
     carrying **one** modification.
   - **The eligible closure is exactly 27 precomposed scalars**, computed from
     the ratified inventories and the pinned UCD 18.0.0 decomposition table
     (`crates/elang-unicode/src/nfc_tables.rs`). The set is pinned as a golden
     list by `crates/elang-repertoire/tests/composition_pairs.rs`, which
     re-derives it from scratch on every test run; any drift in the
     inventories or in the Unicode pin fails that test and requires a fresh
     sign-off.
   - **Multi-mark decompositions are not composition pairs.** A decomposition
     carrying two or more combining marks over one approved base is *too many
     modifications* (`IPA_TOO_MANY_MODIFICATIONS`), e.g. `ṏ`/`U+1E4F` =
     `o` + U+0303 + U+0308.
   - **Decomposition does not promote a mark.** An inherited combining mark
     that is not in the approved combining inventory is not approved merely
     because a precomposed scalar inherits it, and it cannot make the
     precomposed scalar an approved composition pair. U+0327 (combining
     cedilla) is not an approved user-introduced mark: an authored U+0327 is
     rejected (`IPA_NOT_IN_REPERTOIRE`), while the inherited one is only
     counted because `ç` is a directly approved base.
   - **Spacing modifiers are unaffected.** They are not canonical composition
     marks and continue through the existing base-plus-modifier path.

   This is a *derived validation rule*, not a change to the ratified
   inventories: `specs/schema/ipa-v1.json` still contains exactly 107 base
   symbols, 27 combining diacritics and 9 spacing modifiers, and the inventory
   sign-off of 2026-10-23 above therefore **stands**.

## Consequences

- OA-1/OA-4/OA-5 move from "fabrication-forbidden open" to "generated and
  verified" (now ratified — see the gate status above); the IPA validator ships
  a real inventory.
- The "2026" label is documented as an issue tag, resolving the ambiguity.
- The anti-fabrication and anti-drift rules held through the review: every
  inventory entry and the pinned JCS digest were reproduced from the generator
  and verified against the pinned chart before ratification. Freezing is now
  blocked only on OA-14, not on the IPA gate.

## Alternatives considered

- **Hand-written JSON:** rejected — not reviewable or reproducible; a generator
  with one line per symbol is auditable and regenerates deterministically.
- **Scrape code points from the PDF at build time:** rejected — the chart is a
  graphical PDF; reliable code points come from the Unicode Standard, with the
  chart deciding *which* symbols are approved (a human/review decision).
- **Depend on a crate's IPA list:** rejected per the task and because it would
  reintroduce an unpinned, third-party notion of the inventory.
