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
- **Phase 11 (Issue #21): canonical ELD serialization** (steering §9/§9.1,
  [ADR 0009](docs/decisions/0009-residual-eld-canonicalization-edge-cases.md)).
  `elang-eld::canon::to_string` / `to_bytes` serialize a validated
  `PhonemeInventory` into byte-deterministic canonical KDL 2.0, and
  `elang-eld::canon::key` implements the frozen Canonical Condition Sorting
  Key v1 (`atomic_key = rank:u8 ‖ len:u16BE ‖ value`,
  `composite_key = rank:u8 ‖ count:u16BE ‖ (len:u32BE ‖ child)*`, ranks
  `word-position`=0 … `or`=5). `elang-eld::inventory::from_document` joins the
  Phase 9 structural document and the Phase 10 condition AST into the
  validated core model without re-implementing any validation rule.
  Ordering uses the exact authored NFC IPA UTF-8 bytes — the ADR 0009 E-1
  tie-bar-folded key is uniqueness-only and is never used for ordering or
  emission. Duplicate predicates, contradictory conditions, tree nesting, the
  authored IPA reference, the tie-bar variant and the romanization case are
  all preserved verbatim.
- **OA-14 (canonical ELD golden vectors, T1121).**
  `specs/registry/eld-canonical-vectors.json` publishes 13 canonical ELD
  goldens with their SHA-256, and `fixtures/valid/canonical/<case>/` holds
  the byte-exact goldens, their `.sha256` sidecars, the authored `input.eld`
  sources and the conformance `expected.json` records. Everything is produced
  by the real pipeline via
  `cargo run -p elang-conformance --bin gen_canonical_goldens`; regeneration
  is byte-identical and no digest is hand-written.

### Notes

- Phase 11 (Issue #21) closes T1101–T1121 and ratifies OA-14, the last
  open normative artifact other than the OA-17 freeze itself. The only
  remaining v0.1.0 release step is T0123: add the OA-14 artifacts to the
  normative manifest corpus, regenerate `PINNED_SPEC_CHECKSUM` per ADR 0011
  §6, declare the freeze and create the `v0.1.0` tag. None of that is
  declared here.
- Phase 1 normative-artifact reconciliation (Issue #14): the binary
  header/index/footer reference vectors (T0118, OA-11), tie-bar equivalence
  and IPA comparison-key test vectors (T0117, OA-12), and the residual KDL
  canonicalization edge cases (T0122, OA-16) are now ratified and listed as
  such in `docs/open-artifacts.md`. The only remaining open artifacts are
  OA-14 (canonical ELD golden files — closed by Phase 11 / Issue #21, see
  above) and OA-17 (formal freeze + spec checksum/release tag, depends on
  the second human review of the IPA diacritic/modifier inventories per
  ADR 0008). No new normative contracts introduced; this update reconciles
  the documentation with the state of the repository.
- Phase 5 (Issue #15) closes T0501–T0509, T0512–T0515. T0510 (semantic
  references / traversal depth) and T0511 (schema validation) remain
  Planned for downstream phases, as documented in
  `docs/traceability.md` and the `open-artifacts.md` rules of engagement
  (the KDL surface parser is the implementation of OA-8/OA-9/OA-16's
  normative grammar, not a new normative artifact).

[Unreleased]: https://github.com/elang-lang/elang-rs/compare/HEAD
