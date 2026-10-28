# Security testing

This document records the security-testing strategy for ELANG v0.1.0 — the
fuzzing, property-testing, resource-exhaustion, safe-extraction, and
audit work that lands across Phase 15 (Issue #25, T1501–T1520).

It complements [SECURITY.md](../SECURITY.md), which captures the threat
model and trust boundaries; this file is the **how** that the threat
model is exercised by automated tests.

## Engines

| Engine                 | Used for                                              | Stable? | Where it lives                  |
|------------------------|-------------------------------------------------------|---------|---------------------------------|
| `cargo-fuzz` (libFuzzer) | coverage-guided fuzzing of every untrusted-input parser | nightly | [`fuzz/`](../fuzz/)            |
| `proptest`              | property-based invariants on canonicalization, round-trips, and bytewise ordering | stable  | dev-deps inside the relevant crate |
| `cargo test`            | deterministic randomized fuzz (`crates/elang-kdl/tests/fuzz.rs`) | stable  | per-crate test suite            |
| `cargo llvm-cov`        | line/branch coverage reporting                       | stable  | CI (`coverage` job)             |
| `cargo audit`, `cargo deny` | dependency vulnerability and license audit       | stable  | CI (`security.yml`)             |

`cargo-fuzz` is **excluded from the workspace** (`Cargo.toml` excludes the
`fuzz` member). The main crate builds, lints and tests run on stable and
MSRV; only the fuzz crate uses nightly with sanitizer instrumentation and
a relaxed lint policy documented per fuzz target. This split is by
design — Phase 15 needs the coverage-guided power of libFuzzer without
loosening the strict untrusted-input lints that guard the production
crates.

## Fuzz targets

Every target in `fuzz/fuzz_targets/` accepts arbitrary `&[u8]` from
libFuzzer and drives it through one parser chain. The target name maps
1-to-1 to a Phase 15 task ID:

| Target              | Task   | Drives                                                                                                   |
|---------------------|--------|----------------------------------------------------------------------------------------------------------|
| `container_structure` | T1501  | `Header::decode`, `Footer::decode`, `Index::decode` against arbitrary bytes                            |
| `resource_path`       | T1502  | UTF-8 → NFC → `ResourcePath::new` chain against arbitrary bytes                                      |
| `zstd_frame`          | T1503  | Zstd frame-header inspection + bounded streaming decode with a 1 MiB per-resource ceiling             |
| `kdl_parser`          | T1504  | KDL 2.0 parser and AST construction, with both per-resource and archive-wide `BudgetTracker`s         |
| `epf_schema`          | T1505  | EPF schema validation, including the 1 MiB `MAX_EPF_BYTES` input-size cap                              |
| `eld_schema`          | T1506  | ELD structural validation (groups, roots, allophones, IPA, romanization, tie-bar uniqueness)          |
| `ipa_tokenizer`       | T1507  | IPA unit-splitter, tie-bar state machine, modifier classifier, and comparison-key normalisation        |
| `phon_condition`      | T1508  | condition AST builder, canonical key + sort helpers, depth introspection, and pure evaluator           |

Targets T1501 + T1502 landed in PR #55; T1503–T1508 land in this PR. The
remaining Phase 15 tasks (T1512–T1520, resource-limit tests and
supply-chain audit) follow in subsequent PRs.

The fuzz crate is wired so each target is its own `[[bin]]` entry in
`fuzz/Cargo.toml` and its own row in the `smoke` / `nightly` matrices of
`.github/workflows/fuzz.yml`. Per-target budgets are tuned to the cost
of the parser under test: Zstd decode gets the largest budget because
it is the most expensive codepath; the cheap IPA tokenizer and
condition AST get smaller ones.

### Lint policy

The workspace denies `unwrap_used`, `expect_used`, `panic`,
`indexing_slicing`, `arithmetic_side_effects`, `cast_possible_truncation`,
`cast_sign_loss` and `cast_possible_wrap` precisely *because* the
production code must not panic on untrusted input. Fuzz targets invert
this: panics on adversarial input are the failure signal that surfaces
robustness defects in the parser under test. Each fuzz target therefore
opens with:

```rust
#![no_main]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fuzz harness drives adversarial inputs through <parser>; \
              panics are the failure signal that surfaces robustness defects"
)]
```

The allow list is per-file, never crate-wide. Each `reason = ...` is the
justification required by the untrusted-input lint policy in
`Cargo.toml`.

## Seed corpus

Initial seeds live in [`fuzz/seeds/<target>/`](../fuzz/seeds/). They are
committed; the fuzzer-generated corpus under `fuzz/corpus/<target>/` is
git-ignored (already covered by `fuzz/.gitignore` from `cargo fuzz init`)
so CI smoke runs cannot bloat the repository.

Seeds are hand-picked to exercise the happy path plus the documented
rejection paths (path traversal, absolute paths, backslashes, NUL,
non-UTF-8, oversized inputs, etc.). Conformance fixtures from
`crates/elang-conformance/` are out of scope as direct seeds for the
container and path targets — those bytes are valid KDL/ELD, not valid
container slices — but the **golden packages** produced by the
conformance corpus (T1408, binary form) are candidates for the
`container_structure` seed corpus once they land in a stable location.

## Build and run

```sh
# One-time: nightly toolchain (cargo-fuzz requires it).
rustup toolchain install nightly --profile minimal \
    --component rustfmt --component clippy
# Pin cargo-fuzz itself; `--locked` only fixes the transitive graph.
cargo +nightly install cargo-fuzz --version 0.13.2 --locked

# Build every target (used by CI smoke job).
cargo +nightly fuzz build

# Copy committed seeds into the writable, git-ignored corpus.
mkdir -p fuzz/corpus/container_structure
cp -r fuzz/seeds/container_structure/. fuzz/corpus/container_structure/

# Run a target for a fixed time budget using that corpus.
cargo +nightly fuzz run container_structure fuzz/corpus/container_structure \
    -- -max_total_time=300 -max_len=4096
```

Per-target smoke budgets (configured in `.github/workflows/fuzz.yml`):

| Target              | PR smoke (per CI run) | Nightly campaign (cron) |
|---------------------|------------------------|--------------------------|
| `container_structure` | 60 s                  | 600 s                    |
| `resource_path`       | 30 s                  | 300 s                    |
| `zstd_frame`          | 60 s                  | 600 s                    |
| `kdl_parser`          | 30 s                  | 300 s                    |
| `epf_schema`          | 45 s                  | 450 s                    |
| `eld_schema`          | 45 s                  | 450 s                    |
| `ipa_tokenizer`       | 30 s                  | 300 s                    |
| `phon_condition`      | 45 s                  | 450 s                    |

The per-target `-max_len` for each `cargo +nightly fuzz run` call is
configured in `.github/workflows/fuzz.yml` alongside the time budget:
Zstd decode and KDL lexing take 4 KiB; EPF/ELD/condition docs take 8
KiB; IPA identifiers are capped at 256 B (the maximum 8-scalar
identifier is well under that).

## Reproducing a failure

When libFuzzer finds a crashing input it writes it to
`fuzz/artifacts/<target>/crash-<hash>`. To reproduce:

```sh
cargo +nightly fuzz run container_structure fuzz/artifacts/container_structure/crash-<hash>
```

The repro runs the original input through the target outside the
fuzzer loop. The corresponding entry then becomes a regression fixture
under `crates/<relevant>/tests/regressions/`, with a corresponding
issue and Phase 15 T1519 regression-coverage update.

## Reporting findings

Suspected vulnerabilities found by these campaigns are reported through
the same private channel as SECURITY.md; the fuzzer-derived regression
fixture is included as an attachment, not pasted inline.

## Limitations

A successful fuzzing campaign does not establish a proof of correctness.
The recorded budget, seed set and CI integration are evidence of a
structured adversarial search, not a guarantee that the implementation
is free of remaining defects. See [SECURITY.md §Non-goals](../SECURITY.md)
for the explicit trust limitations of ELANG v0.1.0.

## Security audits (T1515–T1518)

Phase 15 also closes four audit-style tasks that are not fuzz harnesses
themselves but produce the evidence the threat model in
[SECURITY.md](../SECURITY.md) is enforced by.

### T1515 — `unsafe` / FFI / libzstd linkage audit

The workspace contains **one** `unsafe` block. It lives at
[`crates/elang-container/src/frame.rs:497`](../crates/elang-container/src/frame.rs)
and is a single `zstd_sys::ZSTD_getErrorCode(e)` call inside the
streaming-decode error mapper. The block carries a SAFETY comment
(`zstd-safe 8.2.0 has no safe error-code accessor`) and a
`#[allow(unsafe_code, reason = "...")]` attribute. No other crate in
the workspace uses `unsafe`.

The libzstd pin is enforced at three layers:

- **Workspace dep pin** in [`Cargo.toml`](../Cargo.toml):
  `zstd = "=0.14.3"`, `zstd-safe = "=8.2.0"`, `zstd-sys = "=2.1.2"` —
  all three exact pins.
- **Compile-time constant** at
  [`crates/elang-container/src/frame.rs:66`](../crates/elang-container/src/frame.rs):
  `PINNED_LIBZSTD_VERSION: u32 = 10_507;` (i.e. 1.5.7).
- **Runtime FFI check** via
  [`linked_libzstd_version()`](../crates/elang-container/src/frame.rs),
  asserted by `pinned_zstd_constant_is_157` and `linked_libzstd_is_157`
  in `frame/tests.rs`.

### T1516 — dependency vulnerability and license audit

`cargo audit` and `cargo deny` are wired into CI via
[`.github/workflows/security.yml`](../.github/workflows/security.yml).
The `security.yml` workflow runs on every push to `main` and on a
nightly cron at 05:30 UTC. At the time of T1516 closure both tools
report clean:

- `cargo audit`: 55 dependencies scanned, 0 advisories.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
  Warnings about un-encountered allow-listed licenses
  (`BSD-2-Clause`, `ISC`, `NCSA`, `Unicode-DFS-2016`, `Zlib`) are
  expected — they cover future dependency choices and stay in the
  policy so adding a same-license dep never trips the check.

License allow-list is in [`deny.toml`](../deny.toml). The T1518
differential harness adds `kdl = "=6.5.0"` (Apache-2.0, already in
the allow list) as a `[dev-dependency]` of `elang-kdl` only.

### T1517 — panic / out-of-bounds on untrusted input

The workspace lint policy
([`Cargo.toml [workspace.lints]`](../Cargo.toml)) denies, with `-D
warnings` in CI:

- `clippy::unwrap_used` / `clippy::expect_used` / `clippy::panic`
  / `clippy::indexing_slicing` — every panic-prone path on untrusted
  data fails the build before merge.
- `clippy::arithmetic_side_effects` / `cast_possible_truncation` /
  `cast_sign_loss` / `cast_possible_wrap` — every unchecked integer
  math or cast on untrusted data fails the build before merge.

The implemented fuzz targets, `container_structure` (T1501) and
`resource_path` (T1502), complement these lint checks with panic/OOB
coverage. [The fuzz workflow](../.github/workflows/fuzz.yml) runs smoke
checks on PRs matching its path filters (60 s and 30 s, respectively)
and longer nightly campaigns at 04:00 UTC (600 s and 300 s,
respectively). The remaining T1503–T1508 targets are planned.

### T1518 — differential tests against an independent parser

[`crates/elang-kdl/tests/differential.rs`](../crates/elang-kdl/tests/differential.rs)
cross-checks `elang-kdl` against the official `kdl` crate (kdl-rs v6.5.0)
on a corpus of KDL 2.0 spec inputs:

1. **Accept/reject verdict agreement** — both parsers must agree on
   accept for spec shapes both implement, and on reject for KDL 1.0
   shapes and KDL 2.0 syntax errors.
2. **Top-level node name agreement** — for inputs both accept, the
   sequence of decoded top-level node names must match directly.
3. **Spec-surface coverage** — the corpus covers every documented
   KDL 2.0 surface (value forms, escape sequences, raw strings,
   slashdash, comments, version pragma, type annotations,
   unicode identifiers) and at least one reject + one elang-only
   accept case.

The KDL 2.0 version marker `/- kdl-version 2` is a shared-accept case.
The separately labeled ELANG-specific directive `/kdl-version "2.0"`
is flagged as `elang_only_accept` because the reference parser rejects
it. The harness asserts this expected asymmetry rather than allowing
it to drift silently.

This is a structural check, not a tautology: a grammar drift in
either parser surfaces as a verdict or node-name mismatch with the
case id and the source pinned in the corpus.
## Fuzz triage (T1519)

The 8 fuzz targets above are preventive — they run, the parser
holds, CI is green. Phase 15 also requires the **reactive** side:
when libFuzzer *does* find a crash, what happens?

- The nightly campaign's `upload-artifacts-on-failure` step in
  [`.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml)
  packages the `crash-<sha1>` / `timeout-<sha1>` / `oom-<sha1>` /
  `leak-<sha1>` artifact as `fuzz-artifacts-<target>` (7-day
  retention).
- The triage procedure lives at
  [`fuzz/findings/README.md`](../fuzz/findings/README.md). One
  finding record per finding (`fuzz/findings/<NNNN>-<slug>.md`),
  one committed reproduction byte sequence per finding
  (`fuzz/regressions/<target>/<NNNN>-<slug>.bin`), one regression
  test per finding
  (`crates/elang-conformance/tests/fuzz_regressions.rs::regression_<target>_<NNNN>`).
- The audit trail lives at
  [`fuzz/findings/index.md`](../fuzz/findings/index.md): one row
  per finding, plus the lifetime campaign totals (executions,
  crashes, peak RSS).
- The end-to-end procedure, including the discipline ("no finding
  record without a committed reproduction", "no regression test
  that calls the wrong code path", "no `fixed` finding without a
  passing regression test"), is documented at
  [`docs/fuzz-triage.md`](../fuzz-triage.md).

T1519 closed with **0 findings** after the original local fuzz
campaign on the two targets then on `origin/main` (`container_structure`,
`resource_path`) — 6.7 M adversarial executions in 242 s. The full
8-target surface was re-verified post-PR-#58 with a uniform 60 s
budget per target (14.0 M additional executions, 0 findings); the
combined audit trail totals **20.7 M adversarial executions across
all 8 targets**. The triage infrastructure is in place; the first
finding that surfaces will follow the procedure above.
