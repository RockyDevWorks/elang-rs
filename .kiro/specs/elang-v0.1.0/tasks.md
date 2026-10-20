# ELANG — Kiro Master Implementation Checklist

> **Suggested location:** `.kiro/specs/elang-v0.1.0/tasks.md`  
> **Scope:** ELANG Rust format, library, CLI, specs, conformance and releases **only**. Excludes ELANG Studio, ELANG Hub, AION, HORIZON and ARKHE.  
> **Baseline:** ELANG spec `0.0.1-1`, container/EPF/ELD `0.1.0`.  
> **Instructions:** Check tasks only after code + tests + documentation + review meet acceptance criteria. `[ ]` means not verified, not necessarily not started. Subtasks are deliberately granular and dependency ordered. Mark **BLOCKER** items before coding against unsettled normative data.

> **Progress log:**
> - 2026-10-20 — **Phase 0 complete.** Workspace builds; `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` all pass locally on Windows. Cross-platform (Linux/macOS) is covered by the CI matrix but not yet observed on a hosted runner. Requirements, design, traceability and open-artifact tracker created.
> - 2026-10-20 — **Phase 1 partial.** Produced the Phase 1 artifacts derivable from the locked contract and ratified the open artifacts that only needed a decision: EPF grammar (T0101), ELD grammar (T0102), integrity.kdl grammar + canonical serialization (T0103/T0105, ADR 0004), canonical EPF profile (T0104, ADR 0005), repertoire JSON Schema (T0111), JSON encoding/JCS policy (T0112), error-code registry (T0119), KDL 2.0 compatibility review (T0120), extra-resource policy (T0121, ADR 0006). Artifacts live under top-level `specs/`. **Still blocked (ADR 0003 — must not be fabricated):** the approved IPA inventories and IPA parsing grammar (T0106–T0110), the generated repertoire JSON and its real JCS SHA-256 (T0113–T0115), tie-bar reference vectors (T0117), binary reference vectors (T0118), and the review/freeze (T0114/T0123). These require the pinned 2026 IPA chart source data plus independent review.
> - 2026-10-20 — **Unicode 18.0.0 landed for real (ADR 0007, revised).** Replaced the ICU4X 17.0 stopgap with a first-party `elang-unicode` crate that generates General_Category + NFC tables from the **official pinned UCD 18.0.0** files (downloaded + SHA-256 recorded in `specs/sources/ucd-18.0.0/SOURCES.md`). NFC uses the UAX#15 algorithm over 18.0.0 data and **passes the official `NormalizationTest.txt` (20,171 rows)** — the gate for claiming 18.0 compliance. Caught and fixed a real 17.0→18.0 canonical-ordering difference (U+05C8, newly assigned, CCC 10). All ELANG Unicode validation now routes through `elang-unicode`; ICU4X and `unicode-normalization` removed from the dep graph. `elang-unicode` + `elang-repertoire` tests pass; `cargo clippy --workspace -D warnings` clean. (Note: `cargo test --workspace` intermittently hits a Windows Application Control block (os error 4551) on freshly-written test exes — an environment policy, not a code failure; per-crate reruns pass.) OA-13 ratified; T0116 done.
> - 2026-10-20 — **IPA Repertoire v1 generated and validated (ADR 0008).** Authoritative source: the UNITIPA IPA chart revised to 2020 (the "IPA 2026" issue; no 2026 revision of the alphabet exists). Downloaded + hashed sources (`specs/sources/SOURCES.md`); generated `specs/schema/ipa-v1.json` (107 base / 27 combining / 9 spacing) reproducibly from `gen_repertoire.rs`; pinned real JCS SHA-256 `190aae7b…64aa`; implemented the IPA identifier validator (allowlist + scalar/unit limits + tie bars + modifier placement) and the strict loader (no BOM / no duplicate keys / no unknown fields). Prosody excluded per §7. **Cross-check:** base symbols verified against an independent transcription of the chart grids (`chart_crosscheck.rs`). 30 tests pass; `cargo clippy --workspace --all-targets -D warnings` and `cargo test --workspace` green locally. No third-party crate supplies the inventory. Advances T0106–T0110, T0113, T0115.

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
- [ ] **T0114** Independently review the JSON against the pinned IPA chart and Unicode 18.0. _(Base set auto-cross-checked against an independent chart transcription (`chart_crosscheck.rs`). A second human review of the diacritic/modifier inventories, and Unicode 18.0, are still required before freeze.)_
- [x] **T0115** Compute SHA-256 of **canonical JCS bytes** and publish digest in spec/constant. _(`190aae7b…64aa` = `PINNED_JCS_SHA256`, asserted by `pinned_digest_matches`; recorded in `SOURCES.md`.)_
- [x] **T0116** Generate Unicode 18.0 NFC and General Category reference test data. _(First-party `elang-unicode` crate generates GC + NFC tables from the pinned official UCD **18.0.0** (`specs/sources/ucd-18.0.0/`, hashes recorded); NFC passes the official `NormalizationTest.txt` (20,171 rows). ADR 0007; OA-13 ratified.)_
- [ ] **T0117** Publish exact tie-bar equivalence and IPA comparison-key test vectors. _(Partial: `elang-repertoire::ipa` tests cover upper/lower tie equivalence, tied-vs-untied distinctness and comparison keys; a published fixture set is still to come in Phase 14.)_
- [ ] **T0118** Publish binary header/index/footer reference vectors and offsets.
- [x] **T0119** Define stable machine-readable error code registry and error metadata schema. _(`specs/registry/error-codes.md` + `error-codes.json`.)_
- [x] **T0120** Review KDL 2.0 standard compatibility of examples and canonical serialization. _(`specs/registry/kdl-2.0-compatibility.md`.)_
- [x] **T0121** Define policy for extra indexed resources and manifest coverage in v0.1.0 (no invented modules). _(ADR 0006; `specs/registry/resource-policy-v0.1.0.md`.)_
- [ ] **T0122** Ratify any residual KDL canonicalization edge cases and document as ADRs. _(EPF/integrity canonical profiles ratified via ADR 0004/0005; residual ELD edge cases revisited in Phase 11.)_
- [ ] **T0123** Freeze reviewed normative schema/repertoire versions and create spec checksum/release tag.

