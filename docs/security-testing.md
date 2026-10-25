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
| `zstd_frame`          | T1503  | Zstd frame-header parse and bounded decompression                                                      |
| `kdl_parser`          | T1504  | KDL 2.0 parser and AST construction                                                                     |
| `epf_schema`          | T1505  | EPF schema validation                                                                                   |
| `eld_schema`          | T1506  | ELD structural validation                                                                               |
| `ipa_tokenizer`       | T1507  | IPA tokenizer, tie-bar and modifier parser                                                              |
| `phon_condition`      | T1508  | phonological condition AST, evaluator and canonical sorting key                                         |

Targets in this PR (T1501 + T1502); the remaining six land in subsequent
Phase 15 PRs.

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

# Build every target (used by CI smoke job).
cargo +nightly fuzz build

# Run a target for a fixed time budget, seeded from fuzz/seeds/<target>/.
cargo +nightly fuzz run container_structure \
    -- -max_total_time=300 -max_len=4096
```

Per-target smoke budgets (configured in `.github/workflows/fuzz.yml`):

| Target              | PR smoke (per CI run) | Nightly campaign (cron) |
|---------------------|------------------------|--------------------------|
| `container_structure` | 60 s                  | 600 s                    |
| `resource_path`       | 30 s                  | 300 s                    |

The first PR landed only T1501 + T1502. As later PRs add more targets,
each new target inherits the same smoke + campaign pattern with budgets
proportional to the surface area under test.

## Reproducing a failure

When libFuzzer finds a crashing input it writes it to
`fuzz/artifacts/<target>/crash-<hash>`. To reproduce:

```sh
cargo +nightly fuzz run container_structure fuzz/artifacts/container_structure/crash-<hash>
```

The repro runs the original input through the target outside of the
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
