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
| **Total** | | 242 | **6,708,929** | **3019** | — | **0** |

Campaign log: `/opt/data/fuzz-20261025T054633.log` (workspace
scratch; will be removed after T1519 lands).

## PR #58 will extend the surface

PR #58 (`feat/25-phase15-fuzz-t1503-t1508`, target branch) adds six
additional fuzz targets: `zstd_frame`, `kdl_parser`, `epf_schema`,
`eld_schema`, `ipa_tokenizer`, `phon_condition`. Once #58 lands the
nightly fuzz campaigns will exercise the full 8-target surface and
the index above will grow as findings surface from those targets.
The triage procedure is target-agnostic — every finding, regardless
of which target surfaced it, follows the same template and the same
test wiring.

## Future audits

The nightly fuzz campaigns in `.github/workflows/fuzz.yml` (cron
`0 4 * * *`) will surface findings in the workflow run page; the
upload-artifacts-on-failure step packages crash/timeout/oom/leak
artifacts as `fuzz-artifacts-<target>` (7-day retention). The repo
owner triages within 24 hours, following the procedure in
`fuzz/findings/README.md`.