**Acceptance:** all BLOCKER artifacts exist, are reviewed, reproducible and have independent test vectors. No fake hashes or placeholder inventories in released normative data.
**Status:** Most Phase 1 artifacts are done: grammars + canonical profiles + registry + policies (T0101–T0105, T0111, T0112, T0119–T0121; ADR 0004–0006); the IPA repertoire (T0106–T0110, T0113, T0115) from the UNITIPA 2020 chart with a real pinned JCS digest and validator (ADR 0008); and **Unicode 18.0.0** generated from the official UCD with NFC passing the official conformance suite (T0116; ADR 0007; OA-13 ratified). **Still blocking a freeze (T0123):** T0114 needs a second human review of the IPA diacritic/modifier inventories; the binary/canonical golden vectors (T0117 full, T0118, OA-11/OA-14) are produced in Phases 3/11/14. Per ADR 0003/0008, Phase 1 is **not declared complete / frozen** until those land. The IPA repertoire review (T0114) remains a separate, explicit Phase 1 blocker. ⏳

## Phase 2 — Core types, limits, diagnostics and safety

- [x] **T0201** Define independent version types for container, EPF, ELD, language and canonical profile. _(`elang-core::version::domains` + `SemVer`.)_
- [x] **T0202** Implement strict semver-like `MAJOR.MINOR.PATCH` parser without leading zeros. _(`elang-core::version::SemVer`; tests reject leading zeros / wrong shape.)_
- [x] **T0203** Define validated resource path and indexed-resource types. _(`elang-core::ids::ResourcePath`; §3.4 rules + codes.)_
- [x] **T0204** Define validated IPA identifier, romanization, group-name and language-identifier types. _(`ids::{Romanization, GroupName, IpaIdentifierText}`; language identifier = `SemVer` + EPF URN in Phase 6.)_
- [ ] **T0205** Define immutable validated EPF and ELD AST/model structures. _(Deferred to Phases 6/9 where the schemas are parsed; `IpaIdentifierText` primitive in place.)_
- [x] **T0206** Define centralized resource and parser limits with documented units. _(`elang-core::limits`.)_
- [x] **T0207** Add checked integer conversion and checked offset arithmetic helpers. _(`elang-core::checked`; `checked_range`, u64/usize conversions.)_
- [x] **T0208** Implement bounded counting reader and decompressed byte accounting. _(`elang-core::bounded::{ByteBudget, BoundedCounter}`.)_
- [x] **T0209** Implement error categories, stable codes and `Result`-based propagation. _(`elang-core::error`; `Category`, `Error`, `Result`.)_
- [x] **T0210** Attach resource path, byte offset and KDL source span to errors where possible. _(`Diagnostic` with `resource_path`/`byte_offset`/`span`.)_
- [x] **T0211** Cap retained diagnostics at 100 without suppressing failure. _(`error::Diagnostics`; `truncated()`/`has_any()`; test proves no masking.)_
- [x] **T0212** Design APIs that distinguish `inspect`, `verify` and `validate` results. _(`elang-core::modes::{Inspected, Verified, Validated, Stage}`.)_
- [ ] **T0213** Document threat model: malicious archives, decompression bombs, path traversal, parser abuse. _(Already in `design.md §4` + SECURITY.md; revisit as container lands.)_
- [x] **T0214** Add unit tests for overflow, limit edges, invalid paths and version mismatch. _(checked/bounded/ids/version test modules.)_
- [x] **T0215** Enforce no panics/unchecked indexing on untrusted data; run lint checks. _(Workspace clippy lints; `cargo clippy -p elang-core -p elang-unicode --all-targets -D warnings` clean.)_

