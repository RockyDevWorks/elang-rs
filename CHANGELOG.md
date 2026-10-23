# Changelog

All notable changes to the ELANG reference implementation are recorded here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses independent version domains (see
[README](README.md#version-domains)); the crate/SDK version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 0 scaffold: Cargo workspace with nine crates
  (`elang-core`, `elang-container`, `elang-kdl`, `elang-epf`, `elang-eld`,
  `elang-repertoire`, `elang-integrity`, `elang-conformance`, `elang-cli`).
- Pinned toolchain (`rust-toolchain.toml`) and MSRV policy (1.90).
- Governance: `README`, `LICENSE` (Apache-2.0), `CONTRIBUTING`,
  `CODE_OF_CONDUCT`, `SECURITY`, issue/PR templates.
- Specification artifacts: `docs/specification.md`, ADR template + ADRs 0001–0003,
  `.kiro/specs/elang-v0.1.0/{requirements,design,tasks}.md`,
  `docs/traceability.md`, `docs/open-artifacts.md`, `docs/supply-chain.md`,
  `docs/release-strategy.md`, fixture conventions.
- `elang-core::version_domains` locked version constants with tests.
- CI workflow (fmt, clippy, test, docs, audit, cross-platform build).
- **Phase 5 (Issue #15): KDL 2.0 parser adapter and resource budgets**
  ([ADR 0010](docs/decisions/0010-kdl-parser-hand-roll.md)). The
  `elang-kdl` crate now provides a hand-rolled KDL **2.0** parser, the
  KDL 2.0 source spans (`Position`, `SourceSpan`), the AST
  (`KdlDocument`, `KdlNode`, `KdlEntry`, `KdlValue`, `KdlChildren`,
  `KdlVersionPragma`), the 27-entry `ErrorCode` registry (all
  registered in `specs/registry/error-codes.{md,json}` and
  enforced by `elang-conformance::registry_completeness`), the
  budget-enforcing `BudgetTracker` (per-resource **and**
  archive-wide ceilings), the canonical double-quoted string emitter
  (ADR 0009 E-3 escaping set, no leading zeros, idempotent), and the
  entry points `parse` / `parse_with_budgets`. T0510 (semantic
  references / traversal depth) and T0511 (schema-level validation:
  duplicate singletons, extra args, unknown props/nodes) are explicitly
  downstream concerns — the schema layers in `elang-epf`,
  `elang-eld`, and `elang-integrity` consume the AST and enforce them
  in Phases 6/7/9. Test counts: 64 unit tests + 53 KDL-2.0 spec-fixture
  tests + 4 fuzz-harness tests + 6 consumer-wiring tests (under
  `elang-conformance`).

### Notes

- Phase 1 normative-artifact reconciliation (Issue #14): the binary
  header/index/footer reference vectors (T0118, OA-11), tie-bar equivalence
  and IPA comparison-key test vectors (T0117, OA-12), and the residual KDL
  canonicalization edge cases (T0122, OA-16) are now ratified and listed as
  such in `docs/open-artifacts.md`. The only remaining open artifacts are
  OA-14 (canonical ELD golden files, Phase 11 deliverable) and OA-17 (formal
  freeze + spec checksum/release tag, depends on the second human review of
  the IPA diacritic/modifier inventories per ADR 0008). No new normative
  contracts introduced; this update reconciles the documentation with the
  state of the repository.
- Phase 5 (Issue #15) closes T0501–T0509, T0512–T0515. T0510 (semantic
  references / traversal depth) and T0511 (schema validation) remain
  Planned for downstream phases, as documented in
  `docs/traceability.md` and the `open-artifacts.md` rules of engagement
  (the KDL surface parser is the implementation of OA-8/OA-9/OA-16's
  normative grammar, not a new normative artifact).

[Unreleased]: https://github.com/elang-lang/elang-rs/compare/HEAD
