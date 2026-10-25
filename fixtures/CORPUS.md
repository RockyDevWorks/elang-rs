# ELANG v0.1.0 conformance corpus

The published conformance corpus is the official
`elang-canonical-zstd-v1-conformance` corpus for the ELANG v0.1.0
release. Every fixture on disk under `fixtures/` ships with an
`expected.json` record that an independent implementation can validate
its results against, plus a `.sha256` sidecar so byte-exactness is
machine-checkable.

## Layout

```text
fixtures/
  CORPUS.md                          this file
  README.md                          conventions + expected-result format (older)
  corpus_index.json                  machine-readable index of every catalog entry
  expected-result.schema.json        JSON Schema for `expected.json`
  valid/                             inputs that MUST be accepted
    <family>/<name>/
      input.<ext>                    the input file
      expected.json                  the expected-result record (JSON Schema above)
      golden/                        byte-exact golden outputs (canonical ELD only)
      *.sha256                       sidecar digests for the inputs / segments
  invalid/                           inputs that MUST be rejected
    <family>/<name>/
      input.elang|input.epf|input.eld the malformed input
      expected.json                  outcome + stage + error code
      *.sha256                       sidecar digest for the input
```

`fixtures/valid/` and `fixtures/invalid/` together hold **74 fixture
directories on disk** — 31 that MUST be accepted and 43 that MUST be
rejected. Of those, **60 are published** (flagged `published: true` in
`corpus_index.json`):

- 2 golden packages (full archives: `valid/minimal`, `valid/full`)
- 6 inspect/verify/validate level-matrix acceptances (`valid/matrix/<2>/<3>`)
- 5 ELD accept edges (`valid/eld/accept-*`)
- 1 EPF accept edge (`valid/epf/accept-minimal`)
- 3 parser-budget at-limit acceptances (`valid/budget/<3>`)
- 43 malformed archives and documents that MUST be rejected
  (`invalid/<family>/<name>` across `binary/`, `zstd/`, `integrity/`,
  `reader/`, `eld/`, `epf/`, `budget/`)

That is 17 accepted + 43 rejected = **60 published**.

The other 14 directories on disk are not conformance fixtures but OA-14
canonical artefacts. They are frozen by the normative manifest
(`specs/registry/normative-manifest.json`) rather than by the
conformance index, and are covered by `docs/release/v0.1.0/conformance-hashes.txt`
only insofar as they are part of the published set above:

- 13 canonical ELD text goldens (`valid/canonical/<13>`)
- 1 container reference archive (`valid/container/minimal-archive`)

The machine-readable `corpus_index.json` records every published entry
alongside 64 in-code-only entries: 47 vector fixtures (byte-equality
checks) and 17 meta-assertions (the CI matrix runs on three OSes, the
conformance doc exists, etc.). The `kind` field distinguishes them.

## Running the corpus on an independent implementation

1. **Read `fixtures/expected-result.schema.json`.** Every
   `fixtures/.../expected.json` record must validate against it.

2. **Walk `fixtures/valid/` and `fixtures/invalid/`.** For each
   directory, open the input file at `input.<ext>` (or `archive.elang`
   for archive-level fixtures), run it through your implementation at
   every mode listed in `expected.json`'s `modes` array, and compare
   the outcome against the record's `outcome` field.

   * For `outcome == "pass"`, every mode must accept the input. If the
     fixture pins a `golden` array, the mode listed there must
     produce byte-identical output with the published `sha256`.
   * For `outcome == "fail"`, the mode listed in `modes` must reject
     the input. The rejection must surface at the stage recorded in
     `expected.json.stage` and emit the registered code in
     `expected.json.errorCode`.

3. **Verify byte-exactness.** Every input carries a `.sha256` sidecar
   (`<input>.sha256`). Compare the digest of the bytes your
   implementation reads against the published digest before running.
   The golden packages ship three additional segment sidecars
   (`header.bin.sha256`, `index.bin.sha256`, `footer.bin.sha256`)
   covering the byte-exact header, index and footer segments.

4. **Cross-check `corpus_index.json`.** Every `published: true` entry
   has a non-empty `input`, `sha256`, and `corpusId` field. A reader
   that finds a missing or stale field has caught a corpus publication
   bug — file an issue rather than working around it.

## What the corpus does **not** test

- **Byte-equality of computed values** (condition sorting keys, NFC
  forms, Zstd frame digests, …). These are asserted by the
  `elang-conformance` crate's own test suite against production code,
  and have no portable on-disk representation an independent
  implementation could verify without first *computing* the value
  itself. They appear in `corpus_index.json` with `kind: "vector"`
  and `published: false`.

- **Repository-internal assertions** (the CI matrix runs on three
  OSes, `docs/conformance.md` exists, …). These are checked by the
  conformance crate's own tests and have no external input. They
  appear with `kind: "meta"`.

## Provenance and regeneration

Every published fixture is generated by
`cargo run -p elang-conformance --bin gen_conformance_corpus`. The
generator is idempotent: re-running it on an unchanged tree rewrites
nothing, and the
`crates/elang-conformance/tests/corpus_publication.rs` drift test
re-asserts that the on-disk bytes match what the generator would
produce. A corpus that disagrees with its source is a CI failure,
not a working state.

Regeneration:

```sh
cargo run -p elang-conformance --bin gen_conformance_corpus
```

## Constraints honoured by every fixture

- **No fabricated normative data.** Every byte is produced by the
  production writer, decoder, or test-data builder. The pinned
  digests are computed at generation time, never typed by hand.
- **Every requirement has at least one fixture.** The
  `requirement-coverage` crate test asserts that every requirement
  named by a fixture is a registered requirement id and that every
  normative requirement has at least one fixture naming it.
- **Every negative fixture names the exact stage and error code.** A
  reader that rejects a negative input with the right code but at
  the wrong stage (or vice versa) is a release blocker, not a minor
  divergence.

## See also

- [`fixtures/README.md`](README.md) — the original conventions and
  the expected-result field reference.
- [`docs/open-artifacts.md`](../docs/open-artifacts.md) — which
  normative artifacts are ratified vs. open; the corpus reflects OA-1
  through OA-16 (OA-14 canonical ELD goldens were the last to land,
  in Issue #21).
- [`docs/traceability.md`](../docs/traceability.md) — every
  requirement id and where it is enforced.
