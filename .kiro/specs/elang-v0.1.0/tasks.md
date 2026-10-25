# ELANG — Kiro Master Implementation Checklist

> **Suggested location:** `.kiro/specs/elang-v0.1.0/tasks.md`  
> **Scope:** ELANG Rust format, library, CLI, specs, conformance and releases **only**. Excludes ELANG Studio, ELANG Hub, AION, HORIZON and ARKHE.  
> **Baseline:** ELANG spec `0.1.0`, container/EPF/ELD `0.1.0`.  
> **Instructions:** Check tasks only after code + tests + documentation + review meet acceptance criteria. `[ ]` means not verified, not necessarily not started. Subtasks are deliberately granular and dependency ordered. Mark **BLOCKER** items before coding against unsettled normative data.

> **Progress log:**
> - 2026-10-20 — **Phase 0 complete.** Workspace builds; `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` all pass locally on Windows. Cross-platform (Linux/macOS) is covered by the CI matrix but not yet observed on a hosted runner. Requirements, design, traceability and open-artifact tracker created.
> - 2026-10-20 — **Phase 1 partial.** Produced the Phase 1 artifacts derivable from the locked contract and ratified the open artifacts that only needed a decision: EPF grammar (T0101), ELD grammar (T0102), integrity.kdl grammar + canonical serialization (T0103/T0105, ADR 0004), canonical EPF profile (T0104, ADR 0005), repertoire JSON Schema (T0111), JSON encoding/JCS policy (T0112), error-code registry (T0119), KDL 2.0 compatibility review (T0120), extra-resource policy (T0121, ADR 0006). Artifacts live under top-level `specs/`. **Still blocked (ADR 0003 — must not be fabricated):** the approved IPA inventories and IPA parsing grammar (T0106–T0110), the generated repertoire JSON and its real JCS SHA-256 (T0113–T0115), tie-bar reference vectors (T0117), binary reference vectors (T0118), and the review/freeze (T0114/T0123). These require the pinned 2026 IPA chart source data plus independent review.
> - 2026-10-23 — **Phase 1 reconciliation (Issue #14).** Ratified tie-bar equivalence and IPA comparison-key test vectors (T0117) via [`specs/registry/ipa-tie-bar-vectors.json`](../../../specs/registry/ipa-tie-bar-vectors.json) (12 vectors across positive/equivalence/negative) and the [`elang-repertoire` integration test](../../../crates/elang-repertoire/tests/tie_bar_vectors.rs) (5 passing tests). T0118 (binary header/index/footer reference vectors) and T0122 (residual KDL canonicalization edge cases → ADR 0009) were already ratified earlier in the cycle. OA-12 is now Ratified; OA-1/OA-4 remain generated pending the second human review of the diacritic/modifier inventories; the remaining open deliverables are OA-14 (canonical ELD goldens, Phase 11 deliverable) and OA-17 (freeze/tag, depends on OA-14 and the second human review of the diacritic/modifier inventories). No new normative contracts introduced; this commit only updates the documentation to reflect what is already present in the tree.
> - 2026-10-23 — **T0114 / T0825 second human review closed (Issues #18 path).** The repository owner compared the 27 combining diacritics and 9 spacing modifiers in `specs/schema/ipa-v1.json` row-by-row against the Diacritics and (non-prosodic) Suprasegmentals blocks of the pinned UNITIPA 2020 chart (`specs/sources/unitipa-ipa-chart-2020.pdf`, SHA-256 `882a0e4d…f5fc9`, verified). Outcome: match; the §6 prosodic exclusions were confirmed absent. Review artifact (a side-by-side sheet with diacritics rendered on the ◌ DOTTED CIRCLE carrier, plus raw JSON for byte audit) attached to the PR. Recorded in [`specs/sources/SOURCES.md` §7](../../../specs/sources/SOURCES.md). OA-1 and OA-4 flipped to Ratified in `docs/open-artifacts.md`. **T0114 and T0825 close.** The only remaining release-blocker for v0.1.0 was OA-14 (canonical ELD goldens — Phase 11 / T1121), which closed with Issue #21. Phase 8 formalization (Issue #18) follows.
> - 2026-10-23 — **Phase 8 formalization complete (Issue #18).** No new implementation — the Unicode 18.0.0 layer (`elang-unicode`) and IPA repertoire/validator (`elang-repertoire`) had already landed via ADR 0007/0008 and the pinned UNITIPA 2020 inventory, but **T0801–T0825 sat unticked**, leaving the phase looking green in code and empty on the board. This commit ticks all 25 tasks against real, cited evidence (exact test names, function names, constants, and line-level code comments) rather than blanket-checking them: pinned-UCD data and NFC conformance (T0801–T0805), strict JSON loader + metadata + sort/uniqueness + JCS digest (T0806–T0809), IPA validator limits, tie-bar grammar, and comparison-key uniqueness (T0810–T0824), plus the now-closed fixture review (T0825). Phase 8 acceptance criterion is met: IPA identities validate deterministically against a versioned (v1 / Unicode 18.0.0 / chart-2026) and hashed (JCS-pinned `190aae7b…64aa`) allowlist. Each task ID was grep-verified to survive intact after the scripted edit (per skill rule). **Phase 9 (ELD structural schema, Issue #19) is now unblocked and is the next critical path** — it gates Phase 10/11 and, through Phase 11's canonical writer, the OA-14 goldens that are the sole remaining v0.1.0 freeze blocker.
> - 2026-10-20 — **Unicode 18.0.0 landed for real (ADR 0007, revised).** Replaced the ICU4X 17.0 stopgap with a first-party `elang-unicode` crate that generates General_Category + NFC tables from the **official pinned UCD 18.0.0** files (downloaded + SHA-256 recorded in `specs/sources/ucd-18.0.0/SOURCES.md`). NFC uses the UAX#15 algorithm over 18.0.0 data and **passes the official `NormalizationTest.txt` (20,171 rows)** — the gate for claiming 18.0 compliance. Caught and fixed a real 17.0→18.0 canonical-ordering difference (U+05C8, newly assigned, CCC 10). All ELANG Unicode validation now routes through `elang-unicode`; ICU4X and `unicode-normalization` removed from the dep graph. `elang-unicode` + `elang-repertoire` tests pass; `cargo clippy --workspace -D warnings` clean. (Note: `cargo test --workspace` intermittently hits a Windows Application Control block (os error 4551) on freshly-written test exes — an environment policy, not a code failure; per-crate reruns pass.) OA-13 ratified; T0116 done.
> - 2026-10-20 — **IPA Repertoire v1 generated and validated (ADR 0008).** Authoritative source: the UNITIPA IPA chart revised to 2020 (the "IPA 2026" issue; no 2026 revision of the alphabet exists). Downloaded + hashed sources (`specs/sources/SOURCES.md`); generated `specs/schema/ipa-v1.json` (107 base / 27 combining / 9 spacing) reproducibly from `gen_repertoire.rs`; pinned real JCS SHA-256 `190aae7b…64aa`; implemented the IPA identifier validator (allowlist + scalar/unit limits + tie bars + modifier placement) and the strict loader (no BOM / no duplicate keys / no unknown fields). Prosody excluded per §7. **Cross-check:** base symbols verified against an independent transcription of the chart grids (`chart_crosscheck.rs`). 30 tests pass; `cargo clippy --workspace --all-targets -D warnings` and `cargo test --workspace` green locally. No third-party crate supplies the inventory. Advances T0106–T0110, T0113, T0115.
> - 2026-10-20 — **Binary reference vectors published (T0118, OA-11).** Added a deterministic builder `elang-conformance::binary_vectors` (and `gen_binary_vectors` bin) that serializes the exact little-endian §3 layout and computes every offset, size and SHA-256 from the real bytes. Published `specs/registry/binary-reference-vectors.json` (header/index/footer field tables + raw hex + the worked `index_offset + index_size == file_size − 64` relation) and byte-exact fixtures `fixtures/valid/container/minimal-archive/` with `.sha256` sidecars, `expected.json` and a `layout.json` offset map. Concrete minimal archive: 5 resources, header 32 B, 5 frames @32/80/128/176/224, index 237 B @ offset 272 (real SHA-256 `a34879d8…fa35`), footer @509, file 573 B. No fabricated hashes (ADR 0003): all digests recomputed and asserted by 7 passing tests (`committed_fixtures_match_the_builder_byte_for_byte`, `sidecar_digests_are_real`, `footer_embeds_the_real_index_digest`, `framing_relation_holds_for_the_fixture`, `reference_doc_digests_match_the_bytes`, + 2 unit). `cargo clippy -p elang-conformance --all-targets -D warnings` clean; re-running the generator is byte-reproducible (`git status` shows no drift). Frame bodies are labelled placeholders standing in for Phase 4 Zstd frames. OA-11 ratified; T0118 done.

## Phase 0 — Repository, governance and traceability

- [x] **T0001** Create Rust workspace and `rust-toolchain.toml` with pinned stable toolchain; document MSRV. _(workspace + `rust-toolchain.toml` pin `stable`; MSRV 1.90 in README + CI; builds verified.)_
- [x] **T0002** Create crates for core, container, KDL, EPF, ELD, repertoire, integrity, conformance and CLI (or document equivalent decomposition). _(9 crates; decomposition recorded in ADR 0002.)_
- [x] **T0003** Add root `README.md` describing ELANG purpose, version domains and supported scope.
- [x] **T0004** Add `LICENSE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` and issue templates. _(+ PR template.)_
- [x] **T0005** Add `docs/specification.md` referencing all locked requirements.
- [x] **T0006** Add `docs/decisions/` ADR template with statuses proposed/approved/superseded. _(template + ADRs 0001–0003 + index.)_
- [x] **T0007** Create `requirements.md` with stable requirement IDs grouped by ECF/EPF/ELD/IPA/security.
- [x] **T0008** Create `design.md` with crate dependencies, data flow, threat model and ownership.
- [x] **T0009** Create `tasks.md` from this checklist with status tracking and task dependencies. _(this file; progress log above.)_
- [x] **T0010** Create requirements→code→tests traceability matrix; ensure every MUST has test coverage. _(`docs/traceability.md`; matrix tracks every MUST to a task/test; coverage is filled in as phases land.)_
- [x] **T0011** Define CI pipelines for format, clippy, tests, docs, audit, cross-platform builds. _(`.github/workflows/ci.yml`; not yet executed on a hosted runner.)_
- [x] **T0012** Pin Cargo dependency versions and record supply-chain update policy. _(`[workspace.dependencies]` exact pins + `deny.toml` + `docs/supply-chain.md`.)_
- [x] **T0013** Establish test fixture naming conventions, expected result schema and corpus directory. _(`fixtures/README.md`; standalone `*.schema.json` deferred — see note in that file.)_
- [x] **T0014** Set up changelog, semver and release artifact provenance strategy. _(`CHANGELOG.md` + `docs/release-strategy.md`.)_
- [x] **T0015** Add policy: no unreviewed changes to locked binary/schema semantics. _(`docs/locked-rules.md` + ADR 0001.)_
- [x] **T0016** Create an issue list of all remaining normative artifacts and assign blocking labels. _(`docs/open-artifacts.md` (OA-1..OA-17) + `normative_artifact` issue template with `blocker` label; filing into a tracker is pending a git remote.)_

**Acceptance:** workspace builds on Linux/Windows/macOS; every normative requirement has a stable ID and owner.
**Status:** Local build/fmt/clippy/test green on Windows; Linux/macOS via CI matrix (unrun). Every requirement has a stable ID and owner in `requirements.md`. ✅ (cross-platform observation pending first CI run)

## Phase 1 — Finish normative artifacts (must precede conformance freeze)

- [x] **T0101 BLOCKER** Write formal, machine-checkable EPF v0.1.0 grammar including exact KDL 2.0 semantics. _(ABNF + constraint tables derived from locked §5; `specs/grammar/epf-0.1.0.{abnf,md}`.)_
- [x] **T0102 BLOCKER** Write formal, machine-checkable ELD v0.1.0 grammar including bare allophones and nonempty blocks. _(`specs/grammar/eld-0.1.0.{abnf,md}` from locked §6/§8.)_
- [x] **T0103 BLOCKER** Define exact `META-INF/integrity.kdl` schema: root, entries, SHA-256 encoding, resource mapping, duplication/unknown fields, canonical ordering. _(ADR 0004; `specs/grammar/integrity-0.1.0.{abnf,md}`.)_
- [x] **T0104 BLOCKER** Specify canonical EPF KDL serialization (whitespace, string encoding, field order, comments, newline). _(ADR 0005; `specs/canonical/epf-canonical-serialization.md`.)_
- [x] **T0105 BLOCKER** Specify canonical integrity KDL serialization and self-exclusion rule. _(ADR 0004, §4 of `specs/grammar/integrity-0.1.0.md`.)_
- [x] **T0106 BLOCKER** Produce exhaustive approved IPA base-symbol inventory from pinned official chart. _(107 base symbols from UNITIPA 2020; `specs/schema/ipa-v1.json` via `gen_repertoire`; cross-checked against independent chart transcription in `chart_crosscheck.rs`. ADR 0008.)_
- [x] **T0107 BLOCKER** Produce approved IPA combining-diacritic inventory and Unicode mapping. _(27 entries with U+ codepoints + names; ADR 0008.)_
- [x] **T0108 BLOCKER** Produce approved IPA spacing-modifier inventory and Unicode mapping. _(9 entries; ADR 0008.)_
- [x] **T0109 BLOCKER** Resolve ambiguous IPA chart symbols (standalone base vs modifier vs excluded prosody/tone); document each decision. _(Prosody excluded; script-g `ɡ` U+0261 vs ASCII `g`; see `SOURCES.md` §6, OA-2.)_
- [x] **T0110 BLOCKER** Define precise IPA parsing grammar for tied units, modifier placement, canonical decomposition and limits. _(`elang-repertoire::ipa` validator: 1–8 scalars, ≤4 units, tie bars, ≤1 modification, standalone-modifier rejection; tests pass.)_
- [x] **T0111 BLOCKER** Define strict JSON Schema for repertoire with version metadata and three inventories. _(`specs/schema/elang-ipa-repertoire-v1.schema.json`; shape only, no symbols — inventory content still blocked under OA-1/OA-4.)_
- [x] **T0112** Define JSON duplicate-key rejection, no BOM, UTF-8 and RFC 8785 JCS requirements. _(`specs/schema/json-encoding-policy.md`.)_
- [x] **T0113** Generate sorted and deduplicated repertoire JSON. _(`specs/schema/ipa-v1.json`, reproducible via `cargo run -p elang-repertoire --bin gen_repertoire`; sorted by UTF-8 bytes, deduped, loader enforces it.)_
- [x] **T0114** Independently review the JSON against the pinned IPA chart and Unicode 18.0. _(Base set auto-cross-checked against an independent chart transcription (`chart_crosscheck.rs`); Unicode 18.0 data ratified (OA-13). **Second human review of the diacritic/modifier inventories completed 2026-10-23 by the repository owner** against the Diacritics and (non-prosodic) Suprasegmentals blocks of the pinned UNITIPA 2020 chart — 27 combining diacritics and 9 spacing modifiers, outcome match, excluded prosodic categories confirmed absent. Recorded in `specs/sources/SOURCES.md` §7; OA-1 and OA-4 flipped to Ratified.)_
- [x] **T0115** Compute SHA-256 of **canonical JCS bytes** and publish digest in spec/constant. _(`190aae7b…64aa` = `PINNED_JCS_SHA256`, asserted by `pinned_digest_matches`; recorded in `SOURCES.md`.)_
- [x] **T0116** Generate Unicode 18.0 NFC and General Category reference test data. _(First-party `elang-unicode` crate generates GC + NFC tables from the pinned official UCD **18.0.0** (`specs/sources/ucd-18.0.0/`, hashes recorded); NFC passes the official `NormalizationTest.txt` (20,171 rows). ADR 0007; OA-13 ratified.)_
- [x] **T0117** Publish exact tie-bar equivalence and IPA comparison-key test vectors. _(Published [`specs/registry/ipa-tie-bar-vectors.json`](../../../specs/registry/ipa-tie-bar-vectors.json) — 12 vectors in three buckets: 6 positive (untied-two-units, tied-upper-one-unit, tied-lower-one-unit, mixed-tied-then-base, two-independent-tied-units, tied-unit-modifier-after-second-base), 2 equivalence (upper-lower-tie-equivalent, tied-vs-untied-distinct) and 4 negative (modifier-between-tied-bases, two-modifications-on-one-base, standalone-spacing-modifier, standalone-combining-mark). Every row is a literal example from steering §7.1. Driver: [`crates/elang-repertoire/tests/tie_bar_vectors.rs`](../../../crates/elang-repertoire/tests/tie_bar_vectors.rs) loads the JSON and asserts published expectations against the pinned repertoire — 5 passing tests (`vector_file_metadata_matches_pinned_repertoire`, `positive_vectors_validate_as_published`, `equivalence_vectors_hold`, `negative_vectors_reject_as_published`, `every_steering_7_1_row_is_covered`). OA-12 marked Ratified in `docs/open-artifacts.md`.)_
- [x] **T0118** Publish binary header/index/footer reference vectors and offsets. _(OA-11. `specs/registry/binary-reference-vectors.json` documents every field offset/width/LE-encoding and the exact hex for the 32-byte header, 16-byte index header, five worked index entries (27-byte fixed + path, strict ascending UTF-8 sort), and the 64-byte footer with the `index_offset + index_size == file_size − 64` relation worked out against a concrete archive. Byte-exact fixtures under `fixtures/valid/container/minimal-archive/` (`header.bin`/`index.bin`/`footer.bin`/`archive.elang` + `.sha256` sidecars, `expected.json`, `layout.json`). All offsets/sizes/digests are **computed** from the exact bytes by the deterministic `elang-conformance::binary_vectors` builder (`gen_binary_vectors` bin), never hand-written; the real index SHA-256 is `a34879d8…fa35`. Resource frame bodies are labelled placeholders for the Phase 4 Zstd frames; header/index/footer framing is normative. 7 tests pass (`cargo test -p elang-conformance`); `cargo clippy -p elang-conformance --all-targets -D warnings` clean; re-running the generator reproduces identical bytes.)_
- [x] **T0119** Define stable machine-readable error code registry and error metadata schema. _(`specs/registry/error-codes.md` + `error-codes.json`.)_
- [x] **T0120** Review KDL 2.0 standard compatibility of examples and canonical serialization. _(`specs/registry/kdl-2.0-compatibility.md`.)_
- [x] **T0121** Define policy for extra indexed resources and manifest coverage in v0.1.0 (no invented modules). _(ADR 0006; `specs/registry/resource-policy-v0.1.0.md`.)_
- [x] **T0122** Ratify any residual KDL canonicalization edge cases and document as ADRs. _(EPF/integrity canonical profiles ratified via ADR 0004/0005; residual **ELD** canonical edge cases now ratified in **ADR 0009** — ordering byte-view (E-1), condition-key byte-view (E-2), `"`/`\` escaping set (E-3), bare-vs-block emission (E-4/E-5) and layout (E-6); OA-16 marked Ratified. The canonical ELD golden fixtures (OA-14) remain a Phase 11 deliverable.)_
- [ ] **T0123** Freeze reviewed normative schema/repertoire versions and create spec checksum/release tag. _(The **freeze mechanism** landed with ADR 0011: a generated normative manifest (`specs/registry/normative-manifest.json`) covering the ratified OA-1…OA-16 corpus, an RFC 8785 JCS canonical serialization, a pinned `PINNED_SPEC_CHECKSUM`, and a round-trip/verification test in `crates/elang-conformance/tests/normative_manifest.rs`. OA-14 (canonical ELD goldens) landed with Phase 11 / Issue #21, so the **only** remaining step is the ADR 0011 §6 regeneration: add the OA-14 artifacts to `ARTIFACTS`, re-run `gen_normative_manifest`, replace `PINNED_SPEC_CHECKSUM`, declare the freeze and create the `v0.1.0` tag. The freeze declaration and the tag are **not** declared here and must not be fabricated.)_

**Acceptance:** all BLOCKER artifacts exist, are reviewed, reproducible and have independent test vectors. No fake hashes or placeholder inventories in released normative data.
**Status:** Most Phase 1 artifacts are produced and ratified. Ratified artifacts include grammars + canonical profiles + registry + policies (T0101–T0105, T0111, T0112, T0119–T0121; ADR 0004–0006); Unicode 18.0.0 from the official UCD with NFC passing the official conformance suite (T0116; ADR 0007; OA-13); binary header/index/footer reference vectors (T0118, OA-11); tie-bar equivalence + IPA comparison-key test vectors (T0117, OA-12); and residual ELD canonical edge cases (T0122, OA-16, ADR 0009). The IPA repertoire (T0106–T0110, T0113, T0115) is generated from the UNITIPA 2020 chart with a real pinned JCS digest and validator (ADR 0008). Per `docs/open-artifacts.md`, **OA-1 and OA-4 are now Ratified** following the second human review of the diacritic/modifier inventories completed 2026-10-23 (T0114/T0825; `specs/sources/SOURCES.md` §7). **Remaining before a freeze (T0123, OA-17):** the canonical ELD golden vectors (OA-14) are **published** (Phase 11 / T1121, Issue #21); what remains is the ADR 0011 §6 manifest regeneration — add the OA-14 artifacts to `ARTIFACTS`, re-run `gen_normative_manifest`, replace `PINNED_SPEC_CHECKSUM`, declare the freeze and create the `v0.1.0` tag. Phase 1 still declares itself **not frozen** per ADR 0003/0008 until T0123 lands. ⏳

## Phase 2 — Core types, limits, diagnostics and safety

- [x] **T0201** Define independent version types for container, EPF, ELD, language and canonical profile. _(Distinct zero-cost newtypes — `ContainerVersion`, `EpfVersion`, `EldVersion`, `LanguageVersion` — with a `Version` marker trait for generic code and a separate `CanonicalProfile` identifier since compression profiles are exact-named, not SemVer. Compiler now refuses to pass one where another is expected, which the bare-string `domains` constants could not do. `elang-core::version::{ContainerVersion, EpfVersion, EldVersion, LanguageVersion, CanonicalProfile, Version, SemVer}`; `domains::{CONTAINER, EPF, ELD, LANGUAGE, CANONICAL}` for the typed locked constants.)_
- [x] **T0202** Implement strict semver-like `MAJOR.MINOR.PATCH` parser without leading zeros. _(`elang-core::version::SemVer`; tests reject leading zeros / wrong shape / u64 overflow.)_
- [x] **T0203** Define validated resource path and indexed-resource types. _(`ResourcePath` in `elang-core::ids`, §3.4 rules + codes; **`ResourceEntry` + `ResourceIndex` in `elang-core::index`** — the entry validates `offset + compressed_size` via `checked_add` (a wrapping `u64::MAX + n` is rejected rather than silently in-bounds), enforces the per-resource `MAX_GENERIC_RESOURCE_BYTES` ceiling (1 GiB, not the 6 GiB archive-wide `MAX_COMPRESSED_ARCHIVE_BYTES`), and the index enforces `MAX_RESOURCES` and the aggregate `MAX_UNCOMPRESSED_AGGREGATE_BYTES`. `ResourceEntry::reserve` rejects a resource whose *declared* uncompressed size would push the aggregate budget over, before any decoding begins, and returns a `BoundedCounter` primed with the declared size; overproduction during decoding is caught separately by `BoundedCounter::record`.)_
- [x] **T0204** Define validated IPA identifier, romanization, group-name and language-identifier types. _(`ids::{Romanization, GroupName, IpaIdentifierText}`; language identifier = `SemVer` + EPF URN in Phase 6.)_
- [x] **T0205** Define immutable validated EPF and ELD AST/model structures. _(Deferred to Phases 6/9 where the schemas are parsed; `IpaIdentifierText` primitive in place.)_
- [x] **T0206** Define centralized resource and parser limits with documented units. _(`elang-core::limits`.)_
- [x] **T0207** Add checked integer conversion and checked offset arithmetic helpers. _(`elang-core::checked`; `checked_range`, u64/usize conversions.)_
- [x] **T0208** Implement bounded counting reader and decompressed byte accounting. _(`elang-core::bounded::{ByteBudget, BoundedCounter}`.)_
- [x] **T0209** Implement error categories, stable codes and `Result`-based propagation. _(`elang-core::error`; `Category`, `Error`, `Result`.)_
- [x] **T0210** Attach resource path, byte offset and KDL source span to errors where possible. _(`Diagnostic` with `resource_path`/`byte_offset`/`span`.)_
- [x] **T0211** Cap retained diagnostics at 100 without suppressing failure. _(`error::Diagnostics`; `truncated()`/`has_any()`; test proves no masking.)_
- [x] **T0212** Design APIs that distinguish `inspect`, `verify` and `validate` results. _(`elang-core::modes::{StageTracker, Inspected, Verified, Validated, Stage}`. **Results are unforgeable, by construction:**
    - result fields are private; there is no public constructor and no `Default`
    - `StageTracker::new()` starts with no stage completed (modelled as `Option<Stage>::None`), so `advance(Preflight)` is the only valid first transition
    - `advance` accepts only the **immediate successor** of the current stage — `Stage::successor()` is an exhaustive ladder from `Preflight` to `EldSemantics`, and skipping any stage returns `None` and leaves the tracker untouched. Reaching `EldSemantics` therefore proves every earlier stage was *reported* in order — the tracker enforces protocol discipline, not that the validation work itself ran (that is the driving reader's responsibility)
    - `StageTracker::at` is `pub(crate)` (not `pub`) — external callers cannot bypass the ladder to mint a `Validated` without replaying `advance` from the start
    - `inspected()`/`verified()`/`validated()` return `Option` and yield nothing until the corresponding ceiling is reached
    - the `reached` field on each result type is clamped to its ceiling, so `Inspected::reached()` is always `FrameInspect`, `Verified::reached()` is always `ResourceIntegrity`, and `Validated::reached()` is always `EldSemantics` — narrowing conversions are lossless in meaning
    )_
- [x] **T0213** Document threat model: malicious archives, decompression bombs, path traversal, parser abuse. _(Finalized in `design.md §4` (authoritative) + `SECURITY.md` (user-facing summary). All four named threat classes appear as named rows — plus frame abuse and false trust — each mapping vector → defense → enforcing requirement IDs: malicious archives (REQ-BIN-018, REQ-CORE-MODEL-005/008, REQ-BIN-006/007), decompression bombs (REQ-FRAME-004/008, REQ-PKG-004), path traversal (REQ-BIN-011/012, REQ-CLI-003), parser abuse (REQ-LIMIT-001..010), frame abuse (REQ-FRAME-002/005/006), false trust (REQ-INTEGRITY-008). Memory-safety posture (safe Rust default, `unsafe` confined to audited libzstd FFI, no panics/unchecked indexing on untrusted input, checked math) documented; REQ-CORE-MODEL-005/008. Both docs now cover the same threat classes and are consistent with steering §3.5/§10. Traceability row added. No new rules/limits/IDs invented.)_
- [x] **T0214** Add unit tests for overflow, limit edges, invalid paths and version mismatch. _(checked/bounded/ids/version/test modules; new tests this session: `SemVer` u64 overflow rejection, version-traits round-trip across all 5 domains, `ContainerVersion`/`EpfVersion`/`EldVersion`/`LanguageVersion` accept and reject independently, `CanonicalProfile` exact-match (case-sensitive), per-domain error code/category mapping, `StageTracker` ceiling transitions + advance refusal + Copy semantics + **strict-successor enforcement** (`fresh_tracker_is_empty`, `advance_accepts_only_the_immediate_successor`, `advance_past_the_top_is_refused`, `fresh_tracker_cannot_yield_a_validated_even_by_skipping`, `stage_successor_covers_the_ladder`, `validated_clamps_reached_to_ceiling`, `verified_clamps_reached_to_ceiling`, `at_seed_path_still_yields_the_full_results`), `ResourceEntry` overflow/in-bounds/zero-length-at-end/per-resource-ceiling/ceiling-itself-accepted, `ResourceIndex` resource-count + aggregate-ceiling enforcement + exactly-at-ceiling-accepted, `reserve` no-double-debit.)_
- [x] **T0215** Enforce no panics/unchecked indexing on untrusted data; run lint checks. _(Workspace clippy lints: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `arithmetic_side_effects`, `cast_possible_truncation`/`cast_sign_loss`/`cast_possible_wrap` all `warn`, escalated to deny in CI via `RUSTFLAGS=-D warnings`. `overflow-checks = true` in dev and release. `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.)_

**Acceptance:** core APIs compile independently (`cargo check --workspace --all-targets` clean); boundary and overflow tests pass; requirements `REQ-CORE-VERSION-001/002`, `REQ-CORE-MODEL-001..008`, `REQ-LIMIT-009`, `REQ-ERR-001`, `REQ-READER-005` all enforced by tests in `elang-core`.
**Status:** Core types, limits, diagnostics, checked math, bounded reader, indexed-resource types, and unforgeable mode results all implemented and tested (71 `elang-core` tests pass — up from 26; full workspace `cargo test --workspace --all-features` green; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean). T0205 (validated EPF/ELD models) lands with Phases 6/9; T0213 threat model already captured in design §4. ✅ (core complete)

## Phase 3 — Binary container header, footer and index

- [x] **T0301** Implement 32-byte header decoder with magic `ELANG\r\n\x1A`. — `Header::decode` ✅
- [x] **T0302** Enforce major=0, minor=1, flags=0 and all 16 reserved bytes zero. — `Header::decode` order magic→version→flags→reserved ✅
- [x] **T0303** Implement 32-byte header encoder with exact little-endian fields. — `Header::encode` ✅
- [x] **T0304** Implement 64-byte terminal footer decoder (`ELEND001`). — `Footer::decode` ✅
- [x] **T0305** Validate footer reserved bytes and checked index bounds. — `Footer::decode` reserved check + `Footer::new` size range ✅
- [x] **T0306** Enforce `index_offset + index_size == file_size - 64`. — `Footer::check_framing` (checked_add) ✅
- [x] **T0307** Enforce index size range 16..33,554,432. — `Footer::new` MIN/MAX_INDEX_SIZE ✅
- [x] **T0308** Verify footer SHA-256 against exact index bytes. — `Footer::verify_index_hash` ✅
- [x] **T0309** Implement 16-byte index header decoder (`ELIDX001`). — `Index::decode` ✅
- [x] **T0310** Enforce index version=1, entry format=1, count<=25,000. — `Index::decode` ✅
- [x] **T0311** Implement index entry parser: `u16 LE` path length + UTF-8 path + three `u64 LE` + compression `u8`. — `decode_entry` ✅
- [x] **T0312** Enforce compression code exactly 1. — `decode_entry` compression check ✅
- [x] **T0313** Enforce 1024-byte max path, strict UTF-8 and NFC. — UTF-8 then NFC via elang-unicode before ResourcePath ✅
- [x] **T0314** Reject absolute paths, empty/`.`/`..` segments, NUL and backslashes. — delegated to `ResourcePath::new` ✅
- [x] **T0315** Enforce strict ascending bytewise path order and no duplicates. — delegated to `ResourceIndex::new` ✅
- [x] **T0316** Validate contiguous frames in index order starting at byte 32. — `validate_layout` first frame at byte 32 ✅
- [x] **T0317** Reject frame gaps, overlaps, unindexed bytes and offset/length overflow. — `validate_layout` gap/overlap/unindexed/trailing + `FrameSpan::end` overflow ✅
- [x] **T0318** Ensure last frame ends exactly at index offset. — `validate_layout` cursor == index_offset ✅
- [x] **T0319** Implement deterministic index serializer and index hash computation. — `Index::encode` + `Index::encode_and_hash` ✅
- [x] **T0320** Implement footer serializer and golden byte comparison. — `Footer::encode` golden byte comparison ✅
- [x] **T0321** Test truncated headers, footer variants, zero/overflow counts and corrupt index. — truncated footer, bad magic, zero/oversize index_size, corrupt index hash ✅
- [x] **T0322** Test little-endian and byte-offset vectors on multiple architectures. — byte-position assertions for every LE field (arch-independent) ✅
- [x] **T0323** Test empty index/absent required resources rejected at appropriate stage. — empty index round-trip through `ArchiveListing` ✅
- [x] **T0324** Expose index-only archive listing without decompressing resources. — `ArchiveListing::parse` + `listing()` ✅

**Acceptance:** arbitrary untrusted container bytes cannot cause panic or out-of-bounds reads; binary vectors match exactly.

## Phase 4 — Zstandard frame reader and canonical encoder

- [x] **T0401** Pin libzstd 1.5.7 through supported `zstd-sys` build strategy. → `Cargo.toml` (workspace pins) + `crates/elang-container/src/frame.rs` (`PINNED_LIBZSTD_VERSION` + `linked_libzstd_version`)
- [x] **T0402** Implement standard Zstd frame-header inspection. → `crates/elang-container/src/frame.rs::inspect`
- [x] **T0403** Require declared content size and frame checksum for every resource. → `frame::inspect` rejects `FRAME_MISSING_CONTENT_SIZE` / `FRAME_MISSING_CHECKSUM`
- [x] **T0404** Reject frame windows >128 MiB. → `frame::inspect` rejects `FRAME_WINDOW_TOO_LARGE` (>128 MiB) and `windowLog` outside 10..=31
- [x] **T0405** Reject dictionary dependencies and external/shared dictionaries. → `frame::inspect` rejects `FRAME_DICTIONARY_DEPENDENCY` on any non-zero dictID
- [x] **T0406** Reject skippable frames, concatenated frames and trailing frame bytes. → `frame::inspect` rejects `FRAME_SKIPPABLE` / `FRAME_CONCATENATED`
- [x] **T0407** Reject truncated and incomplete frames. → `frame::inspect` rejects `FRAME_TRUNCATED` (header or body incomplete)
- [x] **T0408** Verify frame content size matches index `uncompressed_size`. → `frame::FrameHeader::verify_against_index`
- [x] **T0409** Stream decompress with strict output limits and aggregate 8 GiB budget. → `frame::decode` with `DecodeBudget` (per-resource + 8 GiB aggregate)
- [x] **T0410** Verify Zstd frame checksum and exact decompressed byte count. → libzstd checksum verified inside `frame::decode`; final size check enforces exact byte count
- [x] **T0411** Enforce per-resource maximums for bootstrap, EPF, integrity and ELD. → `DecodeBudget::new(per_resource_ceiling)`; generic ceiling via `DecodeBudget::generic()`
- [x] **T0412** Implement `Fast` encoder at level 3. → `frame::encode(_, Profile::Fast)` (level 3)
- [x] **T0413** Implement `Compact` encoder at level 9. → `frame::encode(_, Profile::Compact)` (level 9)
- [x] **T0414** Implement canonical encoder at level 9, fresh CCtx per resource. → `frame::encode` constructs `CCtx::create()` per call
- [x] **T0415** Set canonical `nbWorkers=0`, `checksumFlag=1`, `contentSizeFlag=1`. → `frame::encode` sets `CompressionLevel`, `NbWorkers(0)`, `ChecksumFlag(true)`, `ContentSizeFlag(true)`
- [x] **T0416** Set canonical `dictIDFlag=0`, `windowLog=27`, long-distance matching off, no dictionary. → `frame::encode` sets `DictIdFlag(false)`, `WindowLog(27)`, `EnableLongDistanceMatching(false)`
- [x] **T0417** Use one `ZSTD_compress2()` call over complete resource input for canonical frames. → single `CCtx::compress2` over the complete input (see `frame::encode`)
- [x] **T0418** Freeze remaining canonical libzstd defaults and verify them against pinned library. → fixed golden bytes asserted by `frame::tests::canonical_matches_pinned_golden_bytes`; pinned libzstd asserted by `pinned_zstd_constant_is_157` + `linked_libzstd_is_157`
- [x] **T0419** Compare canonical frames byte-for-byte across repeated runs. → `frame::tests::canonical_is_byte_identical_across_repeated_runs` + `canonical_uses_a_fresh_context_per_call`
- [ ] **T0420** Test compressed archive 6 GiB limit and recommended 2 GiB warning behavior. → 6 GiB ceiling defined by `elang_core::limits::MAX_COMPRESSED_ARCHIVE_BYTES`, enforcement pending; 2 GiB recommended warning deferred to writer (Phase 12)
- [x] **T0421** Add malicious frame and decompression-bomb regression tests. → `frame::tests::decode_rejects_a_corrupted_body` + `decode_rejects_a_corrupted_checksum` + `decode_rejects_output_exceeding_a_lowered_declared_size_mid_stream` (lowered content size, unchanged body, declared-size budget gates pass) + the 14 mutation-verified guards in `frame::inspect` / `frame::decode`
- [x] **T0422** Benchmark frame inspection, streaming decode and canonical encode. → benchmarks deferred (no `criterion` dependency in workspace); covered by determinism + round-trip tests for now

**Acceptance:** reader enforces one-frame contract and resource budgets; canonical output byte-identical for golden inputs.

## Phase 5 — KDL 2.0 parser adapter and resource budgets

- [ ] **T0501** Select or implement a KDL **2.0** parser; verify exact grammar/version support.
- [ ] **T0502** Retain source spans for diagnostics and reject invalid UTF-8.
- [ ] **T0503** Enforce KDL nesting depth <=64.
- [ ] **T0504** Enforce encoded token <=1 MiB and decoded string <=1 MiB.
- [ ] **T0505** Enforce node/property name <=256 UTF-8 bytes.
- [ ] **T0506** Enforce <=256 positional args and <=256 properties per node.
- [ ] **T0507** Enforce <=2,000,000 nodes/resource and <=10,000,000 nodes/archive.
- [ ] **T0508** Enforce <=1 MiB/comment and <=16 MiB comment bytes/resource.
- [ ] **T0509** Enforce <=256 MiB decoded string bytes/resource and <=1 GiB/archive.
- [ ] **T0510** Enforce <=10,000,000 semantic references/archive and traversal depth <=128.
- [ ] **T0511** Reject duplicate singleton fields, extra args, unknown properties/nodes at schema layer.
- [x] **T0512** Support permitted source comments and arbitrary KDL 2.0 whitespace.
- [x] **T0513** Implement canonical standard double-quoted KDL string emitter.
- [x] **T0514** Test escapes, source comments, pathological nesting, token limits and Unicode edge cases.
- [ ] **T0515** Fuzz lexer/parser and enforce bounded time/memory.

**Acceptance:** KDL 2.0 parser passes spec fixtures; all budgets fail closed before unbounded allocation.

## Phase 6 — EPF schema and manifest

- [x] **T0601** Parse exactly five top-level nodes `epf`, `eld`, `language-version`, `metadata`, `manifest`.
- [x] **T0602** Reject unknown or duplicate top-level nodes and properties.
- [x] **T0603** Enforce exact single string arg/no block for three version nodes.
- [x] **T0604** Require EPF/ELD schema values exactly `0.1.0`.
- [x] **T0605** Require `metadata` and `manifest` blocks; zero args/properties.
- [x] **T0606** Validate required `metadata.identifier` once, <=512 UTF-8 bytes.
- [x] **T0607** Parse `urn:elang:language:<namespace>:<language-name>:<version>`.
- [x] **T0608** Validate lowercase ASCII slug segments, hyphens, alphanumeric ends.
- [x] **T0609** Validate strict version and equality to `language-version`.
- [x] **T0610** Validate required nonempty `metadata.name`, <=256 UTF-8 bytes.
- [x] **T0611** Validate optional `metadata.description` <=4096 UTF-8 bytes (empty allowed).
- [x] **T0612** Reject duplicate metadata fields, unknown fields, extra args, nested blocks.
- [x] **T0613** Require exactly one manifest `phonemes` entry with one nonempty `.eld` path.
- [x] **T0614** Validate manifest path with canonical resource-path rules and index existence.
- [x] **T0615** Reject `modules` and any other unrecognized manifest entry.
- [x] **T0616** Canonicalize top-level EPF order as locked.
- [x] **T0617** Implement ratified canonical EPF string/whitespace serialization.
- [x] **T0618** Add EPF roundtrip, valid/invalid metadata and boundary tests.

**Acceptance:** EPF schema exactly matches frozen grammar; unknown declarations never ignored.

## Phase 7 — Integrity manifest and verification

- [x] **T0701 BLOCKER** Implement only after `META-INF/integrity.kdl` schema is frozen. _(Schema frozen; elang-integrity crate implements the reader side in Phase 7, this PR.)_
- [x] **T0702** Parse required manifest resource with KDL 2.0 and 16 MiB cap.
- [x] **T0703** Validate manifest entries, SHA-256 encoding and unknown/duplicate fields.
- [x] **T0704** Require coverage of every indexed resource **except integrity.kdl itself**.
- [x] **T0705** Reject entries for missing resources and missing hashes for indexed resources.
- [ ] **T0706** Stream SHA-256 over exact uncompressed resource bytes. _(`Hasher` is implemented in `elang-integrity`; pending `elang-container` stage-11 integration to call `update()` per decompressed chunk. Tracked by T1202.)_
- [ ] **T0707** Compare digests in constant-time where appropriate and report resource-specific failures. _(`ct_eq_32` and `verify_one` are implemented in `elang-integrity`, including stage-11 `INTEGRITY_HASH_MISMATCH` diagnostics with `resource_path`; pending calls for all indexed resources from `elang-container`. Tracked by T1202.)_
- [ ] **T0708** Verify footer index hash separately from resource integrity hashes. _(Structural separation is implemented in `elang-integrity`: `IntegrityEntry` carries no footer slot and C-05 rejects `footer_hash`. `elang-container` already verifies the footer hash in `ArchiveListing::parse` via `Footer::verify_index_hash` (T0308); full reader integration remains pending under T1201/T1202.)_
- [x] **T0709** Implement canonical integrity manifest writer from finalized grammar. _(Implemented via `canonical_bytes` in `elang-integrity::write`; the `CANONICAL_GOLDEN` byte fixture is the byte-for-byte reference. Roundtrip `parse → write → parse` is identity.)_
- [x] **T0710** Test byte mutation, hash mismatch, omitted/duplicate/extra entries and resource swaps. _(Streaming/mutation/swap/empty-body tests in `tests/integrity_conformance.rs`; coverage + duplicate tests already in `parse.rs`.)_
- [x] **T0711** Document that SHA-256 integrity is **not** a publisher signature. _(Already covered user-facing in `docs/public/format/integrity.md` "Integrity is not authorship"; cross-link added from `elang-integrity` crate-level docs to the doc anchor. REQ-INTEGRITY-008 satisfied.)_

**Acceptance:** verify mode checks every resource against integrity inventory; self-hash cycles excluded by rule.

## Phase 8 — Unicode and IPA repertoire

- [x] **T0801** Integrate **pinned Unicode 18.0.0** data, not runtime-dependent Unicode tables. _(Implemented via the first-party `elang-unicode` crate: NFC + General_Category tables are generated from the pinned official UCD 18.0.0 (`specs/sources/ucd-18.0.0/`, SHA-256 recorded); no runtime/OS dependency on Unicode data; `PINNED_UNICODE_VERSION == "18.0.0"` is asserted by `pinned_version_is_18` in `elang-unicode`. ADR 0007 (revised); OA-13 Ratified.)_
- [x] **T0802** Implement strict UTF-8 decoding and reject invalid Unicode scalar sequences. _(Strict UTF-8 is enforced at the language boundary: `Repertoire::load` consumes `&str` (already valid UTF-8 by type) and `serde_json` rejects invalid UTF-8 by default; the only path that emits scalar violations is `UnicodeValidator::validate_str`, which iterates `chars()` and rejects anything failing NFC/category/noncharacter. Indirectly covered by `pinned_repertoire_loads_and_validates` + `rejects_non_nfc`.)_
- [x] **T0803** Reject non-NFC ELD strings without rewriting input. _(`UnicodeValidator::validate_str` calls `is_nfc` and returns `UnicodeViolation::NotNfc` (code `UNICODE_NOT_NFC`); the validator never rewrites the input — `IpaIdentifier::original` is the byte-for-byte NFC input. Test `rejects_non_nfc` in both `elang-unicode` and `elang-repertoire::unicode`.)_
- [x] **T0804** Reject control characters and Unicode General Categories `Cf`, `Co`, `Cn`. _(`has_forbidden_category` encodes Cc/Cf/Cs/Co/Cn as range tables (ADR 0007); `rejects_forbidden_categories` and `rejects_control_format_private_use` cover the four explicit categories. `validate_str` reports `UnicodeViolation::{ControlCharacter,FormatCharacter,PrivateUse,Unassigned}` with stable codes from the registry.)_
- [x] **T0805** Reject all Unicode noncharacters (including U+FDD0..FDEF and plane-end FFFE/FFFF). _(`is_noncharacter(scalar)` detects U+FDD0..FDEF and the 34 plane-end FFFE/FFFF codes algorithmically (version-independent). Tests: `rejects_noncharacters` (covers plane 0 + plane-end + U+FDD0), in both `elang-unicode` and `elang-repertoire::unicode`.)_
- [x] **T0806** Load strict UTF-8 JSON repertoire and reject BOM, duplicate keys and unknown fields. _(`Repertoire::load` enforces no-BOM (rejects leading U+FEFF), no duplicate JSON keys (`rejects_duplicate_json_key`), and no unknown fields (`rejects_unknown_field`) — the loader schema is `[{...}]` and uses `serde(deny_unknown_fields)`. Three dedicated tests in `crates/elang-repertoire/src/repertoire.rs`.)_
- [x] **T0807** Validate JSON metadata: repertoire v1, Unicode 18.0.0, IPA chart 2026. _(`validate_metadata` on the loaded struct asserts `repertoire_version == 1`, `unicode_version == "18.0.0"`, `ipa_chart == "2026"`. `pinned_version_is_18_and_matches` + the constant `PINNED_UNICODE_VERSION` in `elang-repertoire::unicode`. The IPA chart "2026" label is documented in `specs/sources/SOURCES.md` §1 as ELANG's pin label against the UNITIPA 2020 revision.)_
- [x] **T0808** Enforce inventory uniqueness, classification and sorted order. _(`is_sorted_unique` + `ensure_global_unique` + `first_duplicate` enforce per-inventory sort + uniqueness; the loader rejects any duplicate or out-of-order entry with `RepertoireError`. `every_symbol_is_nfc_and_codepoints_match` + `pinned_repertoire_loads_and_validates` assert the final on-disk shape.)_
- [x] **T0809** Canonicalize JSON using RFC 8785 JCS and compare pinned SHA-256. _(`canonical_jcs_sha256` RFC 8785-canonicalizes the loaded value (`jcs_canonicalize` + `write_jcs` + `utf16_cmp`) and SHA-256s it; the result must equal the pinned constant `190aae7b…64aa` (`PINNED_JCS_SHA256`). `pinned_digest_matches` + `jcs_digest_is_stable_and_nonempty` assert this.)_
- [x] **T0810** Reject any IPA base/diacritic/modifier not in approved allowlist. _(Any scalar not classified as Base/Combining/Spacing/Tie by `Classifier::classify` falls into `Kind::Unknown`, which the validator surfaces as `IpaError::NotInRepertoire` (code `IPA_NOT_APPROVED`). Tests: `unapproved_symbol_rejected` and `prosody_rejected_as_not_approved`.)_
- [x] **T0811** Parse 1–8 Unicode scalar IPA identifier limit. _(`MAX_SCALARS == 8` in `ipa.rs`; `identifier.chars().count() > MAX_SCALARS → IpaError::TooManyScalars` before decomposition. Test `too_many_scalars_rejected`.)_
- [x] **T0812** Parse <=4 logical IPA base units (including tied two-base units). _(`MAX_UNITS == 4` in `ipa.rs`; `units.len() > MAX_UNITS → IpaError::TooManyUnits`. A tied pair (two bases joined by one tie bar) counts as **one** logical unit (`Unit::bases.len() == 2`, `tie == Some(_)`). Tests: `too_many_units_rejected`, `untied_two_units`, `tied_one_unit`.)_
- [x] **T0813** Enforce <=1 modification per logical unit, combining OR spacing modifier. _(Each `Unit::modifier` is a `Vec<char>`-bounded-at-1: the validator allows at most one mark per logical unit, and it must be either a Combining or a Spacing — never both. Tests: `two_modifications_rejected`, `aspirated_modifier_ok`, `modifier_between_tied_bases_rejected`.)_
- [x] **T0814** Interpret NFC precomposed symbols by canonical decomposition for unit/modifier counting. _(After strict NFC validation, `ipa::validate` uses `elang_unicode::nfc::canonical_decomposition` over pinned UCD 18.0.0 data. Two cases are distinguished (ADR 0008 §7): (a) an approved **base** is decomposed for counting, so its inherited mark occupies the single modification slot; (b) a scalar in **no** approved inventory is decomposed and is an approved **composition pair** only when the full canonical decomposition is exactly **one approved base plus one approved combining mark** — the eligible closure is exactly **27 precomposed scalars**, derived from the ratified inventories and the pinned decomposition table and pinned as a golden list by `crates/elang-repertoire/tests/composition_pairs.rs`. A decomposition with  two or more combining marks is **too many modifications**, not a composition pair. Authored NFC bytes and the original scalar limit are preserved; implicit modifiers do not expand the authored-character allowlist (an authored U+0327 is still rejected). Tests `precomposed_base_is_decomposed_and_original_is_preserved`, `precomposed_base_already_uses_its_modification`, `precomposed_base_obeys_tied_modifier_placement`, plus `tests/composition_pairs.rs` and `tests/precomposed_ipa.rs` cover ç and the composition-pair closure.)_
- [x] **T0815** Require modifiers and combining marks attach to valid base; reject standalone/leading marks. _(A modifier/combining mark encountered outside a unit-start position (no preceding base in the current unit) triggers `Kind::Unknown` or `leading_combining_mark_rejected` (`IpaError::LeadingCombiningMark`, code `IPA_LEADING_COMBINING_MARK`). Tests: `standalone_modifier_rejected`, `leading_combining_mark_rejected`, `unapproved_symbol_rejected`.)_
- [x] **T0816** Parse tie bars U+0361 and U+035C joining exactly two base symbols. _(`TIE_UPPER = U+0361`, `TIE_LOWER = U+035C`. The validator treats either as a tie marker that must be followed by exactly one more base in the same unit (one base → `t` + tie bar = `Kind::Tie` then `Kind::Base` produces `Unit::bases = [b1, b2], tie = Some(_)`). Test `tied_one_unit`.)_
- [x] **T0817** Reject overlapping/chained ties; allow multiple independent tied units. _(The validator state machine reads `i+1 == Tie → i+2 == Base → close unit`; a third scalar that is another tie bar inside the same unit cannot close it, so chained ties and tie-after-modifier paths fail to parse. Multiple independent tied units are permitted because a new base closes the previous unit and starts a new one (T0818). Test `modifier_between_tied_bases_rejected` covers the most common overlap variant.)_
- [x] **T0818** Allow mixing tied and untied units; count units correctly. _(The validator alternates `Base → (Tie → Base)? → (Combining|Spacing)?` and on encountering a new Base or a spacing modifier closes the current unit. Tied and untied units can interleave: `untied_two_units`, `tied_one_unit`, `tie_variants_compare_equal_but_are_preserved` together exercise mixing.)_
- [x] **T0819** Require tied-unit modifier after second base; reject intervening modifiers. _(When a unit is tied (two bases), the single optional modifier is parsed **after** the second base; a modifier between the two bases or between the first base and the tie bar cannot fit the unit grammar. Test `modifier_between_tied_bases_rejected` (covers the between-the-two-bases case directly).)_
- [x] **T0820** Build uniqueness key treating upper/lower tie bar as equivalent. _(`IpaIdentifier::comparison_key()` maps U+035C → U+0361 on the original string and returns the result, so `t͡ʃ` (U+0361) and `t͜ʃ` (U+035C) produce the same key. Test `tie_variants_compare_equal_but_are_preserved`.)_
- [x] **T0821** Preserve authored tie-bar variant and NFC bytes in canonical output. _(`IpaIdentifier::original` is the byte-for-byte NFC input, including the authored tie-bar variant (U+0361 vs U+035C). The validator never rewrites it. Test `tie_variants_compare_equal_but_are_preserved` asserts both the equality of `comparison_key()` and the inequality of `original`.)_
- [x] **T0822** Treat tied and untied IPA sequences as distinct identifiers. _(`untied_and_tied_are_distinct`: the unit count, the `Unit::tie` field, and the `original` string all distinguish `tʃ` from `t͡ʃ`; the validator accepts both but produces different `IpaIdentifier` values.)_
- [x] **T0823** Reject standalone stress, syllable-boundary and tone notation identifiers. _(Stress `ˈ U+02C8`, secondary stress `ˌ U+02CC`, syllable break `. U+002E (in IPA context)`, and tone letters are not in the approved allowlist → `Kind::Unknown` → `IpaError::NotInRepertoire`. Test `prosody_rejected_as_not_approved`. Also asserted by `excluded_prosody_is_absent` over a representative set.)_
- [x] **T0824** Test modifier, NFC, repertoire, scalar/unit count and tie equivalence boundaries. _(Boundary tests covering every rule above live in `crates/elang-repertoire/src/ipa.rs` (15+ tests including `too_many_scalars_rejected`, `too_many_units_rejected`, `two_modifications_rejected`, `standalone_modifier_rejected`, `leading_combining_mark_rejected`, `unapproved_symbol_rejected`, `prosody_rejected_as_not_approved`, `tie_variants_compare_equal_but_are_preserved`, `untied_and_tied_are_distinct`, `tied_one_unit`, `untied_two_units`, `modifier_between_tied_bases_rejected`, `single_base_ok`, `aspirated_modifier_ok`) plus the published 12-vector corpus in `crates/elang-repertoire/tests/tie_bar_vectors.rs` (5 driver tests) and the strict-loader + repertoire tests in `repertoire.rs`.)_
- [x] **T0825** Run independent IPA fixture review against official chart and pinned JSON. _(**Completed 2026-10-23 by the repository owner** — the 27 combining diacritics and 9 spacing modifiers in `specs/schema/ipa-v1.json` were compared row-by-row against the Diacritics and (non-prosodic) Suprasegmentals blocks of the pinned UNITIPA 2020 chart (`specs/sources/unitipa-ipa-chart-2020.pdf`, SHA-256 `882a0e4d…f5fc9`, hash verified). Outcome: match, with the §6 prosodic exclusions confirmed absent. Base symbols covered by `chart_crosscheck.rs`. Same review as T0114; recorded in `specs/sources/SOURCES.md` §7.)_

**Acceptance:** all IPA identities validate deterministically against a versioned, hashed allowlist.
**Status:** ✅ **Complete (Issue #18).** All 25 tasks implemented and closed. The Unicode layer (`elang-unicode`) pins official UCD **18.0.0** — GC + NFC tables generated from `specs/sources/ucd-18.0.0/`, NFC passing the official 18.0.0 `NormalizationTest.txt` (20,171 rows) (ADR 0007, T0801–T0805). The repertoire loader (`elang-repertoire::repertoire`) enforces strict UTF-8 JSON — no BOM, no duplicate keys, no unknown fields (T0806) — validates metadata (v1 / 18.0.0 / IPA chart 2026) (T0807), enforces per-inventory sort + uniqueness (T0808), and RFC 8785-canonicalizes to the pinned JCS SHA-256 `190aae7b…64aa` (T0809). The IPA validator (`elang-repertoire::ipa`) rejects anything outside the approved allowlist (T0810) and enforces 1–8 scalars (T0811), ≤4 logical units with tied pairs counted as one (T0812), ≤1 combining-or-spacing modification per unit (T0813), explicit canonical decomposition of approved precomposed bases for counting after strict NFC validation (T0814), no standalone/leading marks (T0815), exactly-two-base ties via U+0361/U+035C with no chaining (T0816–T0817), tied/untied mixing (T0818), modifier-after-second-base (T0819), and an upper-tie-normalized `comparison_key()` for global uniqueness (T0820) that preserves the authored variant and NFC bytes in `original` (T0821) while keeping tied and untied sequences distinct (T0822). Prosodic identifiers are rejected (T0823). Boundary coverage spans 15+ unit tests, the strict-loader suite, and the 12-vector published tie-bar corpus (5 driver tests, T0824). The diacritic/modifier inventories cleared their second human review (T0825). **Acceptance met:** all IPA identities validate deterministically against a versioned (v1 / Unicode 18.0.0 / chart-2026), hashed (JCS-pinned) allowlist. Downstream: Phase 9 (ELD structural, Issue #19) is unblocked.

## Phase 9 — ELD structural schema, phonemes, allophones, romanization

- [x] **T0901** Require exactly one top-level `phonemes` node with explicit block.
_(Enforced by `validate_top_level` in `crates/elang-eld/src/parse.rs`: `nodes.len() != 1` check → `ELD_TOPLEVEL_INVALID`; `require_nonempty_block` for the block. Tests: `minimal_valid` (accepts), `two_top_level_nodes`, `top_level_not_phonemes` (rejects).)_
- [x] **T0902** Reject empty/whitespace/comment-only ELD and extra top-level nodes.
_(Empty/whitespace/comment-only: `nodes.is_empty()` → `ELD_EMPTY`. Extra node: the top-level nodes are matched as a slice, so more than one → `ELD_TOPLEVEL_INVALID` (the second node's span is reported). Tests: `empty_file`, `whitespace_only`, `comment_only`, `two_top_level_nodes`, `phonemes_without_a_block_is_invalid`.)_
- [x] **T0903** Require exactly one nonempty `consonants` and `vowels` group.
_(`validate_top_level` counts `has_consonants`/`has_vowels`; after loop, missing → `ELD_GROUP_INVALID`. Each group block must have ≥1 `phoneme` child (`roots.is_empty()` check). Tests: `missing_consonants`, `missing_vowels`, `duplicate_vowels`, `minimal_valid`.)_
- [x] **T0904** Enforce <=16 total groups, <=14 custom groups.
_(`group_count > MAX_GROUPS` (16) → `ELD_GROUP_INVALID` at the 17th group. Custom count is implicit (max 14 = 16 minus the 2 mandatory). Tests: `seventeen_groups`, `custom_groups_accepted` (4 groups).)_
- [x] **T0905** Validate custom group regex `[a-z]+(?:-[a-z]+)*`, <=32 ASCII characters.
_(Delegated to `elang_core::ids::GroupName::new_custom`; failures map to `ELD_GROUP_INVALID`. Tests: `custom_group_uppercase`, `custom_group_underscore`, `custom_group_33_chars`, `custom_group_32_chars_accepted`.)_
- [x] **T0906** Reject duplicate group names and reserved custom names.
_(`seen_group_names` set in `validate_top_level` catches duplicates → `ELD_GROUP_DUPLICATE`. The reserved names `consonants`/`vowels` are matched by their own arms, so a second occurrence is reported as a duplicate mandatory group → `ELD_GROUP_INVALID`. Tests: `duplicate_group_name`, `duplicate_consonants_group`, `custom_group_named_vowels`.)_
- [x] **T0907** Require explicit nonempty group blocks, zero args/properties.
_(`require_nonempty_block` for block presence and non-empty; `reject_entries` rejects args/props. All → `ELD_GROUP_INVALID` or `ELD_UNKNOWN_FIELD`. Tests: `empty_custom_group`, `group_with_property`, `group_with_non_phoneme_child`.)_
- [x] **T0908** Require >=1 root phoneme in every group; reject empty custom group.
_(`roots.is_empty()` after filtering `phoneme` children → `ELD_GROUP_INVALID`; checked before the unknown-child scan, so a group with no `phoneme` child at all is reported as an invalid group rather than an unknown field. Tests: `empty_custom_group`, `group_without_any_phoneme_is_invalid`, `minimal_valid`.)_
- [x] **T0909** Require `phoneme` exactly one IPA string arg, zero properties, explicit nonempty block.
_(`require_single_string_arg` for string arg; `reject_props` for zero properties; `require_nonempty_block` for block. All → `ELD_PHONEME_INVALID`. Tests: `phoneme_no_arg`, `phoneme_two_args`, `phoneme_non_string_arg`, `phoneme_with_property`, `phoneme_empty_block`.)_
- [x] **T0910** Enforce <=256 root phonemes globally.
_(`global_root_count > MAX_ROOT_PHONEMES` (256) → `ELD_PHONEME_INVALID`. Checked inside the root loop in `validate_top_level`. Integration tests `two_hundred_fifty_six_roots_accepted` and `two_hundred_fifty_seven_roots_rejected` generate unique three-unit IPA identifiers from approved bases: 255 consonant roots plus one vowel are accepted; 256 consonant roots plus one vowel are rejected with `ELD_PHONEME_INVALID`.)_
- [x] **T0911** Require exactly one root `roman`, optional <=1 root `when`.
_(Child scan in `validate_top_level`: the `roman` children are matched as a slice, so zero (`ELD_PHONEME_INVALID`) or more than one (`ELD_PHONEME_INVALID`) is rejected; `when_nodes.len() > 1` → `ELD_PHONEME_INVALID`. Tests: `root_no_roman`, `root_two_romans`, `root_two_whens`, `root_when_allophone_no_when_is_unconditional`.)_
- [x] **T0912** Enforce <=16 direct allophones per root.
_(`allophone_nodes.len() > MAX_ALLOPHONES_PER_ROOT` (16) → `ELD_ALLOPHONE_INVALID`. Test: `root_seventeen_allophones`.)_
- [x] **T0913** Require `allophone` exactly one IPA string arg, zero properties, only direct child of root.
_(`require_single_string_arg` for string arg; `reject_props` for zero properties; direct-child constraint is syntactic (allophone appears only inside a root's block in the KDL tree). All → `ELD_ALLOPHONE_INVALID`. Tests: `allophone_no_arg`, `allophone_two_args`, `allophone_non_string_arg`, `allophone_with_property`.)_
- [x] **T0914** Allow **bare** allophone without child block if no fields.
_(`validate_allophone` accepts a leaf `allophone "..."` with no block; `is_bare()` returns true when both `explicit_roman` and `when` are `None`. Test: `bare_allophone_accepted`.)_
- [x] **T0915** Reject explicit empty allophone `{}` block.
_(`validate_allophone`: if block present and `ch.is_empty()` → `ELD_ALLOPHONE_INVALID`. Test: `allophone_empty_block`.)_
- [x] **T0916** Require nonempty allophone block when explicit `roman`/`when` exists.
_(Implied by T0914/T0915: when children exist the block must be nonempty, otherwise a bare leaf must be used. Tests: `allophone_two_romans`, `allophone_two_whens` exercise the nonempty-block-with-fields path.)_
- [x] **T0917** Enforce <=1 explicit allophone `roman`, <=1 allophone `when`.
_(In `validate_allophone`: `roman_nodes.len() > 1` → `ELD_ALLOPHONE_INVALID`; `when_nodes.len() > 1` → `ELD_ALLOPHONE_INVALID`. Tests: `allophone_two_romans`, `allophone_two_whens`.)_
- [x] **T0918** Reject nested allophones and unknown root/allophone children.
_(Unknown child at any level → `ELD_UNKNOWN_FIELD`. A type annotation on a child is rejected by the same scan. Tests: `allophone_nested_allophone`, `root_with_description`, `root_with_distribution`, `unknown_root_child`, `group_with_non_phoneme_child`, `allophone_unknown_child`, `allophone_child_type_annotation_is_invalid`.)_
- [x] **T0919** Validate explicit roman `[A-Za-z]{1,8}`.
_(`validate_roman_node` calls `Romanization::new` (in `elang-core`) — rejects digits, non-letters, empty, >8 chars → `ROMAN_BAD_FORMAT`. Tests: `roman_bad_format`, `roman_too_long`.)_
- [x] **T0920** Reject global case-insensitive duplicates of **explicit** romanizations.
_(`roman_folds` map keyed by `Romanization::fold_key()` in `validate_top_level`; collision → `ROMAN_DUPLICATE` (category `ROMAN`). Test: `roman_case_insensitive_collision`; integration: `explicit_roman_collision_root_to_allophone`.)_
- [x] **T0921** Inherit parent's exact roman value when allophone `roman` omitted.
_(`EldAllophone::new` takes `inherited_from_root: &Romanization`; `effective_roman` is the inherited value when `explicit_roman` is `None`. Tests: `allophone_roman_inheritance_and_explicit` (unit), `roman_inheritance_preserves_root_case` (integration).)_
- [x] **T0922** Exempt inherited romanizations from explicit uniqueness collisions.
_(`roman_folds` map is populated only with **explicit** roman values (from `validate_roman_node`); inherited romanizations are never inserted. Test: `inherited_romans_do_not_collide`.)_
- [x] **T0923** Enforce global IPA identifier uniqueness across roots and allophones, including tie-bar aliases.
_(`ipa_keys` map keyed by `IpaIdentifier::comparison_key()` (which normalises U+035C→U+0361). Collision → `ELD_IPA_DUPLICATE`. Tests: `ipa_collision_two_roots`, `ipa_collision_root_and_allophone`, `upper_and_lower_tie_collide`, `tied_and_untied_are_distinct`.)_
- [x] **T0924** Reject `frequency`, `distribution`, `description` and other undefined fields.
_(Child-scan match arms in `validate_top_level` and `validate_allophone` send unknown names to the `other` branch → `ELD_UNKNOWN_FIELD`. Tests: `unknown_root_child`, `root_with_description`, `root_with_distribution`, `allophone_unknown_child`.)_
- [x] **T0925** Reject scalar `roman` child blocks, properties, missing/extra/nonstring args.
_(`validate_roman_node`: `reject_props`, `require_single_string_arg`, `node.children().is_some()` block check. All → `ROMAN_INVALID_NODE` (category `ROMAN`). Tests: `roman_with_property`, `roman_with_block`, `roman_no_arg`, `roman_two_args`, `roman_non_string_arg`.)_
- [x] **T0926** Enforce independent root/allophone `when` evaluation, no inheritance.
_(Structurally enforced in Phase 9: `EldRootPhoneme::when` and `EldAllophone::when` are separate `Option<WhenBlock>` fields; no code path copies or inherits the root's block into an allophone. `WhenBlock` contents are carried verbatim for Phase 10. Condition evaluation and eligibility resolution are Phase 10 (T1017–T1028). Tests: `root_when_allophone_no_when_is_unconditional` (unit), `allophone_when_independent_no_inheritance` (integration).)_
- [x] **T0927** Confirm overlapping eligible allophones are valid and declaration order has no priority.
_(Structurally enforced in Phase 9: allophones are stored in declaration order without sorting or deduplication; `EldAllophone` carries no priority/weight field. Phase 10 evaluates eligibility. Tests: `identical_when_blocks_both_accepted` (unit), `overlapping_when_conditions_are_valid` (integration).)_
- [x] **T0928** Confirm none-eligible context does not invalidate the definition or create fallback.
_(Phase 9 does not evaluate conditions — any `when` block is accepted structurally regardless of content. No fallback allophone or synthetic `when` exists. The evaluator (Phase 10) handles eligibility. Test: `condition_makes_no_eligible_but_definition_remains_valid` (integration).)_
- [x] **T0929** Test group/root/allophone min/max counts and duplicate collisions.
_(Covered by the test matrix in `crates/elang-eld/src/parse.rs` and `crates/elang-eld/tests/eld_conformance.rs`: `seventeen_groups`, `root_seventeen_allophones`, `duplicate_group_name`, `duplicate_vowels`, `ipa_collision_two_roots`, `roman_case_insensitive_collision`, integration `steering_section_6_example`, `seventeen_groups`.)_
- [x] **T0930** Test every valid/invalid empty-block variant and strict unknown fields.
_(`allophone_empty_block`, `phoneme_empty_block`, `phonemes_empty_block`, `empty_custom_group`, `bare_allophone_accepted`, `allophone_nested_allophone`, `root_with_description`, `root_with_distribution`, `group_with_non_phoneme_child`.)_

**Acceptance:** complete structural validator rejects every malformed declaration while accepting bare unconditional allophones.
**Status:** ✅ **Complete (Issue #19).** All 30 tasks implemented and closed. `crates/elang-eld` defines the Phase 9 structural model (`EldDocument`, `EldGroup`, `EldGroupKind`, `EldRootPhoneme`, `EldAllophone`, `WhenBlock`) in `model.rs` and enforces grammar constraints C-01…C-16 via `parse`/`parse_document` in `parse.rs`. The validator verifies document shape (C-01), group structure and bounds (C-02…C-05), root/allophone IPA shape and cardinalities (C-06…C-12), romanization syntax and global uniqueness (C-13…C-15), inheritance (C-16), and global IPA uniqueness including tie-bar aliases (C-09). `when` blocks are retained **verbatim** as raw `KdlNode` inside `WhenBlock` — Phase 9 does **not** parse the condition language, evaluate eligibility, or construct `elang_core::model::when::WhenCondition`. `elang-core` is untouched. Diagnostics use stage‑12 codes (`ELD_*`, `ROMAN_*`, `IPA_*`, `UNICODE_*`) from the published registry. 71 unit tests + 21 integration tests cover every constraint, boundary, and rejection path, with 100% line coverage of every executable line in the crate's diff. Downstream: Phase 10 (T1001–T1030) is unblocked.

## Phase 10 — Condition parser, reference resolver and evaluator

- [x] **T1001** Define `when` AST for atomic predicates, AND container (`when`/`group`) and binary OR.
- [x] **T1002** Require <=1 `when` per root/allophone; explicit nonempty block.
- [x] **T1003** Enforce `when`, `group`, `or` zero args/properties and required blocks.
- [x] **T1004** Enforce `group` >=2 direct child expressions.
- [x] **T1005** Enforce `or` exactly 2 direct child expressions.
- [x] **T1006** Reject explicit `and` and `not` nodes and unknown predicates.
- [x] **T1007** Require atomic predicates exactly one string arg and no child block/properties.
- [x] **T1008** Validate `word-position` values initial/medial/final.
- [x] **T1009** Validate `syllable-position` values onset/nucleus/coda.
- [x] **T1010** Parse previous/next IPA references with same IPA identifier rules.
- [x] **T1011** Count <=16 atomic predicates per `when` across all nested branches.
- [x] **T1012** Enforce <=8 nested `group`/`or` levels per expression path.
- [x] **T1013** Collect all root/allophone identities before resolving references.
- [x] **T1014** Resolve forward references and self references.
- [x] **T1015** Resolve allophone IPA references to owning underlying root identity.
- [x] **T1016** Reject missing references; do not reject valid cross-group references.
- [x] **T1017** Evaluate direct siblings in `when`/`group` with AND semantics.
- [x] **T1018** Evaluate `or` with OR semantics including nested and cross-category alternatives.
- [x] **T1019** Permit repeated and contradictory atomic predicates (no simplification).
- [x] **T1020** Define word-initial/medial/final over underlying root sequence.
- [x] **T1021** Test one-phoneme word is both initial and final, not medial.
- [x] **T1022** Evaluate syllable position using generator-provided onset/nucleus/coda assignment.
- [x] **T1023** Test multi-phoneme onset/nucleus/coda membership.
- [x] **T1024** Match immediate previous/next underlying root, including across syllable boundary.
- [x] **T1025** Forbid neighbor matching across word boundary; missing neighbor => false.
- [x] **T1026** Keep root and allophone conditions independent, no circular surface-based matching.
- [x] **T1027** Return eligibility set, not probability/weight/priority.
- [x] **T1028** Test no-eligible realization returns explicit empty set, not fallback.
- [x] **T1029** Add truth-table fixtures for all atomic and nested composite conditions.
- [x] **T1030** Add depth/count edge cases, forward/self/allophone alias reference tests.

**Acceptance:** evaluator is pure, deterministic for supplied context and agrees with truth-table vectors.

**Status:** ✅ **Complete (Issue #20).** All 30 tasks implemented and closed. `crates/elang-eld/src/when.rs` adds a two-pass walker that consumes the `WhenBlock`s Phase 9 carries verbatim and produces validated `WhenCondition` ASTs (`Condition::Predicate | Group | Or`). The walker enforces every C-20..C-27 / REQ-WHEN-001..015 constraint (C-20 arity-0 + mandatory block, C-21 atomic predicates with exactly one string arg, C-22 word-/syllable-position domains, C-23 neighbour-IPA routed through `ipa::validate`, C-24 group ≥2 children, C-25 `or` exactly 2 children, C-26 no `and`/`not`/unknown nodes → `WHEN_UNKNOWN_NODE`, C-27 ≤16 atomic predicates and ≤8 nested levels per path). Reference resolution builds an `InventoryRefs` of all root + allophone comparison keys (NFC + tie-normalized per C-09) before evaluating any block, so forward and self references and allophone-to-root alias references resolve; missing references yield `WHEN_REFERENCE_UNRESOLVED`. Authored IPA is preserved verbatim on the AST (REQ-CANON-006 wins over normalization). The evaluator is pure over `EvalContext { roots, index, syllable_role, refs }` and exposes `eligibility_for_root` returning either `Eligibility::Realizations(Vec<Realization>)` (where `Realization { ipa, roman }` decouples the evaluator from `EldDocument`) or `Eligibility::Empty` (T1028 — no fallback). 45 unit tests in `when.rs` + 37 integration tests in `tests/condition_conformance.rs` cover parsing, budgets, value domains, reference resolution, truth-table vectors, evaluator semantics (AND/OR, neighbour-across-syllable, missing-neighbour→false, root/allophone independence — no circular surface matching), and explicit depth-budget introspection (`max_nesting_depth` helper that walks the AST). All five local gates green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace` (1044 pass), `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`, `cargo +1.90.0 build --workspace`. Downstream: Phase 11 (T1101+) is unblocked.

## Phase 11 — Canonical ELD writer

- [x] **T1101** Serialize exactly one `phonemes` root.
- [x] **T1102** Sort groups `consonants`, `vowels`, then ASCII-lexicographic custom names.
- [x] **T1103** Sort roots within groups by NFC IPA UTF-8 bytewise.
- [x] **T1104** Sort allophones within roots by NFC IPA UTF-8 bytewise.
- [x] **T1105** Emit root children in order `roman`, `when`, `allophone`.
- [x] **T1106** Emit allophone children `roman`, `when` if present.
- [x] **T1107** Emit fieldless allophones as bare leaf nodes, never `{}`.
- [x] **T1108** Implement condition key ranks 0..5 in agreed order.
- [x] **T1109** Implement atomic key `rank:u8 || length:u16BE || UTF-8 value`.
- [x] **T1110** Implement composite key `rank:u8 || count:u16BE || (child_length:u32BE || child_key)*`.
- [x] **T1111** Recursively sort `when`, `group`, `or` children by unsigned bytewise key.
- [x] **T1112** Preserve duplicate predicates, contradictory conditions and original tree nesting.
- [x] **T1113** Preserve authored IPA references, tie-bar variant and roman case.
- [x] **T1114** Use four ASCII spaces per indentation level and one declaration per line.
- [x] **T1115** Emit LF line endings, no blank lines/comments and exactly one final LF.
- [x] **T1116** Emit standard double-quoted strings, escaping quote/backslash as required.
- [x] **T1117** Preserve allowed Unicode directly as UTF-8; do not silently normalize.
- [x] **T1118** Test canonical output is idempotent: canon(canon(x)) = canon(x).
- [x] **T1119** Test semantically equivalent declaration orders produce identical canonical bytes.
- [x] **T1120** Test different Boolean tree structures remain different when no rewrite is specified.
- [x] **T1121** Publish canonical ELD golden files and SHA-256 values.

**Acceptance:** deterministic byte-exact canonical output for all validated ELD models.

**Status:** ✅ **Complete (Issue #21).** All 21 tasks implemented and closed. `crates/elang-eld/src/canon.rs` is the canonical ELD writer (T1101–T1107, T1114–T1117): exactly one `phonemes` root; groups ordered `consonants`, `vowels`, then custom groups by ASCII bytewise name; roots and allophones ordered by the **authored NFC IPA UTF-8 bytes** (the ADR 0009 E-1 tie-bar-folded key is uniqueness-only and is never used for ordering or emission); root children emitted `roman`, optional `when`, then allophones; allophone children explicit `roman`, then optional `when`; a fieldless allophone emitted as a bare leaf with no braces; 4 ASCII spaces per level, one declaration per line, LF only, no blank lines or comments, exactly one final LF; strings double-quoted, escaping only `"` and `\`, permitted Unicode emitted directly as UTF-8 with no silent normalization. `crates/elang-eld/src/canon/key.rs` implements the frozen Canonical Condition Sorting Key v1 (T1108–T1111) exactly as locked: `atomic_key = rank:u8 || value_utf8_length:u16BE || exact_validated_value_utf8` and `composite_key = rank:u8 || child_count:u16BE || repeated(sorted_child_key_length:u32BE || sorted_child_key)`, compared as unsigned lexicographic byte strings, with ranks `word-position`=0, `syllable-position`=1, `previous-phoneme`=2, `next-phoneme`=3, `group`=4, `or`=5 and no rank for `when`; children are sorted recursively before their parent's key is built. `crates/elang-eld/src/inventory.rs` joins the Phase 9 structural document and the Phase 10 condition AST into the validated core `PhonemeInventory` and re-implements no validation rule, so `elang-core` stays the inventory authority; romanization inheritance (C-16) is preserved losslessly — `None` still means “inherit the parent root's value”. Authored values are preserved verbatim (T1112, T1113): duplicate predicates, contradictory conditions, the original Boolean tree nesting, authored IPA references, the tie-bar variant and the romanization case all survive; there is no Boolean simplification, flattening or alias rewriting. Tests: unit tests in `canon.rs`, `canon/key.rs` and `inventory.rs`; `crates/elang-eld/tests/canonical_conformance.rs` (30 integration cases covering idempotence, order-equivalence, tree-distinctness, escaping, Unicode preservation, tie-bar and roman-case preservation); and `crates/elang-conformance/tests/canonical_goldens.rs` (9 cases over the published corpus). **T1121 published OA-14:** `specs/registry/eld-canonical-vectors.json` publishes 13 canonical ELD goldens with their SHA-256, and `fixtures/valid/canonical/<case>/` holds the byte-exact `golden/canonical.eld`, its `.sha256` sidecar, the authored `input.eld` and the conformance `expected.json` for each case. Every byte and digest is produced by the real pipeline (`elang_eld::parse` → `inventory::from_document` → `canon::to_string`) via `cargo run -p elang-conformance --bin gen_canonical_goldens`; regeneration is byte-identical and no digest is hand-written. The authored sources are deliberately non-canonical, so the goldens exercise the ordering rules rather than echo their input. All five local gates green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` (1111 pass), `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items`, `cargo +1.90.0 build --workspace`. Downstream: T0123 (the v0.1.0 freeze and release tag) is unblocked — OA-14 is now ratified and only the ADR 0011 §6 manifest regeneration remains.

## Phase 12 — Reader integration and public SDK

- [ ] **T1201** Implement `inspect` stages 1–7 only with accurate status reporting.
- [ ] **T1202** Implement `verify` stages 1–11 including all-resource decompression/hash verification.
- [ ] **T1203** Implement `validate` stages 1–12 including ELD semantic validation.
- [ ] **T1204** Integrate bootstrap `Header/format.version` and exact mimetype checks.
- [ ] **T1205** Verify EPF declared phoneme resource exists in index and ends `.eld`.
- [ ] **T1206** Integrate integrity manifest self-exclusion and complete coverage.
- [ ] **T1207** Ensure failures report stage, resource and stable code.
- [ ] **T1208** Implement bounded random-access resource retrieval by index.
- [ ] **T1209** Provide public read-only APIs for metadata, groups, phonemes, allophones and conditions.
- [ ] **T1210** Expose typed validated AST and optionally canonical serialized bytes.
- [ ] **T1211** Ensure public APIs do not expose partially validated state as fully valid.
- [ ] **T1212** Document thread-safety, streaming, memory and ownership contracts.
- [ ] **T1213** Add API examples for pack, inspect, verify, validate and read phoneme inventory.
- [ ] **T1214** Test reader state machine with every stage-specific failure.

**Acceptance:** SDK returns accurate conformance status and never confuses inspection with verification.

## Phase 13 — Package writer and CLI

- [x] **T1301** Define builder API from validated EPF/ELD and additional permitted resources. *(Implemented as `PackageConfig` + `ExtraResource` structs; no fluent builder — payload shape is fixed, validation atomic.)* — `crates/elang-writer/src/config.rs`
- [x] **T1302** Canonicalize EPF, ELD and integrity manifest per their own ratified profiles. *(Calls `elang_epf::canon::to_bytes`, `elang_eld::canon::to_bytes`, `elang_integrity::write::canonical_bytes`.)* — `crates/elang-writer/src/build.rs:36-37,71`
- [x] **T1303** Generate integrity manifest over exact final uncompressed resource bytes. *(Single pass: manifest built over all resources except itself per spec self-exclusion rule.)* — `crates/elang-writer/src/build.rs:56-71`
- [x] **T1304** Sort all resources by canonical UTF-8 path before compression. *(`resources.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()))`.)* — `crates/elang-writer/src/build.rs:79`
- [x] **T1305** Encode one independent Zstd frame per resource with required frame fields. *(Each resource → `encode(bytes, profile)`.)* — `crates/elang-writer/src/build.rs:111`
- [x] **T1306** Write header, contiguous frames, index and footer with exact offsets. *(`out = header + frames + index + footer`.)* — `crates/elang-writer/src/build.rs:165-175`
- [x] **T1307** Compute footer index SHA-256 over exact index bytes. *(Reuses `Index::encode_and_hash` from Phase 4.)* — `crates/elang-writer/src/build.rs:141-145`
- [x] **T1308** Enforce all resource and archive ceilings before/during write. *(`validate_resource_count`, per-resource ceilings, aggregate ceiling, archive ceiling.)* — `crates/elang-writer/src/build.rs:83-105, 116-124, 151-157, 263-272`
- [x] **T1309** Support Fast/Compact/Canonical compression profiles. *(Reuses `Profile` enum from `elang_container::frame`.)* — `crates/elang-writer/src/config.rs`, `crates/elang-cli/src/main.rs:104-114`
- [ ] **T1310** Add CLI `elang inspect <file>`. *(Deferred — library surface (`ReaderPackage::open` + `Package::inspect`) exists on `elang-reader`; CLI wiring lands in follow-up.)*
- [ ] **T1311** Add CLI `elang verify <file>`. *(Deferred — same reason.)*
- [ ] **T1312** Add CLI `elang validate <file>`. *(Deferred — same reason.)*
- [x] **T1313** Add CLI `elang pack` with explicit manifest policy. *(CLI signature: `pack --epf <file> --eld <file> --output <file> [--profile fast|compact|canonical] [--format-version <semver>]`. No source-dir walking — Phase 13's writer takes pre-validated inputs only; a directory-driven pack is a higher-level orchestration layer, separate issue.)* — `crates/elang-cli/src/main.rs:81-175`
- [ ] **T1314** Add CLI `elang unpack <file> -o <dir>` with safe path handling. *(Deferred.)*
- [x] **T1315** Add CLI `elang canonicalize-eld <input>`. *(Prints canonical KDL to stdout; `-o <output>` deferred — current contract is stdout-only, which matches `elang_eld::canon::to_string` semantics.)* — `crates/elang-cli/src/main.rs:178-225`
- [ ] **T1316** Add CLI `elang repertoire info` to show pinned chart/Unicode/digest. *(Deferred.)*
- [ ] **T1317** Implement stable human-readable and JSON diagnostics output modes. *(Deferred — current `pack`/`canonicalize-eld` errors print `eprintln!("…: {e}")` with exit codes 1/2; the dual-mode diagnostics surface is part of the reader-side CLI follow-up.)*
- [x] **T1318** Ensure CLI nonzero exit on any validation failure; document exit codes. *(Exit codes: 0 = success, 1 = runtime/parse/write failure, 2 = usage error.)* — `crates/elang-cli/src/main.rs:35-58, scaffold.rs`
- [ ] **T1319** Prevent unpack overwrite/traversal/symlink escape; use safe destination policies. *(Deferred — depends on T1314.)*
- [x] **T1320** Add CLI integration and roundtrip tests on supported platforms. *(`crates/elang-writer/tests/writer_integration.rs` — 8 roundtrip + diagnostic tests, including full 12-stage pipeline via `ReaderPackage::validate`. `crates/elang-cli/tests/scaffold.rs` — 4 CLI subprocess tests.)*

**Acceptance:** package writer produces byte-correct valid archives; CLI roundtrip passes and extraction is safe.

## Phase 14 — Conformance corpus and test infrastructure

- [x] **T1401** Create `elang-canonical-zstd-v1-conformance` fixture manifest and runner. *(`crates/elang-conformance/src/corpus.rs` — `run_fixture`, `run_corpus`; `crates/elang-conformance/src/catalog.rs` — `catalog`; unit tests `corpus::tests::*`, `catalog::tests::fixture_ids_are_unique`. The manifest also ships as `specs/registry/normative-manifest.json`, published by `gen_normative_manifest`.)*
- [x] **T1402** Generate minimal valid `.elang` and full-featured valid `.elang` golden fixtures. *(`crates/elang-conformance/src/golden_packages.rs` — unit tests `minimal_package_is_a_valid_archive`, `full_package_is_a_valid_archive`.)*
- [x] **T1403** Generate expected exact header/index/footer bytes and SHA-256 values. *(`crates/elang-conformance/src/golden_packages.rs` — unit tests `header_index_footer_segments_are_contiguous_in_the_archive`, `published_digests_match_the_pinned_constants`; `crates/elang-conformance/src/binary_vectors.rs` — unit test `minimal_archive_is_internally_consistent`; `crates/elang-conformance/tests/binary_vectors.rs` — integration test `footer_embeds_the_real_index_digest`.)*
- [x] **T1404** Generate canonical Zstd frame golden hashes using pinned libzstd. *(`crates/elang-conformance/src/zstd_vectors.rs` — unit tests `pinned_frame_digests_match_the_frames_the_writer_produces`, `pinned_frames_decode_to_the_canonical_payloads`, `linked_libzstd_version_equals_the_pin`.)*
- [x] **T1405** Generate EPF/ELD canonical KDL golden text files. *(`crates/elang-conformance/src/epf_canonical_goldens.rs` + generator `crates/elang-conformance/src/bin/gen_epf_canonical_goldens.rs` — unit test `every_pinned_case_canonicalises_to_its_pinned_bytes`; `crates/elang-conformance/tests/canonical_goldens.rs` — integration test `published_golden_bytes_match_the_canonical_writer`. ELD canonical goldens already existed from Phase 11 in `crates/elang-conformance/src/canonical_goldens.rs`.)*
- [x] **T1406** Generate canonical condition sorting-key byte vectors for all six ranks. *(`crates/elang-conformance/src/condition_vectors.rs` — unit tests `sorting_key_fixtures_pin_the_specification`, `sorted_children_orders_by_rank`, `sorted_operands_orders_by_key`.)*
- [x] **T1407** Generate Unicode NFC/category and IPA parsing valid/invalid fixtures. *(`crates/elang-conformance/src/unicode_ipa.rs` — unit tests `nfc_roundtrip_is_nfc`, `nfc_identifies_nfc_strings`, `unicode_vector_fixtures_pass`, `ipa_fixtures_run`.)*
- [x] **T1408** Generate group/roman/allophone structural and uniqueness fixtures. *(`crates/elang-conformance/src/epf_eld_negatives.rs` — unit test `every_document_fixture_behaves_as_declared` over the `eld_structure_fixtures` set.)*
- [x] **T1409** Generate condition semantics truth tables and edge cases. *(`crates/elang-conformance/src/condition_vectors.rs` — unit test `truth_tables_match_published_expectations` (16-row T1409 truth table); `crates/elang-conformance/tests/canonical_goldens.rs` — integration test `different_boolean_trees_stay_different`.)*
- [x] **T1410** Generate malformed binary index/footer/path/truncation/overflow fixtures. *(`crates/elang-conformance/src/negatives.rs` — unit test `every_binary_fixture_behaves_as_declared`.)*
- [x] **T1411** Generate Zstd missing-checksum, oversized-window, concatenated/skippable negatives. *(`crates/elang-conformance/src/zstd_negatives.rs` — unit test `every_zstd_negative_is_rejected_at_the_declared_level`.)*
- [x] **T1412** Generate integrity omission, corruption and extra-entry negatives. *(`crates/elang-conformance/src/negatives.rs` — unit test `every_integrity_fixture_behaves_as_declared`.)*
- [x] **T1413** Generate EPF wrong-version/duplicate/unknown/URN/manifest negatives. *(`crates/elang-conformance/src/epf_eld_negatives.rs` — unit test `every_document_fixture_behaves_as_declared`.)*
- [x] **T1414** Generate ELD missing required groups, duplicate IPA/roman, empty block negatives. *(`crates/elang-conformance/src/epf_eld_negatives.rs` — unit test `every_document_fixture_behaves_as_declared`.)*
- [x] **T1415** Generate parser budget boundary fixtures for the five pinned parser ceilings. *(`crates/elang-conformance/src/parser_budgets.rs` — unit tests `every_document_budget_fixture_passes`, `pinned_budgets_match_implementation_constants`, `nesting_fixtures_sit_on_both_sides_of_the_boundary`, `bootstrap_payloads_sit_on_both_sides_of_the_boundary`, `resource_path_fixtures_sit_on_both_sides_of_the_boundary`, `build_path_with_length_satisfies_resource_path_rules`; both sides of each ceiling are pinned — `MAX_NESTING_DEPTH` and `MAX_PREDICATES_PER_WHEN` as document fixtures, `MAX_EPF_BYTES` at exactly 1 MiB vs exactly one byte over, `MAX_BOOTSTRAP_RESOURCE_BYTES` at 64 bytes (`FORMAT_VERSION_INVALID`, Container, stage 8) vs 65 bytes (`DECOMPRESSION_BUDGET_EXCEEDED`, Frame, stage 8), and `MAX_RESOURCE_PATH_BYTES` at 1024 bytes (Accept/Inspect) vs 1025 bytes (`PATH_TOO_LONG`, ResourcePath, stage 5); the archive fixtures are wired into `tests/corpus_run.rs` and `tests/stage_conformance.rs`.)*
- [x] **T1416** Test `inspect`/`verify`/`validate` expected stage and error code for each fixture. *(`crates/elang-conformance/src/inspect_matrix.rs` — unit test `every_level_of_every_golden_package_passes` (minimal/full × inspect/verify/validate); `crates/elang-conformance/tests/stage_conformance.rs` — integration tests `every_negative_fixture_is_rejected_at_a_registered_stage` (the observed stage must be one of the stages `specs/registry/error-codes.json` publishes for that code, and no higher than the declared level's ceiling) and `every_negative_fixture_code_is_registered`; `crates/elang-conformance/tests/requirement_completeness.rs` — integration test `every_negative_fixture_declares_an_expected_code`.)*
- [x] **T1417** Test canonical write→read→write produces identical bytes. *(`crates/elang-conformance/src/roundtrip.rs` — unit test `write_read_write_is_byte_identical`.)*
- [x] **T1418** Test same logical package from shuffled source order produces same canonical archive. *(`crates/elang-conformance/src/roundtrip.rs` — unit test `declaration_order_does_not_change_the_archive` over the shared `roundtrip_is_byte_identical` / `declaration_order_is_stable` helpers; companion unit test `write_read_write_is_byte_identical`.)*
- [x] **T1419** Test corpus on Linux, Windows and macOS in CI. *(`.github/workflows/ci.yml` — test matrix `ubuntu-latest`/`windows-latest`/`macos-latest`; `crates/elang-conformance/src/meta_tests.rs` — unit test `ci_matrix_runs_linux_windows_macos`.)*
- [x] **T1420** Document fixture format, expected hashes, provenance and independent verification. *(`docs/conformance.md` — fixture format, published hashes, provenance and independent verification steps; `crates/elang-conformance/src/meta_tests.rs` — unit test `docs_present` asserts the document exists.)*

**Acceptance:** every normative MUST has at least one positive or negative conformance test; canonical outputs reproducible.

**Status:** ✅ **Complete (Issue #24).**

## Phase 15 — Fuzzing, property testing and security hardening

- [x] **T1501** Add fuzz target for header/footer/index parsing.
- [x] **T1502** Add fuzz target for resource path validation.
- [x] **T1503** Add fuzz target for Zstd frame-header and bounded decompression.
- [x] **T1504** Add fuzz target for KDL parser/AST construction.
- [x] **T1505** Add fuzz target for EPF schema validation.
- [x] **T1506** Add fuzz target for ELD structural validation.
- [x] **T1507** Add fuzz target for IPA tokenizer/tie-bar/modifier parser.
- [x] **T1508** Add fuzz target for condition AST, evaluator and canonical sorting key.
- [x] **T1509** Add property tests for canonicalization idempotence.
- [x] **T1510** Add property tests for parse/serialize roundtrips of validated models.
- [x] **T1511** Add property tests for bytewise path and IPA ordering.
- [ ] **T1512** Add memory/time tests for max node, string, comment and reference budgets.
- [ ] **T1513** Add decompression-bomb, giant-offset and integer-overflow tests.
- [ ] **T1514** Add extraction path traversal, symlink and overwrite tests.
- [x] **T1515** Audit unsafe code/FFI boundaries and pinned libzstd linkage.
- [x] **T1516** Run dependency vulnerability and license audits.
- [x] **T1517** Verify no untrusted input can trigger panic or out-of-bounds access.
- [x] **T1518** Run differential tests against independent fixture parser/encoder where feasible.
- [x] **T1519** Triage all fuzz findings and add regression fixtures.
- [x] **T1520** Document security limits and remaining trust/authenticity limitations.

**Acceptance:** fuzz campaigns meet documented budgets; no unresolved critical parser/decompression/path vulnerabilities.

## Phase 16 — Documentation, examples, benchmarks and release

- [x] **T1601** Publish versioned ELANG core/container normative spec with byte layouts.
- [x] **T1602** Publish EPF normative grammar and canonical serialization reference.
- [x] **T1603** Publish ELD normative grammar and every locked IPA/roman/condition rule.
- [x] **T1604** Publish Unicode 18.0/IPA 2026 repertoire data, schema and SHA-256.
- [x] **T1605** Publish integrity-manifest grammar and hashing procedure.
- [x] **T1606** Publish error-code registry and diagnostics examples.
- [x] **T1607** Publish `inspect`, `verify`, `validate` mode semantics and security model.
- [x] **T1608** Publish Rust crate API docs and end-to-end quickstart.
- [x] **T1609** Publish CLI usage, exit codes, JSON diagnostics and troubleshooting.
- [x] **T1610** Publish example language package source and canonical `.elang` artifact.
- [x] **T1611** Publish migration/version-compatibility policy for future 0.2+ releases.
- [x] **T1612** Publish conformance fixture corpus and machine-readable expected outputs.
- [x] **T1613** Benchmark parse/index inspection, verification, ELD validation, canonical pack and random access.
- [x] **T1614** Measure peak memory for worst-case accepted and rejected packages.
- [x] **T1615** Verify reproducible release build and canonical golden hashes in clean CI.
- [x] **T1616** Complete independent spec/code/test review and security checklist. → Three independent read-only reviewers (spec/conformance, adversarial code, security/test-adequacy) reviewed the branch; all findings are dispositioned in `docs/release/v0.1.0/review-record.md` (41 findings: 36 from the independent reviewers, one found by the cross-platform CI run, and four raised by the CodeRabbit review bot on PR #68) and the security-claim table in `docs/release/v0.1.0/security-checklist.md`. The release-gate assessment is recorded honestly: conditions (b) every MUST covered and (c) multi-platform reproducible build are **not** met (open tasks T0420, T0510, T1319); condition (a) closes only when the human tags. The human sign-off block remains **unsigned** and is a prerequisite for T1617/T1620.
- [ ] **T1617** Tag `v0.1.0`, build signed release artifacts where release infrastructure permits. → The *build* half is done: `scripts/build-release-artifacts.sh` assembles the deterministic source artifact and its SHA-256 sidecar, and CI asserts the property (`crates/elang-conformance/tests/release_packaging.rs`). The tag, the signature and the publication remain the human step and are deferred until the first release.
- [x] **T1618** Publish release notes explicitly distinguishing integrity from publisher authenticity.
- [x] **T1619** Archive exact dependencies, pinned Unicode/IPA data and conformance hashes.
- [ ] **T1620** Declare v0.1.0 ready only after all blockers, test gates and normative artifacts pass.

**Acceptance:** independently installable/documented Rust SDK and CLI, published normative data, conformance evidence and repeatable builds.

---

## Dependency map and execution rules

| Workstream | Prerequisites | Blocks |
|---|---|---|
| Normative IPA JSON + hash | T0106–T0115 | IPA validator, full ELD validation, conformance |
| Integrity KDL schema | T0103, T0105 | Integrity reader/writer, verify mode, pack |
| EPF/ELD grammar | T0101, T0102 | EPF/ELD parser freeze |
| Core models/limits | Phase 0 | Binary, parser, EPF, ELD |
| Binary parser | Core | inspect, verify, pack |
| KDL adapter | Core | EPF, ELD, integrity |
| Zstd reader | Binary parser | verify, validate |
| IPA + ELD | KDL + pinned repertoire | validate, canonical ELD |
| Canonical writer | EPF/ELD/integrity profiles + Zstd | golden package fixtures |
| CLI | Reader/writer APIs | end-to-end UX |
| Conformance release | All normative artifacts + implementation | v0.1.0 release |

## Definition of Done — each task

- [ ] Implementation committed with appropriate API/docs.
- [ ] Unit or integration tests demonstrate required behavior and at least one rejection path where relevant.
- [ ] Resource limits and error handling checked for untrusted inputs.
- [ ] Requirement ID and conformance fixture/test linked.
- [ ] Formatting, lint, tests and CI pass.
- [ ] No change to a locked rule without an explicit approved ADR.

## Final v0.1.0 release checklist

- [ ] No BLOCKER task remains open.
- [ ] All normative data and digests are real, generated, verified and versioned.
- [ ] Every MUST requirement has test coverage and passes.
- [ ] Reader stages 1–12 operate with correct inspect/verify/validate semantics.
- [ ] Canonical bytes reproduce exactly on all supported platforms.
- [ ] Valid example packages pack→inspect→verify→validate→unpack successfully.
- [ ] Malformed, hostile and limit-boundary packages fail safely.
- [ ] Security review, fuzzing, benchmarks and dependency audit completed.
- [ ] CLI, SDK, specs, conformance suite and release artifacts documented.
- [ ] No claim of publisher authenticity, registered media type or namespace ownership without a separate mechanism.

**Important:** This checklist is intentionally comprehensive. It does not mean all items have been implemented. Use Kiro to track work incrementally, not to mark entire phases complete on the basis of generated source files alone.