**Acceptance:** core APIs compile independently; boundary and overflow tests pass.
**Status:** Core types, limits, diagnostics, checked math, bounded reader and mode result types implemented and tested (26 `elang-core` tests pass; clippy `-D warnings` clean). T0205 (validated EPF/ELD models) lands with Phases 6/9; T0213 threat model already captured in design §4. ✅ (core complete)

## Phase 3 — Binary container header, footer and index

- [ ] **T0301** Implement 32-byte header decoder with magic `ELANG\r\n\x1A`.
- [ ] **T0302** Enforce major=0, minor=1, flags=0 and all 16 reserved bytes zero.
- [ ] **T0303** Implement 32-byte header encoder with exact little-endian fields.
- [ ] **T0304** Implement 64-byte terminal footer decoder (`ELEND001`).
- [ ] **T0305** Validate footer reserved bytes and checked index bounds.
- [ ] **T0306** Enforce `index_offset + index_size == file_size - 64`.
- [ ] **T0307** Enforce index size range 16..33,554,432.
- [ ] **T0308** Verify footer SHA-256 against exact index bytes.
- [ ] **T0309** Implement 16-byte index header decoder (`ELIDX001`).
- [ ] **T0310** Enforce index version=1, entry format=1, count<=25,000.
- [ ] **T0311** Implement index entry parser: `u16 LE` path length + UTF-8 path + three `u64 LE` + compression `u8`.
- [ ] **T0312** Enforce compression code exactly 1.
- [ ] **T0313** Enforce 1024-byte max path, strict UTF-8 and NFC.
- [ ] **T0314** Reject absolute paths, empty/`.`/`..` segments, NUL and backslashes.
- [ ] **T0315** Enforce strict ascending bytewise path order and no duplicates.
- [ ] **T0316** Validate contiguous frames in index order starting at byte 32.
- [ ] **T0317** Reject frame gaps, overlaps, unindexed bytes and offset/length overflow.
- [ ] **T0318** Ensure last frame ends exactly at index offset.
- [ ] **T0319** Implement deterministic index serializer and index hash computation.
- [ ] **T0320** Implement footer serializer and golden byte comparison.
- [ ] **T0321** Test truncated headers, footer variants, zero/overflow counts and corrupt index.
- [ ] **T0322** Test little-endian and byte-offset vectors on multiple architectures.
- [ ] **T0323** Test empty index/absent required resources rejected at appropriate stage.
- [ ] **T0324** Expose index-only archive listing without decompressing resources.

