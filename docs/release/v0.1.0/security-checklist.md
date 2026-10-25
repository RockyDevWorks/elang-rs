# Security checklist — ELANG v0.1.0 (T1616)

> **Status: review complete, sign-off pending.** Every claim below was
> checked against the code by an independent reviewer and the result is
> recorded. A human reviewer must still sign the `[Sign-off]` block
> below before the `v0.1.0` release is declared (T1617 / T1620).
>
> Every row below corresponds to a security claim made by the
> project's docs (`SECURITY.md`, `docs/security-limits.md`,
> `docs/public/security.md`, the release notes) and to the code or
> test that backs it. "VERIFIED" means the cited code or test
> substantiates the claim; "OVERSTATED" means the docs go further
> than the evidence; "FALSE" means the claim is wrong; "PENDING"
> means the reviewer must check it.
>
> The reviewer should re-execute or re-read the cited evidence for
> every row they mark as accepted.

## Threat-model claims

| # | Claim | Where claimed | Evidence in code/tests | Status | Reviewer |
|---|---|---|---|---|---|
| T1 | Untrusted input is parsed under bounded budgets (per-resource and aggregate 8 GiB). | `SECURITY.md:11–13`, `docs/security-limits.md:40–58` | `MAX_UNCOMPRESSED_AGGREGATE_BYTES = 8 GiB` in `crates/elang-core/src/limits.rs:17`; aggregate counter charges each decoded resource | VERIFIED |  |
| T2 | Zstd window is capped at 128 MiB; dictionary, skippable, concatenated, truncated and trailing-byte frames are rejected. | `SECURITY.md:19–21`, `docs/security-limits.md:53`, `docs/public/format/container.md` | `MAX_ZSTD_WINDOW_BYTES = 128 MiB` in `crates/elang-core/src/limits.rs:22`, now the single source of truth: the frame layer's duplicate `MAX_WINDOW_BYTES` was removed and `crates/elang-container/src/frame.rs` enforces the `limits.rs` constant directly. All five rejection cases have tests in `crates/elang-container/src/frame/tests.rs`: skippable magic (:70, and the high skippable variant :79), dictionary dependency (:130, and :139), truncated header/body (:159), concatenated frames (:169), trailing bytes (:178 — reports the same `FRAME_CONCATENATED` code as concatenation, documented at `frame.rs:165-166`). | VERIFIED — all five rejection cases have a named test; window-cap constant consolidated |  |
| T3 | Resource paths are NFC UTF-8 relative paths only; no absolute/drive paths, no `.`/`..`/empty segments, no NUL or backslash; extraction rejects overwrite, traversal and symlink escape. | `SECURITY.md:14–16`, `docs/security-limits.md:81–110` | Path validator: `crates/elang-core/src/ids.rs` raises `PATH_NUL` (:63), `PATH_EMPTY_SEGMENT` (:69), `PATH_ABSOLUTE` (:43, :51, :227, :231), `PATH_BACKSLASH` (:58, :235), `PATH_TOO_LONG` (:36); NFC is gated upstream by `crates/elang-unicode/src/lib.rs:103` (`is_nfc`). Extraction: `crates/elang-cli/src/main.rs` — `run_unpack` (:611) canonicalises the output dir to an absolute `base` (:651), rejects a target that is not under `base` (`create_safe_parent`, :568–571), walks each parent component and rejects a symlink whose canonical path escapes `base` (:576–587), refuses to write through a symlinked target (:668–672), and refuses to overwrite an existing file without `--force` (:673–676). | VERIFIED — both the validator and the extractor were read against the claims |  |
| T4 | Parser enforces nesting-depth, token, string, node, comment and reference budgets *during* parsing. | `SECURITY.md:17–18`, `docs/security-limits.md:111–137` | `BudgetTracker` in `crates/elang-kdl/src/budget.rs`; per-counter constants in `crates/elang-core/src/limits.rs:24–49`. Fuzz target `kdl_parser` exercises the counter | VERIFIED — the KDL path; the semantic-reference path (S1) is reserved-but-not-enforced |  |
| T5 | SHA-256 integrity is **not** a publisher signature / authenticity claim. | `SECURITY.md:25–28`, `docs/security-limits.md:223–280`, `docs/public/security.md`, `docs/release/v0.1.0/README.md:49–76`, `docs/public/cli.md` | Stated in all five documents; release README explicitly lists the absent claims (no signing, no IANA, no URN ownership) | VERIFIED (this branch, all five places) |  |

## Memory-safety claims

