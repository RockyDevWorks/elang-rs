# ELANG conformance fixtures

Implements REQ-GOV-FIXTURE-001 (T0013). The conformance corpus is named
**`elang-canonical-zstd-v1-conformance`** (steering §11).

## Directory layout

```text
fixtures/
  README.md                        conventions + the expected-result format (below)
  corpus_index.json                machine-readable index of the whole corpus
  expected-result.schema.json      JSON Schema every expected.json record validates against
  valid/                           packages/resources that MUST pass
    <category>/<name>/
      archive.elang                the fixture input (package fixtures)
      input.<ext>                  the fixture input (document fixtures: .eld/.epf)
      expected.json                conforms to expected-result.schema.json
      header.bin, index.bin,       byte-exact golden outputs + .sha256 sidecars
      footer.bin                   (golden packages)
  invalid/                         inputs that MUST be rejected
    <category>/<name>/
      archive.elang                the fixture input (package fixtures)
      input.<ext>                  the fixture input (document fixtures)
      expected.json                includes the expected stage + error code
  valid/canonical/<case>/          OA-14 canonical ELD goldens
      input.eld
      golden/canonical.eld         byte-exact canonical output + .sha256 sidecar
      expected.json
```

## Naming conventions

- `category` is a lower-kebab area: `container`, `index`, `footer`, `frame`,
  `path`, `integrity`, `epf`, `eld`, `ipa`, `unicode`, `when`, `canonical`,
  `limit`.
- `name` is a lower-kebab, specific description of the single behavior under
  test, e.g. `invalid/footer/bad-index-hash`, `valid/eld/bare-allophone`,
  `invalid/ipa/tie-bar-chained`.
- One behavior per fixture. Negative fixtures assert exactly one failure cause.
- Golden binary outputs carry a `<file>.sha256` sidecar with the lowercase hex
  digest so byte-exactness is machine-checkable.

## Expected-result format

Every fixture has an `expected.json`. The conformance runner (Phase 14,
`elang-conformance`) owns the canonical JSON Schema for this format as a typed
Rust model, and publishes it as a standalone document at
[`expected-result.schema.json`](expected-result.schema.json).

**Path bases.** Every path inside an `expected.json` record is relative to the
directory that holds the record, so a record is self-contained and resolves
wherever the corpus is unpacked. `corpus_index.json` is the one exception: it
is a single manifest for the whole tree, so its paths are repository-root
relative.

Fields:

| Field | Required | Meaning |
|---|---|---|
| `fixture` | yes | Stable id, `^(valid\|invalid)/<category>/<name>$` |
| `description` | no | One-line human description |
| `requirements` | recommended | Requirement IDs exercised (`REQ-...`) |
| `input` | yes | Path to the input file, relative to the directory holding this record |
| `modes` | yes | Reader modes in which the outcome MUST hold (see below) |
| `outcome` | yes | `pass` or `fail` |
| `stage` | if `fail` | Validation stage 1–12 at which rejection MUST occur |
| `errorCode` | if `fail` | Stable error code from the registry (OA-10) |
| `golden` | if `pass` (where relevant) | `[{ "path", "sha256" }]` byte-exact outputs; `path` is relative to the directory holding this record |

### The `modes` field

`modes` is not a free-form annotation: it tells an independent implementation
which reader modes must observe the fixture's outcome. The three modes cover
cumulative pipeline stages — `inspect` covers stages 1–7, `verify` stages
1–11, `validate` stages 1–12.

- **`outcome: pass`** — acceptance is cumulative, so the record lists every
  mode down to the asserted level. A fixture asserted at `validate` lists
  `["inspect", "verify", "validate"]`.
- **`outcome: fail`** — rejection is *not* cumulative. The rejection fires at
  exactly one stage, so only the modes whose pipeline ceiling reaches that
  stage observe it; a mode that stops before it **accepts** the input. A
  rejection at stage 11 therefore lists `["verify", "validate"]` and must not
  list `inspect`.

Example (negative):

```json
{
  "fixture": "invalid/footer/bad-index-hash",
  "description": "Footer SHA-256 does not match the serialized index bytes.",
  "requirements": ["REQ-BIN-008"],
  "input": "input.elang",
  "modes": ["inspect", "verify", "validate"],
  "outcome": "fail",
  "stage": 4,
  "errorCode": "FOOTER_INDEX_HASH_MISMATCH"
}
```

## Provenance

- No fabricated normative data. Golden hashes for canonical frames are generated
  with the pinned libzstd 1.5.7; IPA/Unicode fixtures derive from the ratified
  repertoire (see [`docs/open-artifacts.md`](../docs/open-artifacts.md)).
- Each golden file's generation procedure is documented so it can be
  independently reproduced (T1420).
