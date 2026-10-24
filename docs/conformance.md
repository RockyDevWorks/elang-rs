# Conformance Corpus and Independent Verification

**Phase 14 — T1420.** The `elang-conformance` crate is the canonical
machine-readable interoperability corpus for ELANG. It pins the format
at every layer: byte-level vectors, document-level schemas, archive-level
pipeline behaviour, and the reader's full inspect/verify/validate
contract. Every claim in this document is enforced by the corpus suite
that runs on every pull request.

This document describes:

1. The shape of the corpus (fixture types, runners, manifest format).
2. The provenance of every pinned value (golden bytes, digests, codes).
3. How an **independent implementation** verifies itself against the
   corpus without reading this repository's Rust code.
4. What the corpus does **not** cover, and what to do when a real-world
   shape falls into that gap.

It does **not** repeat the phase-14 task list — see
`.kiro/specs/elang-v0.1.0/tasks.md` for the task-to-fixture mapping and
`docs/traceability.md` for the requirement-to-fixture mapping.

---

## 1. Corpus shape

The corpus lives in `crates/elang-conformance`. Every fixture is one of
three kinds, each with its own runner:

| Kind | Type | Runner | Asserts |
|---|---|---|---|
| Archive | `corpus::Fixture` | `run_fixture` | the reader pipeline accepts or rejects the archive at the declared level with the declared code |
| Document | `corpus::DocumentFixture` | `run_document_fixture` | the EPF/ELD parser accepts or rejects the source with the declared code |
| Vector | `corpus::VectorFixture` | `run_vector_fixture` | a production function's output equals the pinned bytes |

The three kinds together cover every normative contract: archive-level
shape, schema-level shape, and byte-level reproducibility. A regression
in any layer surfaces as a single test failure that names the fixture,
the requirement it protects, and the observed-vs-expected divergence.

### 1.1 The manifest

Every fixture carries:

- A stable `id` (slash-separated, unique across the corpus).
- A one-line `description`.
- A list of `requirements` it protects (REQ-* tokens).
- An `expectation` — accept-or-reject at a declared level, or
  accept-or-reject with a code, or byte equality.

A corpus run that fails prints:

```text
conformance failure: fixture `binary/header-bad-magic` (requirement REQ-BIN-001):
expected code HEADER_BAD_MAGIC (category Container) at level Inspect,
got code HEADER_BAD_VERSION (category Container) stage=Some(1)
message=header version 0.2 does not match the pinned 0.1
```

That message is enough to drive a fix in any implementation — no source
access to this repository is required.

### 1.2 The level ladder

The reader pipeline is split into three levels:

| Level | Stages | Guarantee |
|---|---|---|
| `Inspect` | 1–7 | structure only; no decompression, no integrity |
| `Verify` | 1–11 | every resource decompresses and hashes match |
| `Validate` | 1–12 | full semantic validity of EPF + ELD |

A negative fixture declares the **lowest** level at which it must be
rejected. The runner proves the lower levels **accept** the fixture, so
"it failed somewhere" cannot launder a structurally-broken archive into
a "semantic" rejection.

---

## 2. Provenance

Every byte and every code in the corpus is **derived**, never hand-written.
The rule is the same for all three fixture kinds:

- **Byte-level vectors** — the production code computes the value;
  the corpus records what it produced. The pinned constant is the
  record, not the source of truth. The normative specification
  (Unicode 18.0.0, RFC 8878, the steering doc) is the source of truth;
  the corpus is evidence that this implementation agrees.
- **Canonical goldens** — the corpus's `gen_*` binaries run the
  production pipeline (`parse → canon::to_string → sha256`) over the
  published inputs and write the result. The pinned constant in the
  corpus module is what those binaries produced.
- **Diagnostic codes** — every code the corpus asserts is registered in
  `specs/registry/error-codes.json`. The corpus cannot assert a code
  the registry does not know about, and the registry-completeness test
  fails any code emitted as a literal but missing from the registry.

### 2.1 The pinned profile

The Zstandard frame profile is **pinned** at `elang-canonical-zstd-v1`,
which corresponds to libzstd **10507**. The corpus publishes the
per-frame SHA-256 digests for every indexed frame in both golden
packages. If the linked libzstd version drifts, or the canonical
compression parameters change, those digests stop matching and the
corpus fails loudly. That is the intended behaviour — both are
normative changes that require a superseding ADR.

### 2.2 The pinned Unicode data tables

The Unicode data tables are pinned to version 18.0.0. A change to the
tables or the pinned version is a normative change to the corpus; the
NFC vectors, IPA validation, and forbidden-category fixtures would all
diverge.

---

## 3. Independent verification

A second implementation can verify itself against the corpus as follows.

### 3.1 Byte-level checks

For every `VectorFixture`:

1. Run the corresponding production function on the input.
2. Compare the output bytes against the fixture's `expected` field.

The corpus's `binary_vectors::hex` helper is the canonical encoding for
the comparison.

### 3.2 Document-level checks

For every `DocumentFixture`:

