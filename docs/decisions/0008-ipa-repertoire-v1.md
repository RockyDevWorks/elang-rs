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

## Consequences

- OA-1/OA-4/OA-5 move from "fabrication-forbidden open" to "generated and
  verified"; the IPA validator ships a real inventory.
- The "2026" label is documented as an issue tag, resolving the ambiguity.
- Freezing remains blocked on the Unicode 18.0 pin and a second review, so the
  anti-fabrication and anti-drift rules still hold through release.

## Alternatives considered

- **Hand-written JSON:** rejected — not reviewable or reproducible; a generator
  with one line per symbol is auditable and regenerates deterministically.
- **Scrape code points from the PDF at build time:** rejected — the chart is a
  graphical PDF; reliable code points come from the Unicode Standard, with the
  chart deciding *which* symbols are approved (a human/review decision).
- **Depend on a crate's IPA list:** rejected per the task and because it would
  reintroduce an unpinned, third-party notion of the inventory.
