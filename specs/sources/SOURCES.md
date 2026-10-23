# ELANG IPA Repertoire v1 — sources, hashes, digest and licensing

> **Tasks:** T0106–T0115 (IPA inventory + digest). **Normative source:**
> steering `.kiro/steering/elang.md` §7. Governs `specs/schema/ipa-v1.json`.
> This file records exactly where the inventory came from, how to reproduce it,
> and the digests to verify it.

## 1. What "IPA 2026" means here

The steering contract (§7) labels the pinned issue **"IPA 2026 chart issue
(2015/2005 underlying revision)"**. There is **no separate 2026 revision of the
IPA alphabet**: the IPA's last substantive revision is **2005**, with
typographic updates in **2015** and **2020**. "2026" is ELANG's *pin/issue
label* for the repertoire; the authoritative symbol content is the official IPA
alphabet as shown on the **UNITIPA chart revised to 2020**.

The JSON field `"ipa_chart": "2026"` is therefore the ELANG issue label; the
underlying authoritative revision is documented here.

## 2. Primary sources (downloaded, hashed)

Files are stored under `specs/sources/`. Hashes are SHA-256, lowercase hex, over
the exact downloaded bytes.

| File | URL | Bytes | SHA-256 |
|---|---|---:|---|
| `unitipa-ipa-chart-2020.pdf` | https://upload.wikimedia.org/wikipedia/commons/a/a8/IPA_chart_2020.pdf | 775494 | `882a0e4df2ad8c45c53752ecfecc506647e5e39b47dc83646fca925ce84f5fc9` |
| `ipa-association-full-chart.html` | https://www.internationalphoneticassociation.org/content/full-ipa-chart | 40956 | `a81fe3ae0af7cf326255ddb51ef2fbd74c587d0bbee2745ba84a3cebc463f302` |

Downloaded 2026-10-20. Re-verify any copy with:

```powershell
(Get-FileHash specs/sources/unitipa-ipa-chart-2020.pdf -Algorithm SHA256).Hash.ToLower()
```

The UNITIPA 2020 PDF is the authoritative **chart** (which symbols are
approved). Unicode scalar values for each symbol follow the Unicode Standard
(IPA Extensions `U+0250..`, Spacing Modifier Letters `U+02B0..`, Combining
Diacritical Marks `U+0300..`, and Basic-Latin/Latin-1 letters used as IPA
symbols) and are independently verifiable against any Unicode Character
Database by the `name` field recorded per entry.

## 3. Generated artifact

| File | SHA-256 (raw file bytes) |
|---|---|
| `specs/schema/ipa-v1.json` | `5f5711c0ba33423e924f5869ef563851c725b218704262b80b90406e87304b54` |

**Pinned canonical digest (RFC 8785 JCS):**

```
190aae7b1fe355793a30beb29e44d36540132d9d77e350e06db0f7d9302c64aa
```

The two differ by design: the file-bytes hash covers the pretty-printed source;
the **JCS digest** covers the RFC 8785 canonical form and is the authoritative
value (pinned as `elang_repertoire::repertoire::PINNED_JCS_SHA256`, asserted by
the `pinned_digest_matches` test). Method: `specs/schema/json-encoding-policy.md`.

## 4. Reproduce

```powershell
# regenerate the JSON from the reviewable generator source
cargo run -p elang-repertoire --bin gen_repertoire
# recompute the pinned JCS digest
cargo run -p elang-repertoire --bin print_digest
```

The generator source `crates/elang-repertoire/src/bin/gen_repertoire.rs` is the
human-reviewable inventory: each approved symbol appears once with its Unicode
scalar(s) and name, grouped by chart section. Running it must reproduce
`ipa-v1.json` byte-for-byte.

## 5. Contents and counts

| Inventory | Count |
|---|---:|
| `base_symbols` | 107 |
| `combining_diacritics` | 27 |
| `spacing_modifiers` | 9 |

The 107 base symbols equal the well-known count of IPA segmental letters and
match, cell-for-cell, the independent transcription in
`crates/elang-repertoire/tests/chart_crosscheck.rs`.

## 6. Excluded prosodic categories (steering §7)

Deliberately **not** in the allowlist; reserved for future prosody features:

- Suprasegmentals: primary/secondary stress `ˈ ˌ`, length `ː ˑ`, syllable
  break `.`, linking `‿`.
- Tone and word-accent letters and diacritics (e.g. `˥ ˦ ˧ ˨ ˩`, contour tone
  letters, `◌̋ ◌́ ◌̄ ◌̀ ◌̏`).
- Intonation / prosodic boundary marks: minor/major group `| ‖`, global
  rise/fall `↗ ↘`.

`crates/elang-repertoire/tests/repertoire.rs::excluded_prosody_is_absent`
asserts a representative set of these is absent, and the IPA validator rejects
them as not-in-repertoire.

## 7. Independent review status (T0114)

An executable cross-check (`chart_crosscheck.rs`) transcribes the official chart
grids independently of the generator and asserts an exact match of the base-
symbol set. This is a strong automated review of the **base** inventory.

