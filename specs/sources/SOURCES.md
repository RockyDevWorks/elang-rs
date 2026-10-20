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
symbol set. This is a strong automated review of the **base** inventory. The
combining-diacritic and spacing-modifier inventories are reviewed against the
chart's Diacritics block; a second human reviewer should still sign off before
the repertoire is *frozen* for the v0.1.0 release tag (T0123). Until then the
repertoire is loadable and validated but not declared frozen (ADR 0003, §12).

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