| # | Claim | Where claimed | Evidence in code/tests | Status | Reviewer |
|---|---|---|---|---|---|
| M1 | `unsafe` is denied by lint except at audited FFI boundaries. | `SECURITY.md:34–36` | Workspace `unsafe_code = "warn"` at `Cargo.toml:103`; CI escalates to deny via `cargo clippy … -D warnings`. Crate-level `#![deny(unsafe_code)]` in `elang-kdl`, `elang-epf`, `elang-integrity`, `elang-writer`, `elang-reader`, `elang-eld` (`crates/elang-*/src/lib.rs`). The single FFI `unsafe` block is at `crates/elang-container/src/frame.rs:497` (`ZSTD_getErrorCode`), with a documented SAFETY comment and a per-site `#[allow(unsafe_code, reason = "…")]` | OVERSTATED — the lint level in the workspace is `warn`, not `deny`; only six of the twelve crates layer an additional `#![deny(unsafe_code)]`, and `elang-container` — the crate holding the one FFI `unsafe` block — is not one of them. The CI escalation makes it effectively denied at PR-time. The single `unsafe` block is justified and reviewed. **FIXED-AMEND:** `SECURITY.md:34-36` now states the warn + CI-escalation posture, names the six denying crates, and says explicitly that `elang-container` relies on the CI escalation plus a scoped `#[allow(unsafe_code, reason = "...")]` |  |
| M2 | No `unwrap()`/`expect()`/`panic!`/unchecked indexing on untrusted input, enforced by workspace clippy lints. | `SECURITY.md:37–38` | Workspace `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` set to `warn` in `Cargo.toml:107–115`; CI escalates via `-D warnings`. `#![allow(...)]` attributes are scoped to bin / fuzz / test code only | OVERSTATED — same story as M1: `warn` in the workspace, denied by CI. Four production modules that parse untrusted bytes carry a scoped module-level `#![allow(clippy::indexing_slicing)]` with a written justification (`elang-container/src/{index,header,footer}.rs`, `elang-unicode/src/nfc.rs`); in those modules the no-unchecked-indexing property is upheld by review and bounded-slice style, not by the lint. **FIXED-AMEND:** `SECURITY.md:37-38` and `docs/public/security.md:70-71` now state this instead of implying a blanket lint guarantee |  |
| M3 | Checked integer and offset arithmetic throughout; `overflow-checks` are on in all profiles. | `SECURITY.md:39–40`, `docs/security-limits.md:39` | `overflow-checks = true` in `[profile.release]` and `[profile.dev]` at `Cargo.toml:127,131`. `checked_*` helpers in `crates/elang-core/src/checked.rs` | VERIFIED |  |
| M4 | The `fuzz/` crate (excluded from `--workspace`) follows a different lint policy; its bounds are exercised by `.github/workflows/fuzz.yml`, not by the `cargo clippy --workspace` gate. | `Cargo.toml:14–19` | Comment in `Cargo.toml`; `fuzz` listed under `exclude` | VERIFIED — but note this means the workspace gate does not cover fuzz; the fuzz workflow does |  |

## Container-ceiling claims