1. Run the corresponding parser (`elang_epf::parse` or
   `elang_eld::parse`) on the source bytes.
2. If the fixture expects acceptance, assert the parse succeeds.
3. If the fixture expects rejection, assert the parse fails with exactly
   the pinned `code` and `category`.

The runner's failure message names the observed code and category, so a
divergence is actionable without consulting this repository.

### 3.3 Archive-level checks

For every `Fixture`:

1. Run `Package::open(archive)` against the archive bytes.
2. If the fixture expects acceptance at level `L`, run all stages up
   to `L.ceiling_stage()` and assert the package survives them.
3. If the fixture expects rejection at level `L`:
   - Assert the lower levels (`Inspect`, `Verify`) all accept the
     archive. This is the anti-laundering rule — a "rejection" at a
     lower level cannot masquerade as the declared level.
   - Run the reader at level `L` and assert it fails with the pinned
     code and category.

### 3.4 Source of truth

The corpus's pinned values are **records**, not the source of truth.
The sources of truth are, in order of precedence:

1. The Unicode 18.0.0 data tables (pinned at
   `elang-unicode::PINNED_UNICODE_VERSION`).
2. RFC 8878 (the Zstandard frame format).
3. The normative manifest (`specs/registry/normative-manifest.json`).
4. The steering document (`docs/specification.md`).
5. The error-code registry (`specs/registry/error-codes.json`).
6. This corpus (records what this implementation produced).

When the corpus's record and the normative source disagree, **the
normative source wins** and the corpus is regenerated. The regeneration
procedure is a one-command operation:

```bash
cargo run -p elang-conformance --bin gen_binary_vectors
cargo run -p elang-conformance --bin gen_canonical_goldens
cargo run -p elang-conformance --bin gen_normative_manifest
cargo run -p elang-conformance --bin gen_epf_canonical_goldens
```

Each command regenerates the corresponding pinned constants and writes
the expected artifacts. A CI diff that touches only generated files is
a candidate for a version-bump commit and a normative ADR, not a code
change.

---

## 4. What the corpus does **not** cover

The corpus is comprehensive within the v0.1.0 schema, but the following
intentionally remain outside it:

- **Performance benchmarks** — the corpus asserts correctness, not
  speed. A separate `cargo bench` profile (added in a later phase)
  covers throughput.
- **Resource exhaustion** beyond the documented parser budgets — the
  corpus pins every published budget at its boundary, but a fuzzing
  campaign against the reader is not part of the corpus.
- **Backwards-compatibility shims** — the v0.1.0 corpus is a single
  version. A future v0.2.0 corpus will coexist with the v0.1.0 corpus
  via the version prefix in the manifest.
- **Diagnostic message text** — the corpus asserts codes and categories,
  not the human-readable `message` string. A change to a message's text
  is non-normative; a change to the code or category requires a
  registry update and an ADR.

If a real-world shape falls outside the corpus, the right next step is
**not** to skip the corpus for that shape. Add a fixture — that is the
purpose of the corpus. The runner is intentionally minimal so a new
fixture takes five minutes to add and one test to verify.

---

## 5. The fixture matrix

The full corpus currently publishes:

| Task | Coverage | Files |
|---|---|---|
| T1401 | Fixture manifest, level model, runner | `corpus.rs` |
| T1402/T1403 | Minimal + full golden packages with exact header/index/footer bytes and SHA-256 | `golden_packages.rs` |
| T1404 | Pinned libzstd frame hashes + version | `zstd_vectors.rs` |
| T1405 | EPF canonical KDL golden text files | `epf_canonical_goldens.rs` |
| T1406 | Condition sorting-key byte vectors for all six ranks | `condition_vectors.rs` |
| T1407 | Unicode NFC + IPA valid/invalid fixtures | `unicode_ipa.rs` |
| T1408 | Group/roman/allophone structural + uniqueness fixtures | `epf_eld_negatives.rs` |
| T1409 | Condition semantics truth tables | `condition_vectors.rs` |
| T1410 | Malformed binary / index / footer / path / truncation / overflow fixtures | `negatives.rs` |
| T1411 | Zstd frame contract negatives | `zstd_negatives.rs` |
| T1412 | Integrity omission / corruption / extra-entry fixtures | `negatives.rs` |
| T1413 | EPF schema negatives | `epf_eld_negatives.rs` |
| T1414 | ELD schema negatives | `epf_eld_negatives.rs` |
| T1415 | Parser budget boundary fixtures | `parser_budgets.rs` |
| T1416 | Inspect / verify / validate level matrix | `inspect_matrix.rs` |
| T1417 | Write→read→write byte identity | `roundtrip.rs` |
| T1418 | Shuffled source order → same canonical archive | `roundtrip.rs` |
| T1419 | Cross-platform CI matrix (Linux, macOS, Windows) | `.github/workflows/ci.yml` |
| T1420 | This document | `docs/conformance.md` |

The requirement-to-fixture mapping lives in `docs/traceability.md`. A
new requirement MUST add at least one fixture; a new fixture MUST name
at least one requirement.

