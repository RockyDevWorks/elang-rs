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
| [`registry/binary-reference-vectors.json`](registry/binary-reference-vectors.json) | T0118 | Binary header/index/footer reference vectors + offsets (OA-11) | Generated & verified — see below |
| [`registry/eld-canonical-vectors.json`](registry/eld-canonical-vectors.json) | T1121 | Canonical ELD golden vectors + SHA-256 (OA-14) | Generated & verified — see below |

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

- ~~**Unicode 18.0.0:** ICU4X currently provides Unicode 17.0 (ADR 0007). The
  repertoire is validated but not *frozen* until the pin is met.~~ **Done**
  (ADR 0007, revised): a first-party `elang-unicode` crate generates GC + NFC
  tables from the official pinned UCD 18.0.0; NFC passes the official
  18.0.0 `NormalizationTest.txt`. ICU4X is no longer in the dep graph.
- ~~**Second human review** of the combining-diacritic and spacing-modifier
  inventories against the chart's Diacritics block (T0114).~~ **Done**
  (2026-10-23): the repository owner compared the 27 combining diacritics
  and 9 spacing modifiers row-by-row against the Diacritics and (non-prosodic)
  Suprasegmentals blocks of the pinned UNITIPA 2020 chart. Outcome: match.
  See `specs/sources/SOURCES.md` §7; OA-1/OA-4 Ratified.
- **Golden byte vectors:** the binary header/index/footer vectors (T0118,
  OA-11) are **done** (see below); the canonical ELD goldens (OA-14) are
  **done** too (Phase 11 / issue #21, see below). The remaining release
  blocker is the T0123 freeze declaration and the `v0.1.0` tag: the OA-14
  artifacts must enter the normative manifest corpus and
  `PINNED_SPEC_CHECKSUM` must be regenerated (ADR 0011 §6).

## Binary reference vectors (T0118, OA-11)

- **`registry/binary-reference-vectors.json`** — byte-exact reference vectors
  for the §3 binary container: field tables (offset / width / little-endian
  encoding / value) and raw hex for the 32-byte header, 16-byte index header,
  five worked index entries (27-byte fixed layout + UTF-8 path, strict ascending
  sort) and the 64-byte terminal footer, plus the
  `index_offset + index_size == file_size − 64` relation worked out against a
  concrete minimal archive.
- **Fixtures** — `fixtures/valid/container/minimal-archive/` holds the raw
  `header.bin` / `index.bin` / `footer.bin`, the complete `archive.elang`, each
  with a lowercase-hex `.sha256` sidecar, an `expected.json` (conformance
  format) and a `layout.json` offset map.
- **Generator** — `crates/elang-conformance/src/binary_vectors.rs` serializes
  the exact little-endian layout and computes every offset, size and SHA-256
  from the real bytes; `cargo run -p elang-conformance --bin gen_binary_vectors`
  reproduces the artifacts byte-for-byte. No digest or offset is hand-written
  (steering §0.3; ADR 0003). The real index SHA-256 is
  `a34879d8e4b45c2a9c9f35be47d9c7111340cbe0c535f7cc79d697c1777efa35`.
  Resource frame bodies are labelled placeholders standing in for the Phase 4
  Zstd frames; the header/index/footer framing is fully normative.

## Canonical ELD golden vectors (T1121, OA-14)

- **`registry/eld-canonical-vectors.json`** — the published canonical ELD
  corpus: 13 vectors, each naming its fixture, its byte-exact golden and the
  lowercase-hex SHA-256 of that golden.
- **Fixtures** — `fixtures/valid/canonical/<case>/` holds the authored
  `input.eld`, the byte-exact `golden/canonical.eld`, its `.sha256` sidecar
  and an `expected.json` (conformance format). The authored sources are
  deliberately written in **non-canonical** declaration order, so the goldens
  demonstrate the ordering rules rather than echoing their input.
- **Generator** — `crates/elang-conformance/src/canonical_goldens.rs` runs the
  real pipeline (`elang_eld::parse` → `inventory::from_document` →
  `canon::to_string`) and computes every digest from the real bytes;
  `cargo run -p elang-conformance --bin gen_canonical_goldens` reproduces the
  artifacts byte-for-byte. No digest is hand-written (steering §0.3;
  ADR 0003).
- **Profile** — the canonicalization rules these vectors lock are steering
  §9/§9.1 and [ADR 0009](../docs/decisions/0009-residual-eld-canonicalization-edge-cases.md);
  the public description is in
  [`docs/public/format/canonical.md`](../docs/public/format/canonical.md).
- **Cases** — minimal inventory; group order; root order; allophone order;
  bare allophone; nested condition tree; duplicate/contradictory predicates;
  authored tie-bar variant; romanization case; permitted Unicode preserved;
  condition order; and two declaration-order variants that must canonicalize
  to identical bytes.