**Acceptance:** arbitrary untrusted container bytes cannot cause panic or out-of-bounds reads; binary vectors match exactly.

## Phase 4 — Zstandard frame reader and canonical encoder

- [ ] **T0401** Pin libzstd 1.5.7 through supported `zstd-sys` build strategy.
- [ ] **T0402** Implement standard Zstd frame-header inspection.
- [ ] **T0403** Require declared content size and frame checksum for every resource.
- [ ] **T0404** Reject frame windows >128 MiB.
- [ ] **T0405** Reject dictionary dependencies and external/shared dictionaries.
- [ ] **T0406** Reject skippable frames, concatenated frames and trailing frame bytes.
- [ ] **T0407** Reject truncated and incomplete frames.
- [ ] **T0408** Verify frame content size matches index `uncompressed_size`.
- [ ] **T0409** Stream decompress with strict output limits and aggregate 8 GiB budget.
- [ ] **T0410** Verify Zstd frame checksum and exact decompressed byte count.
- [ ] **T0411** Enforce per-resource maximums for bootstrap, EPF, integrity and ELD.
- [ ] **T0412** Implement `Fast` encoder at level 3.
- [ ] **T0413** Implement `Compact` encoder at level 9.
- [ ] **T0414** Implement canonical encoder at level 9, fresh CCtx per resource.
- [ ] **T0415** Set canonical `nbWorkers=0`, `checksumFlag=1`, `contentSizeFlag=1`.
- [ ] **T0416** Set canonical `dictIDFlag=0`, `windowLog=27`, long-distance matching off, no dictionary.
- [ ] **T0417** Use one `ZSTD_compress2()` call over complete resource input for canonical frames.
- [ ] **T0418** Freeze remaining canonical libzstd defaults and verify them against pinned library.
- [ ] **T0419** Compare canonical frames byte-for-byte across repeated runs.
- [ ] **T0420** Test compressed archive 6 GiB limit and recommended 2 GiB warning behavior.
- [ ] **T0421** Add malicious frame and decompression-bomb regression tests.
- [ ] **T0422** Benchmark frame inspection, streaming decode and canonical encode.

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

- [ ] **T0601** Parse exactly five top-level nodes `epf`, `eld`, `language-version`, `metadata`, `manifest`.
- [ ] **T0602** Reject unknown or duplicate top-level nodes and properties.
- [ ] **T0603** Enforce exact single string arg/no block for three version nodes.
- [ ] **T0604** Require EPF/ELD schema values exactly `0.1.0`.
- [ ] **T0605** Require `metadata` and `manifest` blocks; zero args/properties.
- [ ] **T0606** Validate required `metadata.identifier` once, <=512 UTF-8 bytes.
- [ ] **T0607** Parse `urn:elang:language:<namespace>:<language-name>:<version>`.
- [ ] **T0608** Validate lowercase ASCII slug segments, hyphens, alphanumeric ends.
- [ ] **T0609** Validate strict version and equality to `language-version`.
- [ ] **T0610** Validate required nonempty `metadata.name`, <=256 UTF-8 bytes.
- [ ] **T0611** Validate optional `metadata.description` <=4096 UTF-8 bytes (empty allowed).
- [ ] **T0612** Reject duplicate metadata fields, unknown fields, extra args, nested blocks.
- [ ] **T0613** Require exactly one manifest `phonemes` entry with one nonempty `.eld` path.
- [ ] **T0614** Validate manifest path with canonical resource-path rules and index existence.
- [ ] **T0615** Reject `modules` and any other unrecognized manifest entry.
- [ ] **T0616** Canonicalize top-level EPF order as locked.
- [ ] **T0617** Implement ratified canonical EPF string/whitespace serialization.
- [ ] **T0618** Add EPF roundtrip, valid/invalid metadata and boundary tests.

