# ELANG conformance fixtures

Implements REQ-GOV-FIXTURE-001 (T0013). The conformance corpus is named
**`elang-canonical-zstd-v1-conformance`** (steering §11).

## Directory layout

```text
fixtures/
  README.md                        conventions + the expected-result format (below)
  valid/                           packages/resources that MUST pass
    <category>/<name>/
      input.<ext>                  the fixture input (.elang/.eld/.epf/.json/.bin)
      expected.json                conforms to expected-result.schema.json
      golden/                      byte-exact golden outputs + .sha256 sidecars
  invalid/                         inputs that MUST be rejected
    <category>/<name>/
      input.<ext>
      expected.json                includes the expected stage + error code
  seeds/                           fuzzing seed corpora per target
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
Rust model plus a generated schema file; it is intentionally not committed as a
standalone `*.schema.json` yet (a workspace write restriction blocks committing
remote-style JSON Schema documents in the current mode — tracked for Phase 14).

Fields:

| Field | Required | Meaning |
|---|---|---|
| `fixture` | yes | Stable id, `^(valid\|invalid)/<category>/<name>$` |
| `description` | no | One-line human description |
| `requirements` | recommended | Requirement IDs exercised (`REQ-...`) |
| `input` | yes | Relative path to the input file |
| `modes` | yes | Subset of `inspect`/`verify`/`validate` the expectation holds for |
| `outcome` | yes | `pass` or `fail` |
| `stage` | if `fail` | Validation stage 1–12 at which rejection MUST occur |
| `errorCode` | if `fail` | Stable error code from the registry (OA-10) |
| `golden` | if `pass` (where relevant) | `[{ "path", "sha256" }]` byte-exact outputs |

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
