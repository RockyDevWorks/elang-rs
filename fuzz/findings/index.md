# Fuzz findings index

Auto-curated list of every finding ever produced by the fuzz targets
under `fuzz/fuzz_targets/`. One row per finding; the finding record
itself lives at `fuzz/findings/<NNNN>-<slug>.md`.

| ID | Target | Class | Status | Discovered | Discovered by | Fix PR | Regression test |
|---|---|---|---|---|---|---|---|
| _none yet_ | | | | | | | |

A finding only earns its row once the regression test
(`crates/elang-conformance/tests/fuzz_regressions.rs::regression_<target>_<NNNN>`) is
in place and passing on `origin/main`. See
[`README.md`](./README.md) for the triage procedure.

## T1519 closure audit trail (2026-10-25)

T1519 closes with **0 findings** after the local fuzz campaigns run
on `feat/25-phase15-fuzz-triage-t1519` (HEAD this commit) at the
time of T1519 closure. The triage infrastructure is in place so the
first finding that surfaces — from any future nightly CI run, any
local campaign, or any reviewer-driven `cargo +nightly fuzz run` —
will follow the procedure in `fuzz/findings/README.md` and add a
row here.

| Campaign | Target | Budget (s) | Executions | New units | Peak RSS (MiB) | Crashes |
|---|---|---|---|---|---|---|
| local 2026-10-25T05:46:33Z | `container_structure` | 121 | 4,260,857 | 1346 | 261 | 0 |
| local 2026-10-25T05:46:33Z | `resource_path` | 121 | 2,448,072 | 1673 | 483 | 0 |
| **Total (T1519 closure)** | | **242** | **6,708,929** | **3019** | — | **0** |

Campaign log: `/opt/data/fuzz-20261025T054633.log` (workspace
scratch; will be removed after T1519 lands).

### Full surface post-PR-#58 (re-verified 2026-10-25)

PR #58 (`feat/25-phase15-fuzz-t1503-t1508`) merged after the
original T1519 closure, expanding the surface from 2 to 8 fuzz
targets. The audit trail below re-runs every target with a uniform
60-second budget so the comparison is apples-to-apples. **0 findings
across the full surface** — the parser held under 14M additional
adversarial executions spanning IPA tokenization, KDL parsing,
ELD/EPF schema validation, allophone condition evaluation, and Zstd
frame decoding.

| Campaign | Target | Budget (s) | Executions | New units | Peak RSS (MiB) | Crashes |
|---|---|---|---|---|---|---|
| local 2026-10-25T06:30Z | `container_structure` | 60 | 1,612,290 | 88 | 261 | 0 |
| local 2026-10-25T06:30Z | `resource_path` | 60 | 1,242,200 | 311 | 459 | 0 |
| local 2026-10-25T06:30Z | `zstd_frame` | 60 | 4,415,079 | 9 | 298 | 0 |
| local 2026-10-25T06:30Z | `kdl_parser` | 60 | 888,679 | 4225 | 486 | 0 |
| local 2026-10-25T06:30Z | `eld_schema` | 60 | 1,127,453 | 5007 | 490 | 0 |
| local 2026-10-25T06:30Z | `epf_schema` | 60 | 1,074,799 | 4629 | 492 | 0 |
| local 2026-10-25T06:30Z | `ipa_tokenizer` | 60 | 2,697,572 | 668 | 477 | 0 |
| local 2026-10-25T06:30Z | `phon_condition` | 60 | 972,429 | 4860 | 509 | 0 |
| **Total (post-PR-#58 re-verification)** | | **480** | **14,030,501** | **19,797** | — | **0** |

Combined T1519 audit trail (T1519 closure + post-#58 re-verify):
**20,739,430 adversarial executions, 0 findings, 0 panics, 0
out-of-bounds reads.** The regression-test harness
(`crates/elang-conformance/tests/fuzz_regressions.rs`) wires all
8 targets through `KNOWN_TARGETS` so any future finding gets a
regression test immediately.

## Future audits

The nightly fuzz campaigns in `.github/workflows/fuzz.yml` (cron
`0 4 * * *`) will surface findings in the workflow run page; the
upload-artifacts-on-failure step packages crash/timeout/oom/leak
artifacts as `fuzz-artifacts-<target>` (7-day retention). The repo
owner triages within 24 hours, following the procedure in
`fuzz/findings/README.md`.