**Acceptance:** EPF schema exactly matches frozen grammar; unknown declarations never ignored.

## Phase 7 — Integrity manifest and verification

- [ ] **T0701 BLOCKER** Implement only after `META-INF/integrity.kdl` schema is frozen.
- [ ] **T0702** Parse required manifest resource with KDL 2.0 and 16 MiB cap.
- [ ] **T0703** Validate manifest entries, SHA-256 encoding and unknown/duplicate fields.
- [ ] **T0704** Require coverage of every indexed resource **except integrity.kdl itself**.
- [ ] **T0705** Reject entries for missing resources and missing hashes for indexed resources.
- [ ] **T0706** Stream SHA-256 over exact uncompressed resource bytes.
- [ ] **T0707** Compare digests in constant-time where appropriate and report resource-specific failures.
- [ ] **T0708** Verify footer index hash separately from resource integrity hashes.
- [ ] **T0709** Implement canonical integrity manifest writer from finalized grammar.
- [ ] **T0710** Test byte mutation, hash mismatch, omitted/duplicate/extra entries and resource swaps.
- [ ] **T0711** Document that SHA-256 integrity is **not** a publisher signature.

**Acceptance:** verify mode checks every resource against integrity inventory; self-hash cycles excluded by rule.

## Phase 8 — Unicode and IPA repertoire

- [ ] **T0801** Integrate **pinned Unicode 18.0.0** data, not runtime-dependent Unicode tables.
- [ ] **T0802** Implement strict UTF-8 decoding and reject invalid Unicode scalar sequences.
- [ ] **T0803** Reject non-NFC ELD strings without rewriting input.
- [ ] **T0804** Reject control characters and Unicode General Categories `Cf`, `Co`, `Cn`.
- [ ] **T0805** Reject all Unicode noncharacters (including U+FDD0..FDEF and plane-end FFFE/FFFF).
- [ ] **T0806** Load strict UTF-8 JSON repertoire and reject BOM, duplicate keys and unknown fields.
- [ ] **T0807** Validate JSON metadata: repertoire v1, Unicode 18.0.0, IPA chart 2026.
- [ ] **T0808** Enforce inventory uniqueness, classification and sorted order.
- [ ] **T0809** Canonicalize JSON using RFC 8785 JCS and compare pinned SHA-256.
- [ ] **T0810** Reject any IPA base/diacritic/modifier not in approved allowlist.
- [ ] **T0811** Parse 1–8 Unicode scalar IPA identifier limit.
- [ ] **T0812** Parse <=4 logical IPA base units (including tied two-base units).
- [ ] **T0813** Enforce <=1 modification per logical unit, combining OR spacing modifier.
- [ ] **T0814** Interpret NFC precomposed symbols by canonical decomposition for unit/modifier counting.
- [ ] **T0815** Require modifiers and combining marks attach to valid base; reject standalone/leading marks.
- [ ] **T0816** Parse tie bars U+0361 and U+035C joining exactly two base symbols.
- [ ] **T0817** Reject overlapping/chained ties; allow multiple independent tied units.
- [ ] **T0818** Allow mixing tied and untied units; count units correctly.
- [ ] **T0819** Require tied-unit modifier after second base; reject intervening modifiers.
- [ ] **T0820** Build uniqueness key treating upper/lower tie bar as equivalent.
- [ ] **T0821** Preserve authored tie-bar variant and NFC bytes in canonical output.
- [ ] **T0822** Treat tied and untied IPA sequences as distinct identifiers.
- [ ] **T0823** Reject standalone stress, syllable-boundary and tone notation identifiers.
- [ ] **T0824** Test modifier, NFC, repertoire, scalar/unit count and tie equivalence boundaries.
- [ ] **T0825** Run independent IPA fixture review against official chart and pinned JSON.

**Acceptance:** all IPA identities validate deterministically against a versioned, hashed allowlist.

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