### 5.1 The requirement-coverage table

Fixtures are not the only evidence a requirement can have. A normative
requirement may be enforced by a unit test in the crate that owns the
behaviour — a container test for `REQ-BIN-013`, a KDL test for
`REQ-LIMIT-004` — and the corpus fixture is the wrong granularity for
it. The corpus therefore keeps a second, explicit table:

`crates/elang-conformance/src/requirement_coverage_data.rs` — one row per
`(requirement, crate, test_name, kind)`. It currently holds **374 rows
covering all 169** normative `MUST` / `MUST NOT` requirements in
`.kiro/specs/elang-v0.1.0/requirements.md`.

The `kind` column records what the named test actually does:

| Kind | Meaning |
|---|---|
| `Positive` | the test asserts acceptance behaviour |
| `Negative` | the test asserts rejection behaviour |
| `Meta` | the test is a meta-assertion (registry check, code count, document presence) |
| `Doc` | the test asserts the existence of a documented artifact |

### 5.2 Why the table is checked against the spec at runtime

`crates/elang-conformance/tests/requirement_completeness.rs` parses the
normative-requirement table from `requirements.md` **at test time** and
checks the mapping in both directions:

1. Every normative `MUST` has at least one coverage row.
2. Every coverage row names a real normative `MUST` — a typo like
   `REQ-ELD-18` for `REQ-ELD-018` fails the suite instead of silently
   leaving a requirement uncovered.
3. Every fixture in the catalog names at least one real requirement, and
   every rejection fixture declares an exact diagnostic code.

A hard-coded list of requirement ids would be a snapshot: the day
someone adds a normative `MUST` to `requirements.md`, the snapshot would
still pass and the corpus would silently stop covering the new
requirement. Parsing the spec means the corpus fails on exactly the
change that breaks the invariant.

### 5.3 The anti-fabrication check

A coverage row that names a test which does not exist is a false
coverage claim, and it is the failure mode a table like this invites.
`crates/elang-conformance/src/meta_tests.rs` therefore scans every
crate's source tree at test time and asserts that every named test is
actually present **and actually carries a `#[test]`-family attribute**.
The attribute requirement matters: a zero-argument helper function
named like a test would otherwise satisfy a bare `fn name()` scan while
proving nothing. Renaming a test without updating the table fails CI;
the table cannot drift away from the code it claims to describe.

Two further conditions make the claim honest rather than merely
well-formed:

- **The test must live in the crate the row declares.** The scanner
  indexes test names *per crate*, so a row claiming `elang-epf` cannot
  be satisfied by a same-named test in `elang-conformance`. Without
  this, the `crate_name` column would be decoration.
- **The test must not be `#[ignore]`d.** An ignored test never runs, so
  naming one would claim coverage that is never exercised. The
  scanner reads the whole attribute run above a function — `#[test]`
  and `#[ignore]` may appear in either order — and rejects ignored
  tests outright.

The same module pins the corpus's internal discipline:

- `the_scanner_rejects_non_test_helpers` — a self-test that feeds the
  scanner helpers with and without test attributes and asserts only the
  real tests are collected.
- `the_runner_path_never_panics_on_malformed_input` — the production
  portion of `corpus.rs` (the runner that consumes malformed archives)
  must contain no `expect` / `panic!` / `unwrap()`. The crate-level
  lint allow is scoped to the *authoring* helpers, where a panic is the
  intended fail-closed mode; the *runner* reports a conformance failure
  instead of aborting the suite.

Authoring helpers never silently skip. A fixture whose mutation cannot
be applied panics at authoring time rather than producing an archive
that would be rejected for the wrong reason — a vacuous negative
fixture that still looks complete is worse than no fixture at all.

The governance requirements (`REQ-GOV-*`) are covered by `Meta` rows
that assert the presence of the documented artifacts they name — the
CI workflow, the ADRs, the traceability matrix. `REQ-SEC-001` (fuzz and
property coverage) is covered by the fuzz-survival tests that already
exist in `elang-kdl`; the full fuzzing campaign is Phase 15.

---

## 6. When a fixture disagrees

When the corpus reports a failure, the failure is the source of truth
for the divergence. The order of investigation is:

1. **Re-read the fixture's expectation** — does the fixture claim a
   code/category that matches the registry? The
   `tests/registry_completeness.rs` test catches the inverse.
2. **Check the registry** — is the code still in `error-codes.json`?
   If not, the fixture is stale.
3. **Check the implementation** — has the stage mapping changed? Has
   a recent refactor moved the rejection to a different stage?
4. **Check the fixture** — is the broken condition the one the fixture
   claims? A regression in the helper that builds the fixture
   (`archive_with_frame`, `eld_with_nested_when`, etc.) can produce
   fixtures that test the wrong thing.

If the failure is real, fix the implementation. If the failure is in
the fixture (it pins a stale value), regenerate it via the
corresponding `gen_*` binary. If the failure is in the registry
(a code was removed), the right action is an ADR and a corpus
regeneration, not a code revert.
