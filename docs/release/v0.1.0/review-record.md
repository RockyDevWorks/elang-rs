# Independent review record — ELANG v0.1.0 (T1616)

> **Status: review complete, sign-off pending.** This document records
> the independent review of Phase 16 / Issue #26. The review was
> performed by three subagents, read-only, with no knowledge of how the
> code was written; every finding they raised is listed below with a
> disposition. A **human reviewer** must still sign the `[Sign-off]`
> block below before the `v0.1.0` release is declared (T1617 / T1620).
> The subagent review is evidence for that decision, not the decision.
>
> **41 findings are recorded below: 36 raised by the three subagents,
> one (#37) raised by the cross-platform CI run on the amended commit,
> and four (#38–#41) raised by the CodeRabbit review bot on PR #68,
> which is itself evidence that the review is not a formality.
> Every finding carries its source in the `Source` column.**
>
> Eleven further findings (#42–#52) were raised on PR #71, which added the
> release-artifact packaging script and the test-coverage expansion after
> this record was written. They are recorded separately in
> § "Follow-up review — PR #71".
>
> The subagent review is **one input** into the human sign-off. It is
> not a substitute for it. A human must read this document, read the
> code it points to, and decide for themselves whether the stated
> disposition for each finding is correct.

## Scope

In scope (this workstream):

- Everything the branch `doc/26-documentation-examples-benchmarks-and-release`
  changes versus `origin/main`. The full diff under review is
  `git diff origin/main...HEAD`.
- The public-facing behaviour of `elang` (the CLI), of
  `elang-reader`, of `elang-writer`, of the `gen_*` generators and of
  the conformance corpus, as exercised by the tests in
  `crates/elang-conformance/tests/` and `crates/elang-cli/tests/`.
- The prose in `docs/public/`, `docs/release/v0.1.0/`, `fixtures/CORPUS.md`,
  `docs/benchmarks.md`, `docs/peak-memory.md`, `docs/benchmarks.shape.*`,
  `docs/peak-memory.shape.*`, and the release notes
  `docs/release/v0.1.0/README.md`.

Out of scope (deliberately not reviewed here):

- The T1617 `v0.1.0` tag and the signed release artifacts (user handles them).
- The T1620 final release declaration (depends on T1616 + T1617).
- The signed-tag / code-signing / publisher-authenticity property
  itself: this branch ships **without** a publisher authenticity claim.
  See `docs/release/v0.1.0/README.md` § "Integrity vs. publisher
  authenticity" for what is and is not being asserted.
- Code outside the diff versus `origin/main`. The Phase 1–15 work is
  covered by prior reviews; this review does not re-open it.
- The release-artifact packaging script and the test-coverage expansion
  on PR #71. Those arrived after this record was written; their review
  evidence is recorded in § "Follow-up review — PR #71" below.
- The fuzz / nightly / differential / property-test surfaces. They are
  documented in `docs/security-testing.md` and exercised by
  `.github/workflows/fuzz.yml`; this review did not re-run them.

## Method

The review was performed by **three independent subagents**, each
spawned with a fresh context and no knowledge of how the code was
written. Each was given the worktree path and a precise, self-contained
brief. None was allowed to edit files.

| Subagent | Question they were asked | Review surface |
|---|---|---|
| Spec / conformance | "Does the branch do what every ticked task claims? Are the normative artifacts consistent with the docs? Does the release-gate pass?" | Tasks ↔ deliverables; normative manifest; OA-1..OA-17; doc claims; cross-references |
| Adversarial code | "Is the new Rust code correct, deterministic, panic-free on adversarial input, and free of nondeterminism in the benchmark harness?" | `crates/elang-cli/src/json.rs`, `main.rs` `--json` machinery, `benchmarks.rs`, `peak_memory.rs`, the `gen_*` bins, the new integration tests, `quickstart.rs`, `normative_manifest.rs` |
| Security / checklist | "Does every security claim the docs make hold up against the code? Where are the test gaps?" | `SECURITY.md`, `docs/security-limits.md`, the new tests, the workspace lint policy |

The subagents' transcripts are kept as evidence:

- Spec / conformance: `/opt/data/cache/delegation/live/deleg_c7b82a1f/task-0.log`
- Adversarial code: `/opt/data/cache/delegation/live/deleg_c7b82a1f/task-1.log`
- Security / checklist: `/opt/data/cache/delegation/done/deleg_c7b82a1f/task-2.log` (or `live/`)

A human reviewer should read those transcripts in full before signing.

## Findings

| # | Severity | Source | File:line | Finding | Disposition |
|---|---|---|---|---|---|

Findings table below. Disposition codes: `FIXED-AMEND` = corrected in the amended commit; `FIXED-LATER` = tracked in an open task; `ACCEPT` = the reviewer is right and we have decided to live with it; `REJECT` = the reviewer is mistaken (none here).

| # | Severity | Source | File:line | Finding | Disposition |
|---|----------|--------|-----------|---------|-------------|
| 1 | BLOCKER | spec | `.github/workflows/reproducible-build.yml:86-94,114-122,124-134` | The reproducible-build CI gate tarred `docs/benchmarks.{md,json}` and `docs/peak-memory.{md,json}` into both halves of a `diff -q` comparison. Those reports embed `generated_at_unix` and live timings/VmHWM and so cannot be byte-identical between runs. The job was guaranteed red. | **FIXED-AMEND.** The workflow now compares two `find | sort | xargs sha256sum` content manifests (`run-N.sha256`), not the tar bytes. The four shape files (`docs/{benchmarks,peak-memory}.shape.{md,json}`) are the byte-reproducible side; the measured reports are excluded from the comparison. |
| 2 | BLOCKER | spec | `docs/release/v0.1.0/README.md:140-142` | "Every generator is idempotent" is false: `gen_benchmarks` and `gen_peak_memory` always rewrite their outputs because the outputs carry timings. | **FIXED-AMEND.** Rewritten to state that *deterministic* generators are idempotent; the two *measured* generators are explicitly excluded and their `--shape` modes are described as the idempotent alternative. |
| 3 | MAJOR | spec | `fixtures/CORPUS.md:31-43` | The header claimed 60 published fixtures while the per-family bullets summed to 82; on disk there are 74 directories (31 accept + 43 reject) and the index's `published` count excludes the 13 canonical ELD goldens and the container reference archive (the OA-14 artefacts). | **FIXED-AMEND.** CORPUS.md now states both numbers and reconciles them: 74 on disk = 60 published (17 accept + 43 reject) + 14 OA-14 frozen by the normative manifest. |
| 4 | MAJOR | spec | `docs/release/v0.1.0/README.md:24` | T1604's deliverable pointer cited `specs/repertoire/`, a path that has never existed in the repository. The actual files are `specs/schema/ipa-v1.json` and `specs/schema/elang-ipa-repertoire-v1.schema.json`. | **FIXED-AMEND.** Pointer corrected. |
| 5 | MAJOR | spec | `docs/release/v0.1.0/migration-policy.md:18-19` | The "Repertoire" row pointed at `specs/registry/eld-canonical-vectors.json` (which holds canonical ELD goldens, not the repertoire digest). The "Normative manifest" row claimed the manifest carries a `corpus` field; it does not — that field lives in `fixtures/corpus_index.json`. | **FIXED-AMEND.** Both rows corrected; the corpus row was renamed from "Normative manifest" to "Corpus" and points at the correct file. |
| 6 | MAJOR | spec | `CHANGELOG.md:99-102` | Three errors in one paragraph: "39 OA-14 artifacts" (real count: 40), "with their `.sha256` sidecars" (sidecars are deliberately *not* frozen), and "Total artifact count grew from 32 to 71" (real prior count: 31). | **FIXED-AMEND.** All three corrected and the paragraph now cites the assertion that sidecars are excluded (`tests/normative_manifest.rs::derived_sha256_sidecars_are_not_frozen`). |
| 7 | MINOR | spec | `docs/release/v0.1.0/ARCHIVE.md:9,13,25,26` | Four broken relative links: three resolved one directory too high (`../../` → `../../../`) and one pointed at `reproducible-build-evidence.md`, a file written by CI at runtime. | **FIXED-AMEND.** Links corrected in `gen_release_archive`'s ARCHIVE.md template (so re-generation stays clean) and the runtime file is now stated as such. |
| 8 | MINOR | spec | `docs/release/v0.1.0/README.md:54` | Referenced `docs/release/v0.1.0/reproducible-build-evidence.md` as if it were a committed artifact. | **FIXED-AMEND.** Wording now describes the file as a CI-runtime artifact and notes it is absent from a source checkout. |
| 9 | MAJOR | adversarial | `crates/elang-cli/src/main.rs:185-192` | `elang --json` and `elang --json help` printed the human help text to stdout, violating the contract that JSON mode writes exactly one JSON object to stdout. | **FIXED-AMEND.** `Subcommand::Help` now emits the JSON envelope in `--json` mode and the help text only in human mode. A regression test (`json_help_emits_one_envelope_not_human_text`) pins the contract. |
| 10 | MAJOR | adversarial | `crates/elang-cli/src/main.rs:308` | `pack` failing on EPF parse used `diag!`, leaving `error.code = null` in the JSON envelope even though `e.code()` was available. | **FIXED-AMEND.** Switched to `diag_code!(e.code(), ...)`. New test `json_pack_failure_carries_stable_error_code` pins the contract. |
| 11 | MAJOR | adversarial | `crates/elang-cli/src/main.rs:323` | `pack` failing on ELD parse used `diag!` for the same reason. | **FIXED-AMEND.** Same fix and same kind of test (`json_canonicalize_eld_failure_carries_stable_error_code`). |
| 12 | MAJOR | adversarial | `crates/elang-cli/src/main.rs:346` | `pack` failing on the writer used `diag!` and embedded the code in the *message*; `error.code` was still `null`. | **FIXED-AMEND.** Switched to `diag_code!` with the code as the first argument and the code-driven message in `Display`. |
| 13 | MINOR | adversarial | `crates/elang-conformance/src/bin/gen_peak_memory.rs:569-590, 546-558` | `build_aggregate_size_claim` used `first_extra = 3`, but `package.epf` sorts *after* `data/...` (because `'p' > 'd'`), so the actual 4 mandatory entries are at indices 0–3. The code therefore inflated `META-INF/integrity.kdl` instead of a data extra. `build_oversized_resource_claim` used `len - 1` and the wrong comment — the last entry in path order is `package.epf`, not `data/claim.bin`. | **FIXED-AMEND.** Added `uncompressed_size_fields_with_paths` and `index_entry_for_path`; both builders now target entries *by path* and the intent is explicit in the code. |
| 14 | MINOR | adversarial | `crates/elang-conformance/src/benchmarks.rs:136` | For an even `ITERATIONS` count the median was `samples[len / 2]` (the upper of the two middles), biasing the figure high. | **FIXED-AMEND.** Now averages the two central order statistics when `samples.len()` is even. |
| 15 | DESIGN NOTE | adversarial | `crates/elang-conformance/src/bin/gen_benchmarks.rs` / `gen_peak_memory.rs` | The measured reports embed wall-clock time and RSS, so two runs are never byte-identical. | **FIXED-AMEND** (and partially pre-existing). Both generators now have a `--shape` mode whose outputs are byte-reproducible; the CI workflow compares the shape files, not the measured reports. |
| 16 | MINOR | adversarial | `crates/elang-conformance/src/bin/gen_conformance_corpus.rs:594-599` | `write_bytes` used `fs::write` unconditionally, contradicting the module doc's "Every write goes through `write_if_changed`" claim and contributing to the CI tar mtime drift. | **FIXED-AMEND.** `write_bytes` now compares the on-disk bytes against the new bytes and skips the rewrite when they match. |
| 17 | MAJOR | adversarial | `crates/elang-cli/tests/json_output.rs:144-173` | The "envelope is valid JSON" test was a hand-rolled brace counter that accepted two concatenated objects as "one envelope". | **FIXED-AMEND.** Replaced with `serde_json::Deserializer::from_slice(...).into_iter::<Value>()` that requires exactly one value and zero trailing bytes. Test suite grown from 7 to 12 (added `json_help_*`, `json_per_subcommand_help_emits_one_envelope`, `json_failure_envelope_round_trips_through_escaped_message`, and stricter per-subcommand failures). |
| 18 | MINOR | adversarial | `crates/elang-cli/tests/json_output.rs:48-59` | `field()` helper was escape-naive (`find('"')`). | **FIXED-AMEND.** Removed in the rewrite — assertions now navigate `serde_json::Value` directly. |
| 19 | MINOR | adversarial | `crates/elang-cli/tests/json_output.rs:109-112` | The stable-code assertion was `code.contains("HEADER_BAD_MAGIC") || code.contains("HEADER")`. The second arm would accept any future `HEADER_*` code. | **FIXED-AMEND.** Now strict equality: `field(&v, &["error", "code"]) == Value::String("HEADER_BAD_MAGIC")`. |
| 20 | MINOR | adversarial | `crates/elang-cli/tests/json_output.rs:119-132` | Only `verify` was exercised for the usage-error / null-code contract. | **FIXED-AMEND.** All seven subcommands are now exercised in `json_per_subcommand_help_emits_one_envelope`; usage-error coverage was extended in `json_usage_error_exits_two_with_null_code`. |
| 21 | MINOR | adversarial | `crates/elang-conformance/src/bin/gen_conformance_corpus.rs:170-175` | `split_id` does not sanitise `..` or absolute components; the corpus is safe today because fixture ids are in-crate constants. | **ACCEPT.** No reachable exploit path; documented for future hardening. |
| 22 | BLOCKER | security | `SECURITY.md:73-77` + `docs/security-limits.md:47` + `docs/public/security.md:52` | The reader was claimed to enforce a 6 GiB compressed-archive ceiling. It does not: `MAX_COMPRESSED_ARCHIVE_BYTES` is writer-only, and three security documents disagreed on this. | **FIXED-AMEND** (via documentation reconciliation). The three documents now uniformly state: the 6 GiB ceiling is **writer-side only**; the reader bounds expansion through the 8 GiB aggregate budget and the per-resource ceilings. Reader-side enforcement and the 2 GiB recommended-size warning are tracked as **T0420** (`REQ-FRAME-012`, status *Partial* in `docs/traceability.md`). The honest engineering call: enforce is the right path, but it is a new reader contract and an open task, not a regression of an existing one. |
| 23 | MAJOR | security | `docs/security-limits.md:202` | Cited `Header::archive_size()` as the API a caller pre-checks; the method does not exist. | **FIXED-AMEND.** Rewritten to say callers must check `bytes.len()` *before* invoking `Package::open`. |
| 24 | MAJOR | security | `crates/elang-core/src/limits.rs:18,20` | `MAX_INDEX_BYTES` (32 MiB) and `MIN_INDEX_BYTES` (16 B) were declared but unused; reader-side enforcement used the duplicate `footer::MAX_INDEX_SIZE` / `MIN_INDEX_SIZE`. | **FIXED-AMEND.** `Index::decode` (`crates/elang-container/src/index.rs`) now rejects an index above `MAX_INDEX_BYTES` with the registered `INDEX_SIZE_OUT_OF_RANGE` code, using `usize::try_from(...).unwrap_or(usize::MAX)` rather than an unchecked cast. Tests `index_decode_rejects_an_index_above_the_ceiling` and `index_decode_accepts_an_index_at_the_ceiling` pin the boundary. |
| 25 | MAJOR | security | `docs/security-limits.md:48` | `RECOMMENDED_COMPRESSED_ARCHIVE_BYTES` (2 GiB) was documented as emitting a diagnostic; the constant has no call sites. | **FIXED-AMEND.** The doc now states the diagnostic is **not implemented** and is deferred to the writer under T0420. |
| 26 | MAJOR | security | `crates/elang-core/src/limits.rs:22` and `crates/elang-container/src/frame.rs:50-58` | Two parallel constants both named "128 MiB": `limits::MAX_ZSTD_WINDOW_BYTES` (declared, unused) and `frame::MAX_WINDOW_BYTES = 1u64 << MAX_WINDOW_LOG` (enforced). | **FIXED-AMEND.** The frame layer's local constant is removed; enforcement now uses `MAX_ZSTD_WINDOW_BYTES` from `elang_core::limits` directly, and the doc comment explains that `MAX_WINDOW_LOG` is the bit-shift form of the same number. |
| 27 | MAJOR | security | `docs/security-limits.md:74-79` | Cited `ResourceEntry::reserve` as the cross-check mechanism; it has zero production call sites (only test callers). The real production cross-check is `ResourceEntry::new` (`crates/elang-core/src/index.rs`) and the layout-sum at `crates/elang-container/src/layout.rs:319-330`. | **FIXED-AMEND.** The paragraph now names the production sites and notes that `reserve` is the budget-charging helper exercised by tests, not the live path. |
| 28 | MAJOR | security | `.github/workflows/*` | `Cargo.lock.sha256` is committed but no CI workflow reads or asserts against it; a silent lockfile edit would go unnoticed. | **FIXED-AMEND.** The `reproducible-build` workflow now runs `sha256sum -c Cargo.lock.sha256` as an explicit step. The local repro script does the same. |
| 29 | MAJOR | security | `crates/elang-container/src/{index,header,footer}.rs`, `elang-unicode/src/nfc.rs` | Four production modules carry a module-level `#![allow(clippy::indexing_slicing)]`; the SECURITY.md wording overstated the guarantee. | **FIXED-AMEND** (in docs). SECURITY.md now states the actual lint posture: workspace lints are `warn`, CI escalates via `RUSTFLAGS = "-D warnings"`, six crates additionally carry `#![deny(unsafe_code)]`, and the four indexing-slicing modules' allows are explicitly listed and justified. |
| 30 | OVERSTATED | security | `SECURITY.md:34-36` | "`unsafe` is denied by lint" was false: workspace lint is `warn`, not `deny`. | **FIXED-AMEND.** SECURITY.md now states the warn + CI-escalation posture explicitly. |
| 31 | OVERSTATED | security | `docs/security-limits.md:108` | Claimed extraction used "O_NOFOLLOW semantics where the platform supports it"; the code has zero `O_NOFOLLOW` references. | **FIXED-AMEND.** Rewritten to describe the actual mechanism: `OpenOptions::create_new(true)` (`O_CREAT|O_EXCL`) on the target, and `symlink_metadata` + canonicalisation for every intermediate directory component. |
| 32 | MINOR | security | `docs/security-limits.md:53` | `Category::Zstd` does not exist; the actual category for `FRAME_WINDOW_TOO_LARGE` is `Category::Frame`. | **FIXED-AMEND.** Doc now names `Category::Frame` and the file:line that raises it. |
| 33 | MINOR | security | `crates/elang-cli/src/main.rs:565-602` | TOCTOU race between `symlink_metadata` and `create_dir` / `OpenOptions::create_new`. | **ACCEPT.** Documented in `docs/security-limits.md` "What the parser does NOT enforce" — requires a concurrent local writer. Closing the race fully would need `O_NOFOLLOW` on every level, which the platform support footnote above already explains is not done. |
| 34 | NIT | security | `crates/elang-cli/src/json.rs` | Tautological `PeakMemorySample::is_measured()`. | **ACCEPT.** Minor; the field is part of the type's API and the tautology does not affect correctness. |
| 35 | MINOR | adversarial | `crates/elang-conformance/src/normative_manifest.rs` | Rustdoc linked to private items (`ARTIFACTS`, `OA14_CASE_FILES`); CI `-D warnings` failed. | **PRE-EXISTING** (fixed in the same branch's prior commit). Re-verified: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` is clean. |
| 36 | DESIGN NOTE | adversarial | `Cargo.lock.sha256` not verified in CI | (See #28.) A separate reviewer flagged that `sha256sum -c` is absent; the reproducibility workflow's content-manifest comparison does not assert the lockfile digest. | **FIXED-AMEND** (see #28). |
| 37 | MAJOR | CI | `crates/elang-conformance/src/bin/gen_benchmarks.rs`, `crates/elang-conformance/src/bin/gen_peak_memory.rs` | The `--shape` reports were **not platform-independent**, which was the entire point of them. The resident-set columns rendered the sentinel only when the host had actually measured a value; on a host without `/proc/self/status` (macOS, Windows) the same column rendered an em-dash instead. The benchmark generator likewise emitted the "peak memory was not measured" prose instead of the measured-memory table, and its JSON emitted `{"measured":false}` instead of `{"measured":true,...}`. The committed shape was generated on Linux, so `cargo test --workspace --all-features` failed on the `macos-latest` and `windows-latest` CI runners while passing on `ubuntu-latest`. | **FIXED-AMEND.** In shape mode every resident-set column now carries `<MEASURED>` unconditionally, and the benchmark generator emits the measured-memory table and the `measured: true` object unconditionally. The "this platform cannot measure" fact remains available in the non-shape reports (via `memory_supported` and the methodology line), which is where it belongs. The shape is now identical on Linux, macOS and Windows. |

| 38 | MAJOR | CodeRabbit (PR #68) | `crates/elang-conformance/src/bin/gen_conformance_corpus.rs` | The `modes` array of a rejection record listed **every** mode down to the asserted level, including modes that *accept* the input. A digest mismatch fires at stage 11; `inspect` (pipeline ceiling 7) accepts such a package, yet the record listed `inspect`. An independent implementation reading the published corpus would have concluded that its `inspect` mode must reject five fixtures it never rejects. | **FIXED-AMEND.** `modes_for` was split into `modes_for_pass` (cumulative: every mode down to the asserted level) and `modes_for_fail` (exactly the modes whose `Level::ceiling_stage()` reaches the rejection stage). The corpus was regenerated, and `crates/elang-conformance/tests/corpus_publication.rs` now asserts the invariant against every published fixture. Verified on the data: `integrity/digest-mismatch` (stage 11) now lists `["verify","validate"]`, and the ELD document rejection at stage 12 lists `["validate"]`. |
| 39 | MINOR | CodeRabbit (PR #68) | `crates/elang-conformance/src/bin/gen_conformance_corpus.rs` | The published schema described `input` as "relative to the fixture directory" while the generator wrote **repository-root-relative** paths, and `golden[].path` used the fixture-directory base. An independent implementation following the schema text resolved `input` to a path that does not exist, and the two path fields used different bases. | **FIXED-AMEND.** The corpus was standardised on **fixture-directory-relative** paths — the convention every pre-existing, manifest-covered record already used (`gen_binary_vectors`, `gen_canonical_goldens`, `gen_example_package`). Only the Phase-16 corpus records changed; the OA-14 canonical goldens and the normative manifest are byte-identical, so `PINNED_SPEC_CHECKSUM` did not move. The schema descriptions, `fixtures/README.md` and the corpus index's `conventions` field now state the base explicitly, and `golden[].path` carries a description it previously lacked. |
| 40 | MINOR | CodeRabbit (PR #68) | `crates/elang-conformance/tests/corpus_publication.rs` | The module doc claimed the drift test re-asserted that "the regenerated index matches the committed copy byte for byte". No test compared the index bytes; the test only rebuilt an in-memory table. The documentation overstated what CI enforced. | **FIXED-AMEND.** The drift test now does what the doc claimed: it re-runs the real generator with its working directory pointed at a scratch tree under `target/` and compares **every file it emits** byte-for-byte against the committed corpus. The doc was rewritten to describe the checks that exist. A generator change that was not followed by a regeneration now fails CI with the exact file that drifted. |
| 41 | MAJOR | CodeRabbit (PR #68) | `crates/elang-conformance/tests/example_package.rs` | The CLI-regeneration test ran `gen_example_package`, which **overwrote the published artefacts** (`examples/hello-en/hello-en.elang`, the `.sha256` sidecar and `expected.json`) in the working tree during `cargo test`. A committed artefact that had drifted would be silently "repaired" before another test read it, and two concurrently-running tests could read the archive mid-write. | **FIXED-AMEND.** `gen_example_package` now accepts `--out-dir <path>` and writes its three artefacts there; the test points it at a scratch directory, compares the produced bytes against the committed ones, and asserts the repository was left untouched. The generator also became idempotent (`write_if_changed`), matching the other generators. |


## Release-gate assessment

`docs/release-strategy.md` states four conditions for a v0.1.0 release.
This review assessed each against the branch state and reports the
answer honestly, including the ones that fail.

| Gate condition | Verdict | Evidence and gaps |
|---|---|---|
| (a) No open normative artifact remains | **NOT SATISFIED** | `docs/open-artifacts.md` records OA-17 as *Mechanism landed; freeze/tag still open*. The freeze declaration and the `v0.1.0` tag do not exist, and `T0123` is correctly left unticked for that reason. This is by design — the tag is T1617 and is the user's side of the gate. |
| (b) Every MUST has passing test coverage | **NOT SATISFIED** | `docs/traceability.md` still carries non-Done rows for MUST / MUST NOT requirements: `REQ-SEC-001` (Planned), `REQ-CLI-003` (Planned), `REQ-CLI-001` (Partial), `REQ-LIMIT-002` (Scaffolded), `REQ-LIMIT-008` (Partial), `REQ-LIMIT-010` (Scaffolded). Their tasks (`T0510`, `T1319`, `T0420`) are unticked. **The most relevant to this branch is `REQ-FRAME-012` (6 GiB compressed ceiling + 2 GiB warning), which is *Partial* and tracked as `T0420`;** the three security documents have been reconciled to state that honestly rather than overclaiming (finding #22). |
| (c) Canonical bytes reproduce on all platforms | **PARTIAL** | `cargo test --workspace --all-features` — including the canonical-golden digest tests — runs on ubuntu/windows/macos in `.github/workflows/ci.yml`, so golden digests are exercised on three OSes. But the `reproducible-build` job itself runs on `ubuntu-24.04` only, and the two-pass clean-build comparison is therefore single-platform. Multi-platform *reproducible-build* is not proven by this branch. |
| (d) Malformed inputs fail closed | **SATISFIED for the covered set** | All 43 on-disk `fixtures/invalid/**` fixtures carry a `.sha256` sidecar and an `expected.json` naming a stage and error code; the rejection codes cited in `docs/peak-memory.md` (`INDEX_SIZE_OVER_CEILING`, `LIMIT_UNCOMPRESSED_AGGREGATE`, `INDEX_COUNT_OUT_OF_RANGE`) exist in `specs/registry/error-codes.json`. The full workspace suite is green (1410 passed, 1 ignored, 71 suites). |

**Overall:** conditions (b) and (c) are not met by this branch and were
not expected to be — they are the standing v0.1.0 backlog tracked in
`docs/traceability.md` and the open task list. Condition (a) closes only
when the human tags. Condition (d) holds for the covered input set.

This review therefore **does not** assert that `v0.1.0` is release-ready.
It asserts that the Phase 16 branch does what its ticked tasks claim,
that the findings the reviewers raised have been dispositioned, and that
the documents now describe the implementation truthfully.

## Residual risks (what this review does NOT prove)

This branch and its review do not produce, and do not claim to
produce, the following properties:

1. **Publisher authenticity.** No signature, certificate, transparency
   log or distribution-platform listing is produced. A downstream
   consumer must rely on out-of-band trust to attribute the bytes to
   this repository. SHA-256 integrity is tamper-evidence, not
   tamper-prevention.
2. **Performance on worst-case inputs.** The published benchmarks
   measure one canonical example input (the `hello-en` package). They
   do not characterise performance at the documented ceilings (1 GiB
   per resource, 25,000 resources, 8 GiB aggregate).
3. **Behaviour on non-Linux platforms.** Peak-memory numbers are
   Linux-only (the `VmHWM` sample comes from `/proc/self/status`).
   The shape files are portable, but the measured reports only ship
   for Linux; non-Linux positions are stated explicitly.
4. **A complete enumeration of every malformed-input class.** The
   conformance corpus covers the KDL 2.0 surfaces and the ELD
   rejection paths; the fuzz harnesses extend the surface with
   coverage-guided adversarial input. Together they exercise a large
   but bounded set of inputs — a proof of correctness would require
   a formal model.
5. **A re-review of phases 1–15.** The diff versus `origin/main` is
   what was reviewed. Earlier work is reviewed by the prior phases'
   records.
6. **That the signed-off state survives future changes.** The
   `v0.1.0` tag will pin a commit; the review record describes that
   commit. A future change to the pinned commit reopens the review.

## Follow-up review — PR #71

The release-artifact packaging script
(`scripts/build-release-artifacts.sh`) and the test-coverage expansion
that raised the workspace's Codecov-equivalent line coverage to 99.09%
were added on PR #71, after this record was written. They were reviewed
by the CodeRabbit review bot, which raised **eleven findings**:
**seven** on the coverage work and **four** on the packaging script. Every one was
addressed and the thread resolved; the dispositions are:

| # | Source | Finding | Disposition |
|---|---|---|---|
| 42 | CodeRabbit (PR #71) | `crates/elang-cli/tests/error_paths.rs` imported `std::os::unix::ffi::OsStrExt` without a platform guard, so the Windows CI job failed to compile the test target. | **Fixed** — the import, the `non_utf8_arg` helper and the four tests that use it are `#[cfg(unix)]`; the POSIX symlink and permission-mode tests are `#[cfg(unix)]`; the file-size-limit test is `#[cfg(target_os = "linux")]`, because macOS kills the process with `SIGXFSZ` where Linux returns `EFBIG`. |
| 43 | CodeRabbit (PR #71) | The `0555`-directory setup cannot fail for a privileged process, so the test asserted an outcome the environment cannot produce. | **Fixed** — the test probes the directory and skips itself when the process bypasses the mode bits. |
| 44 | CodeRabbit (PR #71) | Temporary paths were interpolated into a shell script; a path containing an apostrophe would break the quote. | **Fixed** — the paths are passed to `sh -c` as positional parameters. |
| 45 | CodeRabbit (PR #71) | The peak-memory overflow test omitted `VmRSS` and `VmPeak`, so it also passed on the missing-field branch and did not isolate the overflow. | **Fixed** — both fields now carry valid values. |
| 46 | CodeRabbit (PR #71) | The attribution tests' scratch directory was not removed by its callers. | **Fixed** — a drop guard removes it, including when an assertion fails, and the directory name carries a per-process counter so parallel tests cannot race. |
| 47 | CodeRabbit (PR #71) | The repertoire generator test could be satisfied by a stale output file from an earlier run. | **Fixed** — the output is removed before the generator runs. |
| 48 | CodeRabbit (PR #71) | The packaging script's `--out-dir` guard checked whether the *archive* was ignored; a `*.tar.gz` glob leaves the `.sha256` sidecar unignored. | **Fixed** — the guard checks the output *directory*. |
| 49 | CodeRabbit (PR #71) | The sidecar digest was interpolated into the `printf` that wrote it, so a hashing failure produced an empty digest and a successful build. | **Fixed** — the digest is captured first and the function returns non-zero on failure. |
| 50 | CodeRabbit (PR #71) | `--verify` proved two scratch builds identical but delivered a third build without comparing it. | **Fixed** — the delivered artifact is re-hashed and compared with the verified digest. |
| 51 | CodeRabbit (PR #71) | The member comparison cannot compare a path whose name contains a newline: `tar -tzf` escapes the newline while `tr` splits the NUL list on newlines, so a correct archive is reported as a mismatch. Git's index can legally hold such a name. | **Fixed** — the script inspects each NUL-delimited path and refuses to build, naming the offending path, instead of failing later with a misleading mismatch. |
| 52 | CodeRabbit (PR #71) | The peak-memory test accepted `None` on Linux, so a regression in `current()` would ship a silently unmeasured report without the test noticing. | **Fixed** — on Linux the sample must be present and measured; on platforms without `/proc/self/status` the optional sample stays optional. |

This follow-up section is **bot review only**. It is not a substitute
for the human sign-off below, and it does not reopen the dispositions
recorded above for Phase 16.

## Sign-off

> A human reviewer must sign here before T1617 / T1620 close.
>
> I have read this record in full. I have either read or delegated
> the reading of the subagent transcripts listed under § Method. I
> agree with the disposition of every finding listed above, or I
> have written a note beside any I disagree with. I understand the
> residual risks in § "Residual risks" and I am content to ship
> `v0.1.0` with them.

| Role | Name | Date | Signature |
|---|---|---|---|
| Reviewer |  |  |  |
| Co-reviewer (optional) |  |  |  |

## See also

- `docs/release/v0.1.0/security-checklist.md` — the security checklist
  that complements this review record.
- `docs/release/v0.1.0/README.md` — the release notes.
- `docs/release/v0.1.0/migration-policy.md` — the version-compatibility
  policy.
- `SECURITY.md` — the threat model.
- `docs/security-limits.md` — the per-limit table.
