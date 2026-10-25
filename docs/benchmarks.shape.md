# ELANG v0.1.0 benchmark results

> **Shape variant.** Every measured numeric value is replaced with `<MEASURED>` so the structure of this report can be byte-compared across runs and machines. The shape is what the reproducible-build workflow checks; the real numbers live in `docs/benchmarks.md`.

## Environment

- **Build profile:** `<MEASURED>`
- **Target triple:** `<MEASURED>`
- **CPU:** <MEASURED>
- **Kernel:** <MEASURED>
- **rustc:** <MEASURED>
- **Input archive:** `examples/hello-en/hello-en.elang` (1117 bytes, sha256 `3b0cfc2e05155421a1c81f3e25597cc1edefdf32b368ab0af7a4a1c4be317273`)
- **Iterations per benchmark:** 50 (after 5 warm-up iterations)
- **Reported statistic:** median (robust to a single scheduling hiccup)

## Wall-clock timings

| Benchmark | Median (ns) | Median (µs) | Min (ns) | Max (ns) | Iterations | What it measures |
|---|---:|---:|---:|---:|---:|---|
| `inspect` | <MEASURED> | <MEASURED> | <MEASURED> | <MEASURED> | 50 | Parse header/footer/index and build the resource listing (stages 1-7, no decompression). |
| `verify` | <MEASURED> | <MEASURED> | <MEASURED> | <MEASURED> | 50 | Decode every resource within budget and recompute its SHA-256 (stages 8-11). |
| `validate` | <MEASURED> | <MEASURED> | <MEASURED> | <MEASURED> | 50 | Full ELD semantic validation on top of resource integrity (stages 8-12). |
| `canonical-pack` | <MEASURED> | <MEASURED> | <MEASURED> | <MEASURED> | 50 | Build a canonical `.elang` archive (canonical EPF + ELD, integrity manifest, Zstd frames, index, footer). |
| `random-access` | <MEASURED> | <MEASURED> | <MEASURED> | <MEASURED> | 50 | Retrieve one indexed resource from a verified package by index (no re-verification). |

## Peak memory (Linux `/proc/self/status`)

| Metric | Value |
|---|---:|
| `VmHWM` (peak resident set) | <MEASURED> |
| `VmRSS` (resident set at read time) | <MEASURED> |
| `VmPeak` (peak virtual size) | <MEASURED> |

`VmHWM` is the peak resident-set size the kernel attributes to the harness process; it is monotonic across the process lifetime and is the number that belongs in the release notes.

## Methodology

- The benchmark builds the canonical example archive from `examples/hello-en/{package.epf, Linguistics/phonemes.eld}` and measures five operations on those bytes.
- Each operation runs `WARMUP` (5) untimed iterations before the timed `ITERATIONS` (50) to amortise one-off setup costs (page faults, allocator warm-up, lazy statics).
- The reported figure is the **median** wall-clock duration; the min/max span is included so a reader can see how tightly the timings cluster.
- The `random-access` benchmark reuses one verified package — verification is measured separately; the timed operation is the indexed lookup itself.

## Limitations

- These are **micro-benchmarks of one input**. They do not characterise performance on the worst-case inputs the implementation accepts (25,000 resources / 1 GiB per resource / 256-root phoneme inventories). Those boundary cases are documented by [`docs/security-limits.md`](security-limits.md) and require a different harness to measure honestly.
- `VmHWM` is a single sample of one process on one kernel. Two runs of the same workload on the same machine can differ by a few MiB depending on allocator growth patterns and page reclamation at unfortunate moments.
- The benchmark is intentionally dependency-free. There is no `--iter` / `--input` knob; allowing those would let the recorded numbers come from a different run than the release gate.

## Reproducibility

This report mixes two kinds of content: a deterministic half (the
benchmark ids, the iteration counts, the environment block, the
methodology) and a measured half (the durations, the resident-set
numbers, the generation timestamp). The measured half is a function of
the machine and the moment of the run, so it is **not** reproducible.

The generator therefore also has a `--shape` mode that writes
`docs/benchmarks.shape.md` and `docs/benchmarks.shape.json`: the same
report with every measured number replaced by a sentinel and every
host-derived environment value masked. The shape files are what the
`reproducible-build` CI workflow compares across two clean containers,
and `crates/elang-conformance/tests/measurement_shape.rs` mirrors that
check locally.
