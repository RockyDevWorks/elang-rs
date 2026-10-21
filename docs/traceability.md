# Requirements → Code → Tests traceability matrix

> Every normative `MUST` in
> [`requirements.md`](../.kiro/specs/elang-v0.1.0/requirements.md) must have a
> task and, once implemented, a test. This matrix is updated with each PR
> (see the PR template). Status legend:
>
> - **Done** — implemented and covered by a passing test.
> - **Scaffolded** — module/skeleton exists; behavior not yet implemented.
> - **Planned** — not started; task tracked.
> - **Blocked** — waiting on an open normative artifact (ADR 0003).

## Phase 0 (governance) — Done

| Requirement | Code | Test / evidence | Status |
|---|---|---|---|
| REQ-GOV-WORKSPACE-001 | `Cargo.toml`, `rust-toolchain.toml` | `cargo build --workspace` green | Done |
| REQ-GOV-WORKSPACE-002 | `crates/*` (9 crates) | `cargo build --workspace` green; ADR 0002 | Done |
| REQ-GOV-DOCS-001 | `README.md`, `LICENSE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `.github/`, `docs/specification.md` | files present | Done |
| REQ-GOV-ADR-001 | `docs/decisions/` (template + 0001–0003) | files present | Done |
| REQ-GOV-SPEC-001 | `.kiro/specs/elang-v0.1.0/{requirements,design,tasks}.md`, this matrix | files present | Done |
| REQ-GOV-CI-001 | `.github/workflows/ci.yml` | workflow present (runs on push/PR) | Scaffolded |
| REQ-GOV-SUPPLY-001 | `[workspace.dependencies]` pins, `docs/supply-chain.md` | pins present | Done |
| REQ-GOV-FIXTURE-001 | `fixtures/README.md`, `fixtures/expected-result.schema.json` | files present | Done |
| REQ-GOV-RELEASE-001 | `CHANGELOG.md`, `docs/release-strategy.md` | files present | Done |
| REQ-GOV-OPENART-001 | `docs/open-artifacts.md`, ADR 0003 | files present | Done |
| REQ-CORE-VERSION-001 (partial) | `elang-core::version_domains` | `version_domains_are_locked` test passes | Done |

## Phase 1 (normative artifacts) — partial

| Requirement | Artifact | Evidence | Status |
|---|---|---|---|
| REQ-EPF-* (grammar) | `specs/grammar/epf-0.1.0.{abnf,md}` (T0101) | derived from locked §5; JSON-free | Done (artifact) |
| REQ-ELD-*, REQ-WHEN-* (grammar) | `specs/grammar/eld-0.1.0.{abnf,md}` (T0102) | derived from locked §6/§8 | Done (artifact) |
| REQ-INTEGRITY-001..008 (grammar) | `specs/grammar/integrity-0.1.0.{abnf,md}`, ADR 0004 (T0103/T0105) | OA-6 ratified | Done (artifact) |
| REQ-EPF-013, REQ-CANON-001 | `specs/canonical/epf-canonical-serialization.md`, ADR 0005 (T0104) | OA-7 ratified | Done (artifact) |
| REQ-IPA-004/005 (schema shape) | `specs/schema/elang-ipa-repertoire-v1.schema.json` (T0111) | valid JSON; no symbols | Done (shape only) |
| REQ-IPA-004/005 (encoding) | `specs/schema/json-encoding-policy.md` (T0112) | JCS method documented | Done (artifact) |
| REQ-ERR-001..003 | `specs/registry/error-codes.{md,json}` (T0119) | valid JSON; categories+codes | Done (artifact) |
| REQ-BIN-001/003/004/006/007/008/009/010/013/014/015/016 (reference vectors) | `specs/registry/binary-reference-vectors.json`, `fixtures/valid/container/minimal-archive/`, `elang-conformance::binary_vectors` (T0118, OA-11) | `elang-conformance` tests: `committed_fixtures_match_the_builder_byte_for_byte`, `sidecar_digests_are_real`, `footer_embeds_the_real_index_digest`, `framing_relation_holds_for_the_fixture`, `reference_doc_digests_match_the_bytes`, + 2 unit (7 pass) | Done (vectors) — the Phase 3 container reader/writer validates against these in T0320/T0322 |
| REQ-PKG-005 | `specs/registry/resource-policy-v0.1.0.md`, ADR 0006 (T0121) | OA-15 ratified | Done (artifact) |
| REQ-UNICODE-001..004 | `elang-unicode` (NFC + General_Category + noncharacter) | `elang-unicode` unit tests + NFC conformance (20,171 rows); `elang-repertoire::unicode` adapter tests | Done — Unicode **18.0.0** from pinned UCD (ADR 0007) |
| T0116 (Unicode reference data) | pinned UCD 18.0.0 (`specs/sources/ucd-18.0.0/`), generated `gc_tables.rs`/`nfc_tables.rs` | official `NormalizationTest.txt` 18.0.0 passes | Done (OA-13 ratified) |
| REQ-IPA-001..014 (inventory + parser) | `elang-repertoire::repertoire`, `::ipa`; `specs/schema/ipa-v1.json` | `repertoire::tests::*`, `ipa::tests::*`, `chart_crosscheck.rs` (30 tests pass); pinned JCS digest asserted | Done (generated & validated) — ADR 0008; freeze pending Unicode 18.0 + 2nd review |
| REQ-IPA (digest) | `PINNED_JCS_SHA256` + `canonical_jcs_sha256` | `pinned_digest_matches`, `jcs_digest_is_stable_and_nonempty` | Done |

> "Done (artifact)" means the normative document exists and is ratified; the
> corresponding Rust validator and its tests move it to a crate section as later
> phases implement and test against these artifacts.

## Phases 2–16 — Planned / Blocked

The following requirement groups are tracked to their tasks and will move to
**Scaffolded**/**Done** as each phase lands. None are implemented yet beyond the
version-domain constants above.

| Requirement group | Owner crate | Tasks | Status |
|---|---|---|---|
| REQ-CORE-VERSION-001/002 | `elang-core::version` (`SemVer`, `domains`) | `version::tests::*` (strict parse, no leading zeros) | Done |
| REQ-CORE-VERSION-003/004 | elang-container | T1204, T0301 | Planned (Phase 3) |
| REQ-CORE-MODEL-001/002 | `elang-core::ids` (`ResourcePath`, `GroupName`, `Romanization`, `IpaIdentifierText`) | `ids::tests::*` | Done |
| REQ-CORE-MODEL-004 | `elang-core::limits` | `limits::tests::*` | Done |
| REQ-CORE-MODEL-005 | `elang-core::checked` | `checked::tests::*` (overflow/underflow/OOB) | Done |
| REQ-CORE-MODEL-006 | `elang-core::bounded` (`ByteBudget`, `BoundedCounter`) | `bounded::tests::*` | Done |
| REQ-CORE-MODEL-007 | `elang-core::modes` (`Inspected`/`Verified`/`Validated`) | `modes::tests::*` | Done |
| REQ-CORE-MODEL-008 | workspace lints (`unwrap`/`panic`/`indexing_slicing`/arith) | `cargo clippy -D warnings` clean | Done (enforced) |
| REQ-CORE-MODEL (threat model, T0213) | `.kiro/specs/elang-v0.1.0/design.md` §4, `SECURITY.md` | threat model documents malicious archives, decompression bombs, path traversal, parser abuse (+ frame abuse, false trust), each mapped to defenses and requirement IDs; consistent across both docs and steering §3.5/§10 | Done (artifact) |
| REQ-CORE-MODEL-003 | elang-epf / elang-eld validated models | T0205 | Planned (Phases 6/9) |
| REQ-ERR-001/002 | `elang-core::error` (`Category`, `Error`, `Diagnostic`, `KdlSpan`) | `error::tests::*` | Done |
| REQ-LIMIT-009 | `elang-core::error::Diagnostics` (cap 100, no masking) | `diagnostics_cap_does_not_mask_failure` | Done |
| REQ-LIMIT-001..008,010 | elang-kdl / elang-eld | T0503–T0515, T0510 | Planned (Phase 5/10) |
| REQ-BIN-001..018 | elang-container | T0301–T0324 | Planned |
| REQ-FRAME-001..012 | elang-container | T0401–T0422 | Planned |
| REQ-READER-001..006 | elang-container | T1201–T1208 | Planned |
| REQ-PKG-001..005 | elang-container / elang-epf | T1204, T1205, T0411, T0121 | Planned |
| REQ-EPF-001..014 | elang-epf | T0601–T0618 | Planned |
| REQ-INTEGRITY-001..008 | elang-integrity | T0701–T0711 | **Blocked** (integrity KDL grammar — ADR 0003) |
| REQ-UNICODE-001..004 | elang-repertoire | T0801–T0805 | Planned |
| REQ-IPA-001..014 | elang-repertoire / elang-eld | T0106–T0115, T0806–T0825 | **Blocked** (IPA allowlist + digest — ADR 0003) |
| REQ-ELD-001..017 | elang-eld | T0901–T0930 | Planned (full validation blocked on IPA) |
| REQ-WHEN-001..015 | elang-eld | T1001–T1030 | Planned |
| REQ-CANON-001..009 | elang-eld | T1101–T1121 | Planned |
| REQ-WRITER-001..005 | elang-container | T1301–T1309 | Planned |
| REQ-CLI-001..003 | elang-cli | T1310–T1320 | Planned |
| REQ-ERR-003 (registry artifact) | `specs/registry/error-codes.{md,json}` | present | Done (see Phase 1/2 rows above) |
| REQ-CONF-001..005 | elang-conformance / ci | T1401–T1420 | Planned |
| REQ-SEC-001 | all | T1501–T1520 | Planned |

## How to update

When a task lands:

1. Move its requirement row into the appropriate per-crate section with the code
   path and the test name.
2. Change status to **Done** only when a test proves the behavior *and* at least
   one rejection path where relevant (see Definition of Done).
3. If a requirement is **Blocked**, link the open-artifact issue.
