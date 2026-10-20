# ELANG authoritative specs and registries

This directory holds the machine-checkable normative artifacts for ELANG
v0.1.0, as prescribed by the steering contract (`.kiro/steering/elang.md` §1.1,
§12) and the implementation checklist
(`.kiro/specs/elang-v0.1.0/tasks.md`, Phase 1).

Each artifact restates a **locked** rule in a precise, testable form, or
ratifies an **open artifact** via an accompanying ADR. No file here may embed a
guessed IPA inventory, a placeholder/fake SHA-256, or an inferred grammar
presented as ratified (steering §0.3; [ADR 0003](../docs/decisions/0003-open-normative-artifacts.md)).

## Contents

| File | Task | Kind | Status |
|---|---|---|---|
| [`grammar/epf-0.1.0.abnf`](grammar/epf-0.1.0.abnf) | T0101 | EPF grammar (ABNF over KDL 2.0) | Derived from locked §5 |
| [`grammar/epf-0.1.0.md`](grammar/epf-0.1.0.md) | T0101 | EPF constraint tables | Derived from locked §5 |
| [`grammar/eld-0.1.0.abnf`](grammar/eld-0.1.0.abnf) | T0102 | ELD grammar (ABNF over KDL 2.0) | Derived from locked §6 |
| [`grammar/eld-0.1.0.md`](grammar/eld-0.1.0.md) | T0102 | ELD constraint tables | Derived from locked §6 |
| [`grammar/integrity-0.1.0.md`](grammar/integrity-0.1.0.md) | T0103, T0105 | integrity.kdl grammar + canonical serialization | Ratified by ADR 0004 |
| [`canonical/epf-canonical-serialization.md`](canonical/epf-canonical-serialization.md) | T0104 | Canonical EPF KDL serializer profile | Ratified by ADR 0005 |
| [`schema/elang-ipa-repertoire-v1.schema.json`](schema/elang-ipa-repertoire-v1.schema.json) | T0111 | Repertoire JSON Schema | Structure locked by §7 |
| [`schema/ipa-v1.json`](schema/ipa-v1.json) | T0106–T0113 | **The approved IPA repertoire** (107/27/9) | Generated from UNITIPA 2020 (ADR 0008) |
| [`schema/json-encoding-policy.md`](schema/json-encoding-policy.md) | T0112 | JSON encoding + JCS policy | Derived from §7 |
| [`sources/SOURCES.md`](sources/SOURCES.md) | T0106–T0115 | Source URLs, hashes, JCS digest, licensing | Provenance (ADR 0008) |
| [`registry/error-codes.md`](registry/error-codes.md) | T0119 | Error-code registry + metadata schema | Derived from §11 |
| [`registry/kdl-2.0-compatibility.md`](registry/kdl-2.0-compatibility.md) | T0120 | KDL 2.0 compatibility review | Review note |
| [`registry/resource-policy-v0.1.0.md`](registry/resource-policy-v0.1.0.md) | T0121 | Extra-resource / manifest-coverage policy | Ratified by ADR 0006 |

## Generated and validated (ADR 0008)

- **`schema/ipa-v1.json`** — the approved IPA repertoire (107 base, 27
  combining, 9 spacing), generated reproducibly by
  `crates/elang-repertoire/src/bin/gen_repertoire.rs` from the UNITIPA 2020
  chart. Prosody excluded per §7.
- **Validator** — `elang-repertoire` implements the Unicode layer
  (NFC/category/noncharacter) and the IPA identifier parser against the pinned
  JSON. Pinned JCS digest: `190aae7b…64aa`.
- **Cross-check** — `chart_crosscheck.rs` verifies the base set against an
  independent transcription of the official chart grids.

## Still blocked before the freeze (T0123)

- **Unicode 18.0.0:** ICU4X currently provides Unicode 17.0 (ADR 0007). The
  repertoire is validated but not *frozen* until the pin is met.
- **Second human review** of the combining-diacritic and spacing-modifier
  inventories against the chart's Diacritics block (T0114).
- **Golden byte vectors:** binary header/index/footer vectors (T0118, OA-11)
  and canonical ELD goldens (OA-14) are produced in Phases 3/11/14.
