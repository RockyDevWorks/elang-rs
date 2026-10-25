# Fuzz findings triage (Phase 15 / Issue #25, T1519)

This directory holds the triage records for every finding produced by the
8 fuzz targets under `fuzz/fuzz_targets/`. A finding is a crash,
out-of-bounds read, hang, or assertion failure that libFuzzer surfaces
when it mutates an input into something the target binary cannot handle
without violating its safety contract.

## What gets a finding

A fuzz finding has one of these root causes:

- **Panic** — the harness `fuzz_target!` body panics (rust panic,
  assertion failure, `unwrap()`, integer overflow in `debug-assertions`
  mode, division by zero, etc.). `cargo +nightly fuzz run` reports a
  crash artifact `crash-<sha1>` in `fuzz/artifacts/<target>/`.
- **Out-of-bounds read or write** — the fuzz target triggers UB that
  AddressSanitizer or MemorySanitizer catches. Reported as
  `crash-<sha1>` with an `==<NNN>==ERROR: AddressSanitizer: ...` trace.
- **Hang / OOM** — the fuzzer detects an input that takes longer than
  the configured timeout (`-timeout=25` by default) or allocates past
  the RSS cap (`-rss_limit_mb=2048` by default). Reported as
  `timeout-<sha1>` or `oom-<sha1>`.
- **Memory leak** — LeakSanitizer reports an unfreed allocation.
  Reported as `leak-<sha1>`.

All four classes count as findings and all four go through the same
triage procedure below.

## Triage procedure

For every finding produced by CI or by a local fuzz run:

1. **Reproduce locally.** Copy the `crash-<sha1>` artifact into
   `fuzz/regressions/<target>/<id>.<ext>` where `<id>` is the
   monotonically increasing finding number (zero-padded to 4 digits) and
   `<ext>` is the original libFuzzer extension (`bin` for raw byte
   sequences, no extension for the corpus entries that reproduce hangs).
   Verify the reproduction by running
   `cargo +nightly fuzz run <target> <regressions/<target>/<id>.<ext>`
   and confirming the same crash class is reported.

2. **Classify.** Pick one of: `panic`, `oob-read`, `oob-write`,
   `hang`, `oom`, `leak`, `ub-other`. The class drives which fix the
   finding expects (panic → `Result`-returning API + clippy deny
   lints; OOB → checked arithmetic + bounded slices; hang/OOM →
   budget enforcement; leak → `Drop` correctness). See
   `docs/security-testing.md` for the workspace policy that backs
   each fix.

3. **Write the finding record.** Create
   `fuzz/findings/<NNNN>-<short-slug>.md` (template below) with the
   target, class, libFuzzer output snippet, the committed reproduction
   path, the root cause, the fix PR, and the regression test that
   pins the expected behaviour after the fix lands.

4. **Add a regression test.** A regression test must load the exact
   bytes committed in step 1 and assert that the parser returns the
   expected `Result::Err(<error>)` rather than panicking, hanging, or
   OOB-reading. The test lives at
   `fuzz/fuzz_targets/regressions.rs` (one test per finding,
   named `regression_<target>_<id>`) and is wired into the workspace
   `cargo test --workspace --all-features` so any future drift in the
   parse path that re-introduces the defect fails CI before merge.

5. **Tick T1519 in `.kiro/specs/elang-v0.1.0/tasks.md`** only after
   the regression test is in place **and** passing. A finding without
   a regression test is a finding that will re-surface silently the
   next time someone changes the parser.

6. **Update `fuzz/findings/index.md`** with one row per finding. The
   index is committed and serves as the audit trail: a reviewer
   checking the diff against `origin/main` can see at a glance which
   findings were open and which were closed.

## What does NOT get a finding

- A libFuzzer-discovered input that the harness already rejects with a
  `Result::Err(_)` is not a finding. It goes into
  `fuzz/corpus/<target>/` and improves coverage on subsequent runs,
  but no triage record is required.
- A panicking harness from a fuzz target that's `#[allow]`-ed past the
  workspace lint policy (i.e., the fuzz harness itself) is by design
  — the policy exemption is documented at the top of every fuzz target
  source file.

## Finding record template

```markdown
---
id: NNNN
target: <target-name>
class: panic | oob-read | oob-write | hang | oom | leak | ub-other
status: open | fixed-in-#<pr> | wontfix
discovered: YYYY-MM-DD (CI run <id> | local run by <user>)
---

# <short slug>

## Reproduction

\`\`\`
$ cargo +nightly fuzz run <target> fuzz/regressions/<target>/NNNN-<slug>.bin
\`\`\`

## libFuzzer output

\`\`\`
<full crash trace, verbatim>
\`\`\`

## Root cause

<one paragraph: which code path, which budget was missing, why the
parser mis-handled this input shape>

## Fix

PR #<pr> — <commit hash>, <one-line summary>

## Regression test

\`\`\`rust
#[test]
fn regression_<target>_NNNN() {
    let bytes = include_bytes!(
        "../../fuzz/regressions/<target>/NNNN-<slug>.bin"
    );
    let _ = <call the parser with bytes>; // must not panic
}
\`\`\`

Test name: `regression_<target>_<NNNN>`, file:
`fuzz/fuzz_targets/regressions.rs`. Asserts
`<expected error variant or category>`.
```

## Current state

See [`index.md`](./index.md) for the running tally.

| Metric | Count |
|---|---|
| Total findings (lifetime) | 0 |
| Open | 0 |
| Fixed-and-regressed | 0 |
| Wontfix | 0 |

(Updated by every triage. A zero count is a valid result — it means
the parser surface is currently holding against the budgeted fuzz
runs. The next finding increments the count and follows the procedure
above.)