# ADR 0007: Unicode 18.0.0 validation via a first-party `elang-unicode` crate

- **Status:** Approved
- **Date:** 2026-10-20 (revised same day; supersedes the earlier ICU4X stopgap decision recorded under this number)
- **Supersedes:** the initial ICU4X-stopgap form of ADR 0007
- **Superseded by:** none
- **Related requirements:** REQ-UNICODE-001..004, REQ-IPA-001 (NFC/category parts)
- **Related tasks:** T0116, T0801–T0805 (partial), open artifact OA-13

## Context

Steering §7 pins Unicode to **18.0.0** for all NFC and General_Category
behavior, forbids runtime-/OS-dependent Unicode tables, and forbids silent
version drift. Unicode **18.0.0 was released 2026-09-16** and its UCD files are
available. No proven Rust Unicode library ships 18.0.0 data yet:

- ICU4X 2.3 (`icu_normalizer`/`icu_properties`) compiled data is **Unicode 17.0**.
- `unicode-normalization` 0.1.25/0.1.26 data is **Unicode 17.0**.

An initial stopgap used ICU4X 17.0 data with a documented gap. That is a
**silent downgrade** of the 18.0.0 pin and is not acceptable per the steering
contract. The 17.0→18.0 difference is not cosmetic: 18.0 assigns 13,007 new
characters, and at least one (e.g. U+05C8 HEBREW POINT SHEVA NA MUGRASH, CCC 10)
changes NFC canonical **ordering** relative to 17.0 data — this was caught by
running the official 18.0.0 `NormalizationTest.txt` against the 17.0-data
implementation, which **failed**.

## Decision

Implement a first-party crate **`elang-unicode`** that provides Unicode 18.0.0
semantics from the **official pinned UCD 18.0.0 files**, not from any
third-party data crate or the host OS.

1. **General_Category:** a range table of the categories ELANG forbids
   (`Cc`/`Cf`/`Cs`/`Co`/`Cn`) is generated from the pinned
   `extracted/DerivedGeneralCategory.txt` (18.0.0). Because 18.0 newly assigns
   characters, this correctly treats them as assigned (not `Cn`).
2. **Noncharacters:** detected algorithmically (U+FDD0..FDEF and `..FFFE/FFFF`
   per plane); version-independent and stable per the Unicode stability policy.
3. **NFC:** the UAX#15 algorithm (full canonical decomposition → canonical
   ordering by Canonical_Combining_Class → canonical composition, with
   algorithmic Hangul per §3.12) implemented over **tables generated from the
   pinned UCD 18.0.0** (`UnicodeData.txt` + `CompositionExclusions.txt`). The
   algorithm is the standard, well-specified one; its correctness is proven by
   the conformance suite rather than asserted. This honours "prefer a proven
   implementation over writing NFC from scratch" by keeping the algorithm
   standard and gating it on the official test vectors; a stale-data third-party
   crate was tried first and rejected because it fails 18.0.0 conformance.
4. **Pinned inputs with recorded procedure and hashes:**
   `specs/sources/ucd-18.0.0/SOURCES.md` records the source URLs, the SHA-256 of
   each UCD input, and the regeneration command
   (`cargo run -p elang-unicode --bin gen_tables`). Running the generator
   against the pinned inputs reproduces the committed tables byte-for-byte.
5. **Conformance gate:** `tests/normalization_conformance.rs` runs the official
   18.0.0 `NormalizationTest.txt` (20,171 rows) through `elang-unicode`'s NFC.
   Unicode 18.0.0 NFC compliance is claimed **only because this test passes**.
6. **Single source of truth:** all ELANG Unicode validation goes through
   `elang-unicode`. `elang-repertoire::unicode` is now a thin adapter over it;
   ICU4X and `unicode-normalization` are removed from the dependency graph.
7. `DATA_UNICODE_VERSION == PINNED_UNICODE_VERSION == "18.0.0"` and
   `gc_data_matches_pin()` is `true`. There is no 17.0 downgrade anywhere.

## Evidence

- `cargo test -p elang-unicode`: 6 unit tests + the NFC conformance test pass;
  conformance stdout: `NFC conformance: 20171 rows passed (Unicode 18.0.0)`.
- `cargo test -p elang-repertoire`: 28 unit tests + the chart cross-check pass
  against the new backend.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Pinned UCD input SHA-256 (see SOURCES.md): `UnicodeData.txt`
  `0736451d…8950`, `DerivedGeneralCategory.txt` `d6b151d2…0f94`,
  `NormalizationTest.txt` `25a50d81…dde3`, `CompositionExclusions.txt`
  `c759b100…818e`, `DerivedNormalizationProps.txt` `98ac7f67…1ccd`.

## Consequences

- Unicode 18.0.0 is genuinely pinned and conformance-tested; the pin never
  silently downgrades. Dependent work (ELD string validation, IPA validation)
  runs on real 18.0.0 semantics.
- The generated tables are build artifacts under version control; changing the
  UCD inputs requires updating the pinned hashes and regenerating, which the
  conformance test re-validates.
- Writing the NFC kernel in-house is accepted because (a) no proven crate has
  18.0.0 data yet and (b) correctness is bound to the official conformance
  vectors. If a proven crate later ships 18.0.0 data, it may replace the kernel
  **only if** it still passes the same conformance gate.
- OA-13 (Unicode reference data) is satisfied for the NFC + General_Category
  behavior ELANG needs; the IPA repertoire freeze remains a **separate** Phase 1
  blocker (second human review), unaffected by this ADR.

## Alternatives considered

- **ICU4X / `unicode-normalization` with 17.0 data (the stopgap):** rejected —
  fails the 18.0.0 conformance test and is a silent downgrade of the pin.
- **`icu4x-datagen` to produce 18.0 data:** heavier toolchain dependency and
  uncertain property coverage for 18.0 at this date; the direct UCD generator is
  smaller, fully pinned, and conformance-tested.
- **Pretend the data is 18.0.0:** rejected outright (steering §0.3, §7).
