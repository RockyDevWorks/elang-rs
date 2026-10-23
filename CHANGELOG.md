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

[Unreleased]: https://github.com/elang-lang/elang-rs/compare/HEAD