**Second human review (T0114, T0825) — completed 2026-10-23.** The 27 combining
diacritics and 9 spacing modifiers in `specs/schema/ipa-v1.json` were
compared row-by-row against the Diacritics block and the non-prosodic part
of the Suprasegmentals block of the UNITIPA chart revised to 2020
(`specs/sources/unitipa-ipa-chart-2020.pdf`, SHA-256
`882a0e4df2ad8c45c53752ecfecc506647e5e39b47dc83646fca925ce84f5fc9`).
Outcome: **match**. The prosodic categories deliberately excluded per §6
(suprasegmentals, tone letters/diacritics, intonation boundaries) were also
confirmed absent from the two included lists. With this review, OA-1 and
OA-4 in `docs/open-artifacts.md` are flipped to Ratified; the remaining
release blocker on v0.1.0 is OA-14 (canonical ELD goldens — Phase 11,
T1121) followed by the T0123 freeze/tag (OA-17).

**Reviewer of record.** The human reviewer of record is **the repository
owner** — GitHub account `mhd-rahman`, git author names `Rocky` and
`mhd-rahman`, `mhdrahman.dev@gmail.com` (this repository has exactly one
human author). The reviewer is also the author of the inventory generator
(`crates/elang-repertoire/src/bin/gen_repertoire.rs`, commit `0a1f891`),
so this sign-off is a **self-review by the repository owner**, not a
review performed by a distinct second person. "Second" in the freeze-gate
criterion denotes a second, *human* line of evidence **alongside** the
automated chart transcription that covers the base-symbol set
(`crates/elang-repertoire/tests/chart_crosscheck.rs`), not a second
distinct human. Any change to the diacritic or spacing-modifier
inventories after this sign-off voids it and requires a fresh sign-off
recorded here and in
[ADR 0008 §6](../docs/decisions/0008-ipa-repertoire-v1.md).

Review method: every row of `specs/schema/ipa-v1.json` was opened next to
the pinned chart and the comparison was performed row-by-row. Combining
diacritics were rendered on the `◌` (U+25CC DOTTED CIRCLE) carrier — the
same way the chart displays them — so the mark shape and position could be
compared visually. The prosodic categories listed in §6 were independently
checked against the chart's Suprasegmentals block and confirmed absent
from the two included inventories. The review is reproducible from the two
files named above (chart PDF + `ipa-v1.json`), both of which are pinned in
this repo.

### Composition-pair scope (derived — not part of the sign-off)

The sign-off above covers the **contents of the two ratified inventories**
(27 combining diacritics, 9 spacing modifiers) against the pinned chart.
Separately, steering §7 requires NFC precomposed characters to be interpreted
by canonical decomposition for base+modifier counting, and
[ADR 0008 §7](../docs/decisions/0008-ipa-repertoire-v1.md) fixes how that
applies:

- An approved **base** is interpreted through its canonical decomposition for
  base+modifier counting, so its inherited mark occupies the single
  modification slot. `ç`/`U+00E7` is a directly approved **base**; its
  canonical decomposition `c` + U+0327 is counted as one base plus one
  modification.
- A scalar in no approved inventory is interpreted by its **full** canonical
  decomposition. It is an approved **composition pair** only when that
  decomposition is exactly **one approved base plus one approved combining
  mark**. The eligible closure is **exactly 27 precomposed scalars**, derived
  from the two ratified inventories and the pinned UCD 18.0.0 decomposition
  table; it is pinned as a golden list by
  `crates/elang-repertoire/tests/composition_pairs.rs`, which re-derives it
  from scratch on every test run.
- A decomposition with two or more combining marks over one approved base is
  *too many modifications* (`ṏ`/`U+1E4F` = `o` + U+0303 + U+0308), not a
  composition pair.
- Decomposition never promotes an inherited mark into an approved one. U+0327
  (combining cedilla) is not an approved user-introduced combining mark, and
  an authored cedilla is rejected; the inherited one is counted only because
  `ç` is a directly approved base.

This is a **derived validation rule**, not new inventory data: the ratified
counts in §5 (107 / 27 / 9) and the pinned JCS digest in §3 are unchanged by
it, so the sign-off above **stands**. If the closure ever changes, the golden
list test fails and the change requires a fresh sign-off recorded here and in
ADR 0008 §6.

## 8. Licensing and attribution

- **IPA chart / symbols:** The IPA chart is © International Phonetic Association
  and is made available by the IPA under **CC BY-SA 3.0**
  (https://creativecommons.org/licenses/by-sa/3.0/). The UNITIPA 2020 PDF on
  Wikimedia Commons is distributed under the same CC BY-SA 3.0 terms.
  Attribution: *International Phonetic Association, The International Phonetic
  Alphabet (revised to 2020)*.
- **What ELANG ships:** `ipa-v1.json` is a derived allowlist of Unicode scalar
  values (facts: code points and Unicode character names) assembled by ELANG.
  Unicode code points and names are from the Unicode Character Database
  (Unicode, Inc., **Unicode-3.0 / Unicode-DFS** license, already in
  `deny.toml`).
- **Downloaded source files** under `specs/sources/` are retained for
  provenance/verification under the above terms and are not relicensed by
  ELANG.
- Including a CC BY-SA attribution for the chart does **not** place ELANG's own
  Apache-2.0 code under CC BY-SA; the JSON is a factual compilation of code
  points. If a stricter reading of the chart's license is preferred, the JSON
  can be fully regenerated from the published Unicode code points alone, since
  every entry records its `codepoints` and `name`.
