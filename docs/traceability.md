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

## Phases 1–16 — Planned / Blocked

The following requirement groups are tracked to their tasks and will move to
**Scaffolded**/**Done** as each phase lands. None are implemented yet beyond the
version-domain constants above.

| Requirement group | Owner crate | Tasks | Status |
|---|---|---|---|
| REQ-CORE-VERSION-002..004 | elang-core / elang-container | T0202, T1204, T0301 | Planned |
| REQ-CORE-MODEL-001..008 | elang-core | T0203–T0215 | Planned |
| REQ-LIMIT-001..010 | elang-kdl / elang-core | T0503–T0515, T0211 | Planned |
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
| REQ-ERR-001..003 | elang-core | T0209, T0210, T0119 | Planned |
| REQ-CONF-001..005 | elang-conformance / ci | T1401–T1420 | Planned |
| REQ-SEC-001 | all | T1501–T1520 | Planned |

## How to update

When a task lands:

1. Move its requirement row into the appropriate per-crate section with the code
   path and the test name.
2. Change status to **Done** only when a test proves the behavior *and* at least
   one rejection path where relevant (see Definition of Done).
3. If a requirement is **Blocked**, link the open-artifact issue.
