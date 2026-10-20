# Pinned Unicode 18.0.0 UCD inputs — sources, hashes, generation

> Governed by [ADR 0007](../../../docs/decisions/0007-unicode-engine-icu4x.md).
> These are the official Unicode Character Database 18.0.0 files that
> `elang-unicode` generates its General_Category and NFC tables from. Unicode
> 18.0.0 was released 2026-09-16.

## Files (exact downloaded bytes)

| File | URL | Bytes | SHA-256 |
|---|---|---:|---|
| `UnicodeData.txt` | https://www.unicode.org/Public/18.0.0/ucd/UnicodeData.txt | 2243593 | `0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950` |
| `extracted/DerivedGeneralCategory.txt` → `DerivedGeneralCategory.txt` | https://www.unicode.org/Public/18.0.0/ucd/extracted/DerivedGeneralCategory.txt | 282143 | `d6b151d2d40ee9b1876d26f417980f45ffae47b6055ccf7203cb31f07a030f94` |
| `CompositionExclusions.txt` | https://www.unicode.org/Public/18.0.0/ucd/CompositionExclusions.txt | 9007 | `c759b100e9ae8960ae6b50fc5765950a7581fd9f30d4ef95f68f25a6d97e818e` |
| `NormalizationTest.txt` | https://www.unicode.org/Public/18.0.0/ucd/NormalizationTest.txt | 2863708 | `25a50d816764b04abfb4a646d3eb2b2a803284c3873d9a06757b94fe4513dde3` |
| `DerivedNormalizationProps.txt` | https://www.unicode.org/Public/18.0.0/ucd/DerivedNormalizationProps.txt | 1396877 | `98ac7f67d985fe781e317f6182e885e94cabb0c314769e6dd73e48b226931ccd` |

Downloaded 2026-10-20. Each file's first line declares its `-18.0.0` version.
Re-verify any copy:

```powershell
(Get-FileHash specs/sources/ucd-18.0.0/UnicodeData.txt -Algorithm SHA256).Hash.ToLower()
```

## Generated tables

| Output | Generator | Derived from |
|---|---|---|
| `crates/elang-unicode/src/gc_tables.rs` | `cargo run -p elang-unicode --bin gen_tables` | DerivedGeneralCategory.txt |
| `crates/elang-unicode/src/nfc_tables.rs` | same | UnicodeData.txt + CompositionExclusions.txt |

Table sizes (18.0.0): 741 forbidden General_Category ranges; 1002 non-zero
Canonical_Combining_Class entries; 2081 full canonical decompositions; 961
canonical composition pairs.

## Conformance

`crates/elang-unicode/tests/normalization_conformance.rs` runs the pinned
`NormalizationTest.txt` through `elang-unicode`'s NFC: **20,171 rows pass**.
This is the gate for claiming Unicode 18.0.0 NFC compliance (ADR 0007). The
`DerivedNormalizationProps.txt` file is retained for provenance and future
quick-check (NFC_QC) work.

## Licensing

The UCD files are © Unicode, Inc., distributed under the **Unicode License v3
(Unicode-3.0)** / Unicode Data Files terms, which permit redistribution with the
license notice. `Unicode-3.0` and `Unicode-DFS-2016` are already on the
`deny.toml` allowlist. ELANG's generated tables are a factual derivation (code
points, combining classes, decompositions) from these files.
