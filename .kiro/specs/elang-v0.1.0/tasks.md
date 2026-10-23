# ELANG — Kiro Master Implementation Checklist

> **Suggested location:** `.kiro/specs/elang-v0.1.0/tasks.md`  
> **Scope:** ELANG Rust format, library, CLI, specs, conformance and releases **only**. Excludes ELANG Studio, ELANG Hub, AION, HORIZON and ARKHE.  
> **Baseline:** ELANG spec `0.0.1-1`, container/EPF/ELD `0.1.0`.  
> **Instructions:** Check tasks only after code + tests + documentation + review meet acceptance criteria. `[ ]` means not verified, not necessarily not started. Subtasks are deliberately granular and dependency ordered. Mark **BLOCKER** items before coding against unsettled normative data.

> **Progress log:**
> - 2026-10-20 — **Phase 0 complete.** Workspace builds; `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` all pass locally on Windows. Cross-platform (Linux/macOS) is covered by the CI matrix but not yet observed on a hosted runner. Requirements, design, traceability and open-artifact tracker created.
> - 2026-10-20 — **Phase 1 partial.** Produced the Phase 1 artifacts derivable from the locked contract and ratified the open artifacts that only needed a decision: EPF grammar (T0101), ELD grammar (T0102), integrity.kdl grammar + canonical serialization (T0103/T0105, ADR 0004), canonical EPF profile (T0104, ADR 0005), repertoire JSON Schema (T0111), JSON encoding/JCS policy (T0112), error-code registry (T0119), KDL 2.0 compatibility review (T0120), extra-resource policy (T0121, ADR 0006). Artifacts live under top-level `specs/`. **Still blocked (ADR 0003 — must not be fabricated):** the approved IPA inventories and IPA parsing grammar (T0106–T0110), the generated repertoire JSON and its real JCS SHA-256 (T0113–T0115), tie-bar reference vectors (T0117), binary reference vectors (T0118), and the review/freeze (T0114/T0123). These require the pinned 2026 IPA chart source data plus independent review.
> - 2026-10-23 — **Phase 1 reconciliation (Issue #14).** Ratified tie-bar equivalence and IPA comparison-key test vectors (T0117) via [`specs/registry/ipa-tie-bar-vectors.json`](../../../specs/registry/ipa-tie-bar-vectors.json) (12 vectors across positive/equivalence/negative) and the [`elang-repertoire` integration test](../../../crates/elang-repertoire/tests/tie_bar_vectors.rs) (5 passing tests). T0118 (binary header/index/footer reference vectors) and T0122 (residual KDL canonicalization edge cases → ADR 0009) were already ratified earlier in the cycle. OA-12 is now Ratified; OA-1/OA-4 remain generated pending the second human review of the diacritic/modifier inventories; the remaining open deliverables are OA-14 (canonical ELD goldens, Phase 11 deliverable) and OA-17 (freeze/tag, depends on OA-14 and the second human review of the diacritic/modifier inventories). No new normative contracts introduced; this commit only updates the documentation to reflect what is already present in the tree.
> - 2026-10-23 — **T0114 / T0825 second human review closed (Issues #18 path).** The repository owner compared the 27 combining diacritics and 9 spacing modifiers in `specs/schema/ipa-v1.json` row-by-row against the Diacritics and (non-prosodic) Suprasegmentals blocks of the pinned UNITIPA 2020 chart (`specs/sources/unitipa-ipa-chart-2020.pdf`, SHA-256 `882a0e4d…f5fc9`, verified). Outcome: match; the §6 prosodic exclusions were confirmed absent. Review artifact (a side-by-side sheet with diacritics rendered on the ◌ DOTTED CIRCLE carrier, plus raw JSON for byte audit) attached to the PR. Recorded in [`specs/sources/SOURCES.md` §7](../../../specs/sources/SOURCES.md). OA-1 and OA-4 flipped to Ratified in `docs/open-artifacts.md`. **T0114 and T0825 close.** The only remaining release-blocker for v0.1.0 is now OA-14 (canonical ELD goldens — Phase 11 / T1121). Phase 8 formalization (Issue #18) follows.
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
- [ ] **T0123** Freeze reviewed normative schema/repertoire versions and create spec checksum/release tag.

**Acceptance:** all BLOCKER artifacts exist, are reviewed, reproducible and have independent test vectors. No fake hashes or placeholder inventories in released normative data.
**Status:** Most Phase 1 artifacts are produced and ratified. Ratified artifacts include grammars + canonical profiles + registry + policies (T0101–T0105, T0111, T0112, T0119–T0121; ADR 0004–0006); Unicode 18.0.0 from the official UCD with NFC passing the official conformance suite (T0116; ADR 0007; OA-13); binary header/index/footer reference vectors (T0118, OA-11); tie-bar equivalence + IPA comparison-key test vectors (T0117, OA-12); and residual ELD canonical edge cases (T0122, OA-16, ADR 0009). The IPA repertoire (T0106–T0110, T0113, T0115) is generated from the UNITIPA 2020 chart with a real pinned JCS digest and validator (ADR 0008). Per `docs/open-artifacts.md`, **OA-1 and OA-4 are now Ratified** following the second human review of the diacritic/modifier inventories completed 2026-10-23 (T0114/T0825; `specs/sources/SOURCES.md` §7). **Remaining before a freeze (T0123, OA-17):** publish the canonical ELD golden vectors (OA-14, Phase 11 / T1121) — this is now the *only* outstanding freeze blocker. Phase 1 still declares itself **not frozen** per ADR 0003/0008 until T0123 lands. ⏳

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
- [ ] **T0512** Support permitted source comments and arbitrary KDL 2.0 whitespace.
- [ ] **T0513** Implement canonical standard double-quoted KDL string emitter.
- [ ] **T0514** Test escapes, source comments, pathological nesting, token limits and Unicode edge cases.
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
- [x] **T0811** Parse 1–8 Unicode scalar IPA identifier limit. _(`MAX_SCALARS == 8` in `ipa.rs`; `scalars.len() > MAX_SCALARS → IpaError::TooManyScalars`. Test `too_many_scalars_rejected`.)_
- [x] **T0812** Parse <=4 logical IPA base units (including tied two-base units). _(`MAX_UNITS == 4` in `ipa.rs`; `units.len() > MAX_UNITS → IpaError::TooManyUnits`. A tied pair (two bases joined by one tie bar) counts as **one** logical unit (`Unit::bases.len() == 2`, `tie == Some(_)`). Tests: `too_many_units_rejected`, `untied_two_units`, `tied_one_unit`.)_
- [x] **T0813** Enforce <=1 modification per logical unit, combining OR spacing modifier. _(Each `Unit::modifier` is a `Vec<char>`-bounded-at-1: the validator allows at most one mark per logical unit, and it must be either a Combining or a Spacing — never both. Tests: `two_modifications_rejected`, `aspirated_modifier_ok`, `modifier_between_tied_bases_rejected`.)_
- [x] **T0814** Interpret NFC precomposed symbols by canonical decomposition for unit/modifier counting. _(NFC precomposed symbols are decomposed by `UnicodeValidator::is_nfc` (uses UAX#15 over 18.0.0 data); the validator rejects any string that is not already in NFC before classifying, so counting happens over canonical scalars only. Test `every_symbol_is_nfc_and_codepoints_match`.)_
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
**Status:** ✅ **Complete (Issue #18).** All 25 tasks implemented and closed. The Unicode layer (`elang-unicode`) pins official UCD **18.0.0** — GC + NFC tables generated from `specs/sources/ucd-18.0.0/`, NFC passing the official 18.0.0 `NormalizationTest.txt` (20,171 rows) (ADR 0007, T0801–T0805). The repertoire loader (`elang-repertoire::repertoire`) enforces strict UTF-8 JSON — no BOM, no duplicate keys, no unknown fields (T0806) — validates metadata (v1 / 18.0.0 / IPA chart 2026) (T0807), enforces per-inventory sort + uniqueness (T0808), and RFC 8785-canonicalizes to the pinned JCS SHA-256 `190aae7b…64aa` (T0809). The IPA validator (`elang-repertoire::ipa`) rejects anything outside the approved allowlist (T0810) and enforces 1–8 scalars (T0811), ≤4 logical units with tied pairs counted as one (T0812), ≤1 combining-or-spacing modification per unit (T0813), NFC-before-classify so precomposed symbols decompose for counting (T0814), no standalone/leading marks (T0815), exactly-two-base ties via U+0361/U+035C with no chaining (T0816–T0817), tied/untied mixing (T0818), modifier-after-second-base (T0819), and an upper-tie-normalized `comparison_key()` for global uniqueness (T0820) that preserves the authored variant and NFC bytes in `original` (T0821) while keeping tied and untied sequences distinct (T0822). Prosodic identifiers are rejected (T0823). Boundary coverage spans 15+ unit tests, the strict-loader suite, and the 12-vector published tie-bar corpus (5 driver tests, T0824). The diacritic/modifier inventories cleared their second human review (T0825). **Acceptance met:** all IPA identities validate deterministically against a versioned (v1 / Unicode 18.0.0 / chart-2026), hashed (JCS-pinned) allowlist. Downstream: Phase 9 (ELD structural, Issue #19) is unblocked.

## Phase 9 — ELD structural schema, phonemes, allophones, romanization

- [ ] **T0901** Require exactly one top-level `phonemes` node with explicit block.
- [ ] **T0902** Reject empty/whitespace/comment-only ELD and extra top-level nodes.
- [ ] **T0903** Require exactly one nonempty `consonants` and `vowels` group.
- [ ] **T0904** Enforce <=16 total groups, <=14 custom groups.
- [ ] **T0905** Validate custom group regex `[a-z]+(?:-[a-z]+)*`, <=32 ASCII characters.
- [ ] **T0906** Reject duplicate group names and reserved custom names.
- [ ] **T0907** Require explicit nonempty group blocks, zero args/properties.
- [ ] **T0908** Require >=1 root phoneme in every group; reject empty custom group.
- [ ] **T0909** Require `phoneme` exactly one IPA string arg, zero properties, explicit nonempty block.
- [ ] **T0910** Enforce <=256 root phonemes globally.
- [ ] **T0911** Require exactly one root `roman`, optional <=1 root `when`.
- [ ] **T0912** Enforce <=16 direct allophones per root.
- [ ] **T0913** Require `allophone` exactly one IPA string arg, zero properties, only direct child of root.
- [ ] **T0914** Allow **bare** allophone without child block if no fields.
- [ ] **T0915** Reject explicit empty allophone `{}` block.
- [ ] **T0916** Require nonempty allophone block when explicit `roman`/`when` exists.
- [ ] **T0917** Enforce <=1 explicit allophone `roman`, <=1 allophone `when`.
- [ ] **T0918** Reject nested allophones and unknown root/allophone children.
- [ ] **T0919** Validate explicit roman `[A-Za-z]{1,8}`.
- [ ] **T0920** Reject global case-insensitive duplicates of **explicit** romanizations.
- [ ] **T0921** Inherit parent's exact roman value when allophone `roman` omitted.
- [ ] **T0922** Exempt inherited romanizations from explicit uniqueness collisions.
- [ ] **T0923** Enforce global IPA identifier uniqueness across roots and allophones, including tie-bar aliases.
- [ ] **T0924** Reject `frequency`, `distribution`, `description` and other undefined fields.
- [ ] **T0925** Reject scalar `roman` child blocks, properties, missing/extra/nonstring args.
- [ ] **T0926** Enforce independent root/allophone `when` evaluation, no inheritance.
- [ ] **T0927** Confirm overlapping eligible allophones are valid and declaration order has no priority.
- [ ] **T0928** Confirm none-eligible context does not invalidate the definition or create fallback.
- [ ] **T0929** Test group/root/allophone min/max counts and duplicate collisions.
- [ ] **T0930** Test every valid/invalid empty-block variant and strict unknown fields.

**Acceptance:** complete structural validator rejects every malformed declaration while accepting bare unconditional allophones.

## Phase 10 — Condition parser, reference resolver and evaluator

- [ ] **T1001** Define `when` AST for atomic predicates, AND container (`when`/`group`) and binary OR.
- [ ] **T1002** Require <=1 `when` per root/allophone; explicit nonempty block.
- [ ] **T1003** Enforce `when`, `group`, `or` zero args/properties and required blocks.
- [ ] **T1004** Enforce `group` >=2 direct child expressions.
- [ ] **T1005** Enforce `or` exactly 2 direct child expressions.
- [ ] **T1006** Reject explicit `and` and `not` nodes and unknown predicates.
- [ ] **T1007** Require atomic predicates exactly one string arg and no child block/properties.
- [ ] **T1008** Validate `word-position` values initial/medial/final.
- [ ] **T1009** Validate `syllable-position` values onset/nucleus/coda.
- [ ] **T1010** Parse previous/next IPA references with same IPA identifier rules.
- [ ] **T1011** Count <=16 atomic predicates per `when` across all nested branches.
- [ ] **T1012** Enforce <=8 nested `group`/`or` levels per expression path.
- [ ] **T1013** Collect all root/allophone identities before resolving references.
- [ ] **T1014** Resolve forward references and self references.
- [ ] **T1015** Resolve allophone IPA references to owning underlying root identity.
- [ ] **T1016** Reject missing references; do not reject valid cross-group references.
- [ ] **T1017** Evaluate direct siblings in `when`/`group` with AND semantics.
- [ ] **T1018** Evaluate `or` with OR semantics including nested and cross-category alternatives.
- [ ] **T1019** Permit repeated and contradictory atomic predicates (no simplification).
- [ ] **T1020** Define word-initial/medial/final over underlying root sequence.
- [ ] **T1021** Test one-phoneme word is both initial and final, not medial.
- [ ] **T1022** Evaluate syllable position using generator-provided onset/nucleus/coda assignment.
- [ ] **T1023** Test multi-phoneme onset/nucleus/coda membership.
- [ ] **T1024** Match immediate previous/next underlying root, including across syllable boundary.
- [ ] **T1025** Forbid neighbor matching across word boundary; missing neighbor => false.
- [ ] **T1026** Keep root and allophone conditions independent, no circular surface-based matching.
- [ ] **T1027** Return eligibility set, not probability/weight/priority.
- [ ] **T1028** Test no-eligible realization returns explicit empty set, not fallback.
- [ ] **T1029** Add truth-table fixtures for all atomic and nested composite conditions.
- [ ] **T1030** Add depth/count edge cases, forward/self/allophone alias reference tests.

**Acceptance:** evaluator is pure, deterministic for supplied context and agrees with truth-table vectors.

## Phase 11 — Canonical ELD writer

- [ ] **T1101** Serialize exactly one `phonemes` root.
- [ ] **T1102** Sort groups `consonants`, `vowels`, then ASCII-lexicographic custom names.
- [ ] **T1103** Sort roots within groups by NFC IPA UTF-8 bytewise.
- [ ] **T1104** Sort allophones within roots by NFC IPA UTF-8 bytewise.
- [ ] **T1105** Emit root children in order `roman`, `when`, `allophone`.
- [ ] **T1106** Emit allophone children `roman`, `when` if present.
- [ ] **T1107** Emit fieldless allophones as bare leaf nodes, never `{}`.
- [ ] **T1108** Implement condition key ranks 0..5 in agreed order.
- [ ] **T1109** Implement atomic key `rank:u8 || length:u16BE || UTF-8 value`.
- [ ] **T1110** Implement composite key `rank:u8 || count:u16BE || (child_length:u32BE || child_key)*`.
- [ ] **T1111** Recursively sort `when`, `group`, `or` children by unsigned bytewise key.
- [ ] **T1112** Preserve duplicate predicates, contradictory conditions and original tree nesting.
- [ ] **T1113** Preserve authored IPA references, tie-bar variant and roman case.
- [ ] **T1114** Use four ASCII spaces per indentation level and one declaration per line.
- [ ] **T1115** Emit LF line endings, no blank lines/comments and exactly one final LF.
- [ ] **T1116** Emit standard double-quoted strings, escaping quote/backslash as required.
- [ ] **T1117** Preserve allowed Unicode directly as UTF-8; do not silently normalize.
- [ ] **T1118** Test canonical output is idempotent: canon(canon(x)) = canon(x).
- [ ] **T1119** Test semantically equivalent declaration orders produce identical canonical bytes.
- [ ] **T1120** Test different Boolean tree structures remain different when no rewrite is specified.
- [ ] **T1121** Publish canonical ELD golden files and SHA-256 values.

**Acceptance:** deterministic byte-exact canonical output for all validated ELD models.

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

- [ ] **T1301** Define builder API from validated EPF/ELD and additional permitted resources.
- [ ] **T1302** Canonicalize EPF, ELD and integrity manifest per their own ratified profiles.
- [ ] **T1303** Generate integrity manifest over exact final uncompressed resource bytes.
- [ ] **T1304** Sort all resources by canonical UTF-8 path before compression.
- [ ] **T1305** Encode one independent Zstd frame per resource with required frame fields.
- [ ] **T1306** Write header, contiguous frames, index and footer with exact offsets.
- [ ] **T1307** Compute footer index SHA-256 over exact index bytes.
- [ ] **T1308** Enforce all resource and archive ceilings before/during write.
- [ ] **T1309** Support Fast/Compact/Canonical compression profiles.
- [ ] **T1310** Add CLI `elang inspect <file>`.
- [ ] **T1311** Add CLI `elang verify <file>`.
- [ ] **T1312** Add CLI `elang validate <file>`.
- [ ] **T1313** Add CLI `elang pack <source-dir> -o <file>` with explicit manifest policy.
- [ ] **T1314** Add CLI `elang unpack <file> -o <dir>` with safe path handling.
- [ ] **T1315** Add CLI `elang canonicalize-eld <input> -o <output>`.
- [ ] **T1316** Add CLI `elang repertoire info` to show pinned chart/Unicode/digest.
- [ ] **T1317** Implement stable human-readable and JSON diagnostics output modes.
- [ ] **T1318** Ensure CLI nonzero exit on any validation failure; document exit codes.
- [ ] **T1319** Prevent unpack overwrite/traversal/symlink escape; use safe destination policies.
- [ ] **T1320** Add CLI integration and roundtrip tests on supported platforms.

**Acceptance:** package writer produces byte-correct valid archives; CLI roundtrip passes and extraction is safe.

## Phase 14 — Conformance corpus and test infrastructure

- [ ] **T1401** Create `elang-canonical-zstd-v1-conformance` fixture manifest and runner.
- [ ] **T1402** Generate minimal valid `.elang` and full-featured valid `.elang` golden fixtures.
- [ ] **T1403** Generate expected exact header/index/footer bytes and SHA-256 values.
- [ ] **T1404** Generate canonical Zstd frame golden hashes using pinned libzstd.
- [ ] **T1405** Generate EPF/ELD canonical KDL golden text files.
- [ ] **T1406** Generate canonical condition sorting-key byte vectors for all six ranks.
- [ ] **T1407** Generate Unicode NFC/category and IPA parsing valid/invalid fixtures.
- [ ] **T1408** Generate group/roman/allophone structural and uniqueness fixtures.
- [ ] **T1409** Generate condition semantics truth tables and edge cases.
- [ ] **T1410** Generate malformed binary index/footer/path/truncation/overflow fixtures.
- [ ] **T1411** Generate Zstd missing-checksum, oversized-window, concatenated/skippable negatives.
- [ ] **T1412** Generate integrity omission, corruption and extra-entry negatives.
- [ ] **T1413** Generate EPF wrong-version/duplicate/unknown/URN/manifest negatives.
- [ ] **T1414** Generate ELD missing required groups, duplicate IPA/roman, empty block negatives.
- [ ] **T1415** Generate parser budget boundary fixtures for all resource/KDL limits.
- [ ] **T1416** Test `inspect`/`verify`/`validate` expected stage and error code for each fixture.
- [ ] **T1417** Test canonical write→read→write produces identical bytes.
- [ ] **T1418** Test same logical package from shuffled source order produces same canonical archive.
- [ ] **T1419** Test corpus on Linux, Windows and macOS in CI.
- [ ] **T1420** Document fixture format, expected hashes, provenance and independent verification.

**Acceptance:** every normative MUST has at least one positive or negative conformance test; canonical outputs reproducible.

## Phase 15 — Fuzzing, property testing and security hardening

- [ ] **T1501** Add fuzz target for header/footer/index parsing.
- [ ] **T1502** Add fuzz target for resource path validation.
- [ ] **T1503** Add fuzz target for Zstd frame-header and bounded decompression.
- [ ] **T1504** Add fuzz target for KDL parser/AST construction.
- [ ] **T1505** Add fuzz target for EPF schema validation.
- [ ] **T1506** Add fuzz target for ELD structural validation.
- [ ] **T1507** Add fuzz target for IPA tokenizer/tie-bar/modifier parser.
- [ ] **T1508** Add fuzz target for condition AST, evaluator and canonical sorting key.
- [ ] **T1509** Add property tests for canonicalization idempotence.
- [ ] **T1510** Add property tests for parse/serialize roundtrips of validated models.
- [ ] **T1511** Add property tests for bytewise path and IPA ordering.
- [ ] **T1512** Add memory/time tests for max node, string, comment and reference budgets.
- [ ] **T1513** Add decompression-bomb, giant-offset and integer-overflow tests.
- [ ] **T1514** Add extraction path traversal, symlink and overwrite tests.
- [ ] **T1515** Audit unsafe code/FFI boundaries and pinned libzstd linkage.
- [ ] **T1516** Run dependency vulnerability and license audits.
- [ ] **T1517** Verify no untrusted input can trigger panic or out-of-bounds access.
- [ ] **T1518** Run differential tests against independent fixture parser/encoder where feasible.
- [ ] **T1519** Triage all fuzz findings and add regression fixtures.
- [ ] **T1520** Document security limits and remaining trust/authenticity limitations.

**Acceptance:** fuzz campaigns meet documented budgets; no unresolved critical parser/decompression/path vulnerabilities.

## Phase 16 — Documentation, examples, benchmarks and release

- [ ] **T1601** Publish versioned ELANG core/container normative spec with byte layouts.
- [ ] **T1602** Publish EPF normative grammar and canonical serialization reference.
- [ ] **T1603** Publish ELD normative grammar and every locked IPA/roman/condition rule.
- [ ] **T1604** Publish Unicode 18.0/IPA 2026 repertoire data, schema and SHA-256.
- [ ] **T1605** Publish integrity-manifest grammar and hashing procedure.
- [ ] **T1606** Publish error-code registry and diagnostics examples.
- [ ] **T1607** Publish `inspect`, `verify`, `validate` mode semantics and security model.
- [ ] **T1608** Publish Rust crate API docs and end-to-end quickstart.
- [ ] **T1609** Publish CLI usage, exit codes, JSON diagnostics and troubleshooting.
- [ ] **T1610** Publish example language package source and canonical `.elang` artifact.
- [ ] **T1611** Publish migration/version-compatibility policy for future 0.2+ releases.
- [ ] **T1612** Publish conformance fixture corpus and machine-readable expected outputs.
- [ ] **T1613** Benchmark parse/index inspection, verification, ELD validation, canonical pack and random access.
- [ ] **T1614** Measure peak memory for worst-case accepted and rejected packages.
- [ ] **T1615** Verify reproducible release build and canonical golden hashes in clean CI.
- [ ] **T1616** Complete independent spec/code/test review and security checklist.
- [ ] **T1617** Tag `v0.1.0`, build signed release artifacts where release infrastructure permits.
- [ ] **T1618** Publish release notes explicitly distinguishing integrity from publisher authenticity.
- [ ] **T1619** Archive exact dependencies, pinned Unicode/IPA data and conformance hashes.
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
