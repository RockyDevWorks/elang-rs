# Fuzz regression fixtures (Phase 15 / Issue #25, T1519)

This directory holds the committed byte sequences that libFuzzer
identified as triggering a defect in the parser under one of the
targets in `fuzz/fuzz_targets/`. Every committed fixture has a
corresponding row in [`fuzz/findings/index.md`](../findings/index.md)
and a dedicated test in
[`crates/elang-conformance/tests/fuzz_regressions.rs`](../../crates/elang-conformance/tests/fuzz_regressions.rs).

## Layout

```
fuzz/regressions/
  <target-name>/
    <NNNN>-<short-slug>.bin        # raw bytes the fuzzer found
    <NNNN>-<short-slug>.meta.json  # libFuzzer metadata (optional)
```

`<target-name>` matches the cargo-fuzz binary name (e.g.
`container_structure`, `resource_path`). `<NNNN>` is the
monotonically increasing finding number, zero-padded to four digits,
shared with `fuzz/findings/<NNNN>-<slug>.md`.

## Adding a new regression fixture

1. libFuzzer finds a crash and writes `crash-<sha1>` to
   `fuzz/artifacts/<target>/`. CI's `upload-artifacts-on-failure`
   step in `.github/workflows/fuzz.yml` uploads it as the workflow
   artifact `fuzz-artifacts-<target>` (7-day retention).
2. Download the artifact, copy the `crash-<sha1>` file into this
   directory as `<NNNN>-<slug>.bin`.
3. Reproduce locally:
   ```
   cargo +nightly fuzz run <target> fuzz/regressions/<target>/<NNNN>-<slug>.bin
   ```
   Confirm the same crash class is reported.
4. Write the finding record at
   `fuzz/findings/<NNNN>-<slug>.md` following the template in
   [`fuzz/findings/README.md`](../findings/README.md).
5. Add the regression test to
   `crates/elang-conformance/tests/fuzz_regressions.rs`:
   ```rust
   #[test]
   fn regression_<target>_<NNNN>() {
       let bytes = include_bytes!(
           "../../fuzz/regressions/<target>/<NNNN>-<slug>.bin"
       );
       let _ = <call the parser>; // must not panic
   }
   ```
6. Add the row to `fuzz/findings/index.md`.
7. Open a PR. The regression test fails on `origin/main` (because
   the fix is on the branch) and passes once the fix lands. Tick
   T1519 only after this PR merges.

## Why committed, not git-ignored

`fuzz/.gitignore` ignores `corpus/`, `artifacts/`, and `coverage/`
because they grow without bound during a fuzz run and carry no
audit value. `regressions/` is the opposite: every file is small,
named, and tied to a finding record. Reproducibility is the entire
point — a regression fixture that lives only on someone's laptop
is not a regression fixture.

## Current state

No regression fixtures committed yet. The local fuzz campaigns
recorded in `fuzz/findings/index.md` (currently empty) found 0
findings in 240 wall-clock seconds across the two targets on
`origin/main` (`container_structure`, `resource_path`). When the
first finding surfaces, this directory grows its first
`<target>/<NNNN>-<slug>.bin`.