| # | Claim | Where claimed | Evidence in code/tests | Status | Reviewer |
|---|---|---|---|---|---|
| C1 | `MAX_COMPRESSED_ARCHIVE_BYTES = 6 GiB`, enforced on writer side only; reader does not pre-cap on this. | `docs/security-limits.md:47`, `SECURITY.md:73-77` | `crates/elang-writer/src/build.rs` rejects an over-ceiling archive with `LIMIT_ARCHIVE_BYTES`; the reader has **no** compressed-size check, and the CLI's `read_archive_file` performs an unbounded `fs::read`. `tests::ceilings_have_expected_values` asserts the constant value. | OVERSTATED in two of three documents — `docs/public/security.md:52` correctly said the reader does not apply it while `SECURITY.md` and `docs/security-limits.md` implied it did. FIXED-AMEND: all three now state the ceiling is writer-side only, that the reader bounds expansion through the 8 GiB aggregate budget and the per-resource ceilings, and that reader-side enforcement plus the 2 GiB warning are tracked as **T0420** (`REQ-FRAME-012`, *Partial*). |  |
| C2 | `MAX_UNCOMPRESSED_AGGREGATE_BYTES = 8 GiB`, charged during decode, fails closed. | `docs/security-limits.md:49` | `ResourceEntry::reserve` at `crates/elang-core/src/index.rs:135–138` maps the budget overrun to `Category::Limit`; `BoundedCounter::record` at `crates/elang-core/src/bounded.rs:84–91` rejects overrun with `Category::Frame` / `DECOMPRESSION_BUDGET_EXCEEDED`. Other sites: `ResourceIndex::new`, `crates/elang-container/src/frame.rs:402,416`, `crates/elang-container/src/layout.rs:327`, `crates/elang-writer/src/build.rs:102–106`. **The doc row's wording "clamps to ceiling" is imprecise — the implementation rejects rather than truncates.** | VERIFIED with one wording nit (see the review record) |  |
| C3 | `MAX_RESOURCES = 25,000`, enforced at reader stage 5 (index parse) and at the writer. | `docs/security-limits.md:50` | Reader stage 5: `Index::decode` at `crates/elang-container/src/index.rs:278`, **before the entry loop**. `Index::with_entries` at `:220`. `ResourceIndex::new` at `crates/elang-core/src/index.rs:165`. Writer: `crates/elang-writer/src/build.rs:281`. | VERIFIED — the reader check is before any per-resource work |  |
| C4 | `MAX_INDEX_BYTES = 32 MiB`, `MIN_INDEX_BYTES = 16`. | `docs/security-limits.md:51–52` | Reader stage 5 (`Index::decode` in `crates/elang-container/src/index.rs`). The lower bound is enforced implicitly (`bytes.len() < INDEX_HEADER_LEN`, and `INDEX_HEADER_LEN == MIN_INDEX_BYTES == 16`). The upper bound is enforced by an explicit `INDEX_SIZE_OUT_OF_RANGE` check added by this review. | FIXED-AMEND — see the review record. **Before this review the 32 MiB ceiling was declared in `limits.rs` and asserted by `tests::ceilings_have_expected_values`, but no code path ever compared an index length against it; the doc row claiming "index-too-large code" was therefore aspirational, not factual. The check and two tests (`index_decode_rejects_an_index_above_the_ceiling`, `index_decode_accepts_an_index_at_the_ceiling`) were added to `crates/elang-container/src/index.rs`.** |  |
| C5 | `MAX_GENERIC_RESOURCE_BYTES = 1 GiB`, `MAX_EPF_BYTES = 1 MiB`, `MAX_INTEGRITY_BYTES = 16 MiB`, `MAX_ELD_BYTES = 1 GiB` — each declared size is cross-checked against the budget *before* decoding. | `docs/security-limits.md:64–71`, `docs/public/security.md` | The declared-size cross-check runs in `ResourceEntry::new` (`crates/elang-core/src/index.rs:72`), which rejects an over-ceiling declared uncompressed size before decoding, and the archive-level aggregate is summed at `crates/elang-container/src/layout.rs:319-330`. (`ResourceEntry::reserve` is the `ByteBudget`-charging helper; it has no production call sites and the doc previously cited it as the live mechanism — corrected.) Per-resource ceilings: `MAX_EPF_BYTES` at `crates/elang-epf/src/parse.rs:82` (before parse); `MAX_INTEGRITY_BYTES` at `crates/elang-integrity/src/parse.rs:85`; `MAX_GENERIC_RESOURCE_BYTES` at `crates/elang-container/src/layout.rs:319` and `frame.rs:415`; `MAX_ELD_BYTES` passed as the cap to `decode_indexed_resource` at `crates/elang-reader/src/stages.rs:174`; `MAX_BOOTSTRAP_RESOURCE_BYTES` enforced at `crates/elang-reader/src/bootstrap.rs` and `crates/elang-writer/src/build.rs:197` | VERIFIED — cross-checks happen before decode at every enforcement site |  |

## KDL parser-budget claims

| # | Claim | Where claimed | Evidence in code/tests | Status | Reviewer |
|---|---|---|---|---|---|
| K1 | Every KDL parser charges the budget *during* parsing, before unbounded allocation. | `docs/security-limits.md:111–137` | `BudgetTracker` in `crates/elang-kdl/src/budget.rs`; per-counter constants in `crates/elang-core/src/limits.rs:24–49`; `BudgetTracker::nodes_archive()` is incremented per successful node; fuzz target `kdl_parser` exercises the counter | VERIFIED — the KDL path enforces every counter during parsing. The *semantic* (ELD) path is a separate, reserved-but-not-enforced gap: see S1 |  |

## Semantic-budget claims

