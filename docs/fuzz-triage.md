# Fuzz triage (Phase 15 / Issue #25, T1519)

This page documents the end-to-end procedure that turns a fuzz
finding into a regression fixture. The triage directory itself
lives at [`fuzz/findings/`](../fuzz/findings/README.md); the
committed reproduction bytes live at
[`fuzz/regressions/`](../fuzz/regressions/README.md); the regression
tests that pin the fix live at
[`crates/elang-conformance/tests/fuzz_regressions.rs`](../crates/elang-conformance/tests/fuzz_regressions.rs).

## Why this is a separate page from `security-testing.md`

`docs/security-testing.md` describes the **preventive** side of
Phase 15: the 8 fuzz targets, the seed corpora, the CI integration,
and the audit checks (`cargo audit`, `cargo deny`, the workspace
lint policy). This page describes the **reactive** side: what
happens *after* libFuzzer reports a defect. Together the two pages
close the loop — preventive measures surface defects; reactive
measures turn those defects into permanent regression coverage.

## Triage lifecycle

```
libFuzzer finds crash
        │
        ▼
  fuzz/artifacts/<target>/crash-<sha1>     (gitignored)
        │
        ▼
  CI uploads as workflow artifact           (.github/workflows/fuzz.yml)
        │
        ▼  developer downloads
  fuzz/regressions/<target>/<NNNN>-<slug>.bin    (committed)
        │
        ▼
  fuzz/findings/<NNNN>-<slug>.md           (committed)
        │
        ▼
  crates/elang-conformance/tests/fuzz_regressions.rs::regression_<target>_<NNNN>  (committed)
        │
        ▼
  PR with the fix; T1519 ticked only after merge
```

The triage is not done until step 6. A finding without a committed
reproduction, a finding record, and a regression test is a finding
that will re-surface silently the next time the parser changes.

## Triage SLA

A finding reported by CI blocks the merge of any change that touches
the affected code path. The repository owner triages within 24
hours. A local-finding reported by a contributor (e.g., during a
extended fuzz session before a release) follows the same procedure
but is not a merge blocker.

## Triage discipline

These rules are non-negotiable:

- **No finding record without a committed reproduction.** A
  description of "the parser panicked on input X" without the exact
  bytes is a regression hazard — the next maintainer cannot
  reproduce, cannot verify, cannot fix.
- **No regression test that calls the wrong code path.** The test
  must mirror the fuzz target's call graph exactly. A regression
  test that calls `Foo::parse` while the fuzzer drove `Foo::decode`
  is a test that passes even when the defect re-emerges.
- **No finding marked `fixed` without a regression test.** The
  finding is closed only when the regression test fails on
  `origin/main` (no fix) and passes on the fix branch. Ticking T1519
  before this loop is closed is bookkeeping fraud.
- **No regression fixture that depends on environment state.** The
  test must not require a network, a temp dir, an env var, or a
  clock. libFuzzer-minimised crash artifacts are deterministic byte
  sequences — the regression test must be deterministic too.

## What "triage" looks like in practice

A worked example, using a hypothetical finding `0001`:

1. CI logs:
   ```
   fuzz smoke (container_structure) ... FAIL
   Crash artifact produced for container_structure
   ```
2. CI artifact `fuzz-artifacts-container_structure` (7-day
   retention) contains `crash-4f3a...1b`.
3. Developer downloads the artifact, copies the file to
   `fuzz/regressions/container_structure/0001-truncated-magic.bin`.
4. Reproduce locally:
   ```
   $ cargo +nightly fuzz run container_structure \
       fuzz/regressions/container_structure/0001-truncated-magic.bin
   ```
   libFuzzer reports `==ERROR: AddressSanitizer: heap-buffer-overflow
   on address 0x... at pc 0x... bp 0x... sp 0x...`. This is an
   OOB-read in `Header::decode`.
5. Open `fuzz/findings/0001-truncated-magic.md` with the full
   `AddressSanitizer` trace and the root-cause analysis: `Header::decode`
   indexes into `data` before checking `data.len() >= 32`.
6. Fix in PR #<pr>: add the `data.len() < HEADER_LEN` early return.
7. Add the regression test:
   ```rust
   #[test]
   fn regression_container_structure_0001() {
       let bytes = include_bytes!(
           "../../fuzz/regressions/container_structure/0001-truncated-magic.bin"
       );
       let result = elang_container::Header::decode(bytes);
       assert!(result.is_err(), "truncated magic must be rejected");
       assert_eq!(result.unwrap_err().category(), ErrorCategory::Input);
   }
   ```
8. Update `fuzz/findings/index.md` with the row.
9. Open the PR. The regression test fails on `origin/main` (the
   defect is unfixed) and passes on the branch (the fix is in
   place). T1519 is ticked once the PR merges.

## Current state

| Metric | Count |
|---|---|
| Findings triaged (lifetime) | 0 |
| Open findings | 0 |
| Fixed-and-regressed findings | 0 |
| Regression fixtures committed | 0 |

A zero count is a valid result. The nightly fuzz campaigns run on
the schedule in `.github/workflows/fuzz.yml` and have produced no
crash artifacts to date. The triage discipline above ensures that
when the first finding surfaces, the procedure is well-defined,
auditable, and immune to "we'll fix it later" drift.

## See also

- [`docs/security-testing.md`](./security-testing.md) — the
  preventive side: fuzz targets, seed corpora, CI integration.
- [`SECURITY.md`](../SECURITY.md) — the trust model and the
  fuzz budget policy.
- [`fuzz/findings/README.md`](../fuzz/findings/README.md) — the
  finding-record template and classifier.