| # | Claim | Where claimed | Evidence in code/tests | Status | Reviewer |
|---|---|---|---|---|---|
| S1 | `MAX_SEMANTIC_REFERENCES = 10,000,000` and `MAX_SEMANTIC_TRAVERSAL_DEPTH = 128` are wired into `elang-core::Error` but not yet enforced by ELD's semantic phase. | `docs/security-limits.md:140–148` | The constants exist in `limits.rs:50–51` and have no call sites outside `limits.rs`; `crates/elang-conformance/src/requirement_coverage_data.rs:331-352` records that "nothing in the workspace currently enforces them at semantic walk time". The doc says "reserved for ELD's semantic phase" | VERIFIED — the gap is real and the doc accurately calls it out as reserved-but-not-enforced. Phase 16 touched no ELD semantic code |  |
| S2 | `MAX_RETAINED_DIAGNOSTICS = 100` truncates warnings but **never** errors. | `docs/security-limits.md:148–152` | `crates/elang-core/src/error.rs:201` — `if self.retained.len() < MAX_RETAINED_DIAGNOSTICS` appends, otherwise it drops the entry while incrementing a separate total (`:182`, "records whether any were dropped"). Unit test at `error.rs:283-291` pushes 150 diagnostics and asserts `retained().len() == 100` and `total() == 150`. | VERIFIED — truncation without error, with a test that fails if the cap changes |  |

## "What the parser does NOT enforce" claims

The following non-goals are asserted in `docs/security-limits.md:154–221`
and repeated in `SECURITY.md:80–94`. A defensive deployment must layer
them above the reader.

| # | Non-goal | Doc status | Re-verified for this release |
|---|---|---|---|
| N1 | No rate limiting | Stated | Re-stated; nothing in Phase 16 changes it |
| N2 | No resource-type detection beyond the path | Stated | Re-stated |
| N3 | No streaming verification | Stated | Re-stated |
| N4 | No sandboxing | Stated | Re-stated |
| N5 | No resource budget at the API layer | Stated | Re-stated |
| N6 | No concurrency model (single-threaded) | Stated | Re-stated |
| N7 | No persistence of rejected inputs | Stated | Re-stated |

## Authenticity non-claims

| # | Claim | Where claimed | Status |
|---|---|---|---|
| A1 | The release ships **without** a publisher authenticity claim (no signing, no IANA-registered media type, no URN-ownership claim, no distribution-platform listing). | `docs/release/v0.1.0/README.md:60–76` | VERIFIED — the README explicitly enumerates each absent claim |
| A2 | libzstd is pinned to **1.5.7** for canonical reproducibility, not as a security claim about the library itself. | `SECURITY.md:46`, `docs/security-limits.md:271–280` | `PINNED_LIBZSTD_VERSION = 10_507` at `crates/elang-container/src/frame.rs:70`; `zstd-sys = "=2.1.2"` pinned in the root `Cargo.toml:68`. Two tests in `crates/elang-container/src/frame/tests.rs` — `pinned_zstd_constant_is_157` (:24) and `linked_libzstd_is_157` (:29) — the second reading the *actual* linked libzstd version at runtime, so a dependency bump that changes the bundled library fails the suite rather than silently changing canonical bytes | VERIFIED — the pinned constant and the runtime-linked version are both asserted |  |
| A3 | The conformance corpus is **not exhaustive**; fuzz extends coverage but is bounded by the CI wall-clock budget. | `docs/security-limits.md:282–290`, `docs/security-testing.md` | VERIFIED — the caveat is in both docs |

## Test adequacy (independent reviewer focus)

The independent reviewer was asked specifically whether:

- the corpus-publication test fails if a fixture is missing,
- a benchmark nondeterminism regression would be caught,
- the `--json` envelope's `exit_code` matches the process exit code,
- the normative-manifest test fails if an OA-14 artifact is omitted.

Their findings are folded into the rows above (T/C/M/S) and into the
independent review record at `docs/release/v0.1.0/review-record.md`.
The reviewer should mark each row above as accepted only after
checking the cited evidence themselves.

## Sign-off

> A human reviewer must sign here before T1617 / T1620 close.
>
> I have re-checked every row marked VERIFIED that I have not
> personally reviewed, and I have read the evidence cited for every
> row marked FIXED-AMEND or OVERSTATED. I agree with the disposition
> of each finding in `docs/release/v0.1.0/review-record.md`, or I have
> written a note beside any I disagree with. I understand that the
> review's release-gate assessment reports conditions (b) and (c) as
> **not met**, and I am content to ship `v0.1.0` with the residual
> risks in `docs/release/v0.1.0/review-record.md` § "Residual risks".

| Role | Name | Date | Signature |
|---|---|---|---|
| Security reviewer |  |  |  |
| Co-reviewer (optional) |  |  |  |

## See also

- `docs/release/v0.1.0/review-record.md`
- `SECURITY.md`
- `docs/security-limits.md`
- `docs/security-testing.md`
