# Changelog

All notable changes to the ELANG reference implementation are recorded here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses independent version domains (see
[README](README.md#version-domains)); the crate/SDK version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Test-coverage expansion to 99%.** 98 new tests (1,516 passing, 0
  failing, 1 pre-existing ignored) raise the Codecov-equivalent line
  coverage of the workspace from **97.31% to 99.09%** (missed lines 508 → 176
  of 19,356). The measurement is the one the project's own CI produces: `cargo llvm-cov --workspace --all-features
  --lcov --output-path lcov.info`, then the LCOV `DA:` per-line records with
  `codecov.yml`'s ignore list applied — the same artefact
  `.github/workflows/ci.yml` uploads to Codecov. On llvm-cov's own summary
  "Lines" metric (which counts partially-covered lines as missed) the
  workspace moves from 96.08% to 97.85%; the gap between the two metrics is
  a roughly constant set of about 457 lines that llvm-cov's line table
  counts but that Codecov's per-line `DA:` records do not. The new tests
  are test-only: they add `#[cfg(test)]` modules and integration-test files
  and change no production behaviour, and
  no existing test was weakened, deleted or ignored. The 176 remaining
  uncovered lines are defensive arms that earlier validation stages already
  make unreachable (multi-gigabyte archive ceilings, `panic!` arms inside
  the crates' own tests, proptest failure closures), so closing them would
  require changing production behaviour rather than adding tests.

- **Release-artifact packaging (T1617 preparation).**
  `scripts/build-release-artifacts.sh` assembles the distributable source
  artifact (`elang-v0.1.0.tar.gz`) plus a SHA-256 sidecar. The artifact's
  bytes depend on file *contents* only — entries sorted by path, ownership
  forced to `0:0`, every timestamp forced to the Unix epoch, and a gzip
  header carrying neither filename nor timestamp — so two builds of the
  same tree produce the same digest. The script refuses to report success
  unless the archive holds exactly the files it was asked to pack — the
  archive's member names are compared with the file list, not just their
  count — and it refuses an `--out-dir` inside the repository unless that
  path is git-ignored, so the artifact can never be packed into itself.
  It **creates no tag, publishes nothing, signs nothing and makes no network
  call**; the tag, the signature and the publication remain the human
  release step. `crates/elang-conformance/tests/release_packaging.rs`
  asserts the same property in CI. The script requires GNU `tar` and
  therefore builds the release on Linux; it refuses an `--out-dir` inside
  the repository unless the output *directory* is git-ignored (a
  `*.tar.gz` glob would ignore the archive but not its `.sha256` sidecar),
  and `--verify` compares the digest of the artifact it actually delivers
  against the two scratch builds it proved reproducible. A path whose name
  contains a newline is refused by name: the member comparison cannot
  compare such a path correctly, so the build fails with a clear message
  instead of a misleading mismatch.

- **Portable coverage tests.** The new CLI error-path tests are gated per
  platform: the non-UTF-8 argument tests and the POSIX symlink and
  permission tests run on Unix, and the file-size-limit test runs on Linux
  only, because exceeding `ulimit -f` raises `SIGXFSZ` there and kills the
  process on macOS. The Windows and macOS CI jobs therefore compile and run
  the same suite instead of failing on a Unix-only API.

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

### Changed

- **Specification release bumped to `0.1.0`** for the v0.1.0 first release
  (was `0.0.1-1`). The dash-suffix on the previous value signalled that the
  spec was a working draft of the v0.1.0 line; the v0.1.0 spec corpus is now
  fully ratified, so the identifier now matches the release it ships in. Per
  REQ-CORE-VERSION-001 the seven version domains stay independent — the
  binary container, EPF schema, ELD schema, language release, canonical
  compression profile and crate versions are not touched. Normative
  sources updated (`.kiro/steering/elang.md`, `requirements.md` REQ-CORE-VERSION-001,
  `tasks.md` baseline, `specs/registry/error-codes.json`), implementation
  constants updated (`elang-core::{version_domains, version::domains}::SPEC_RELEASE`),
  public docs updated (`README.md`, `docs/specification.md`,
  `docs/public/versioning.md`), and the normative manifest regenerated
  with a new `PINNED_SPEC_CHECKSUM`. Gated by
  [ADR 0013](docs/decisions/0013-spec-release-bump-to-0.1.0.md).

### Fixed

- **Crate `repository` metadata pointed at a repository that does not
  exist.** All twelve crates inherit `repository` from
  `[workspace.package]`, which read
  `https://github.com/elang-lang/elang-rs` — a URL that returns 404. Every
  published crate would have carried a dead repository link, and crates.io
  derives the *Documentation* link from it. Corrected to
  `https://github.com/RockyDevWorks/elang-rs`.

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

- **Phase 16 (Issue #26) — Documentation, examples, benchmarks, release archival.**
  Closes T0123, T1601–T1616, T1618, and T1619 as one PR
  (`doc/26-documentation-examples-benchmarks-and-release`). The v0.1.0 tag,
  the signed release artifacts, and the final v0.1.0 declaration remain on
  the human side of the gate (T1617, T1620 — deferred per the agreed split).
  T1616 is now closed: three independent reviewers (specification
  conformance, adversarial code, security/test-adequacy) were dispatched
  with fresh contexts; all 41 findings are dispositioned in
  `docs/release/v0.1.0/review-record.md` and the security-claim table
  in `docs/release/v0.1.0/security-checklist.md`. The release-gate
  assessment is recorded honestly: conditions (b) every MUST covered and
  (c) multi-platform reproducible build remain unmet on the open-task list
  (T0420, T0510, T1319); condition (a) closes only when the human tags.
  The human sign-off block in both documents remains **unsigned** and is a
  prerequisite for T1617/T1620.
  - **Review-bot findings #38–#41 (CodeRabbit on PR #68).** The published
    conformance corpus now records the **reader modes that actually observe
    each fixture's outcome**: a rejection at stage *S* lists only the modes
    whose pipeline ceiling reaches *S* (it previously listed modes that
    *accept* the input, so five records told an independent implementation to
    expect rejections it would never see). The published schema,
    `fixtures/README.md` and the corpus records now agree on
    **fixture-directory-relative** paths for both `input` and
    `golden[].path` — the convention every pre-existing, manifest-covered
    record already used, so the OA-14 canonical goldens and
    `PINNED_SPEC_CHECKSUM` are byte-identical. The corpus drift test re-runs
    the real generator into a scratch tree and compares every emitted file
    byte-for-byte against the committed corpus, and `gen_example_package`
    gained `--out-dir`, so the integration test no longer overwrites the
    published example artefacts while `cargo test` runs.
  - **T0123:** the normative manifest corpus now includes the **40 OA-14
    artifacts** — `specs/registry/eld-canonical-vectors.json` plus the
    13 canonical ELD golden fixtures at three files each
    (`input.eld`, `golden/canonical.eld`, `expected.json`). Total
    artifact count grew from **31 to 71**. The derived `.sha256`
    sidecars are deliberately **not** frozen in the manifest; they are
    regenerated from the frozen bytes and are asserted to be absent by
    `derived_sha256_sidecars_are_not_frozen` in
    `crates/elang-conformance/tests/normative_manifest.rs`. `PINNED_SPEC_CHECKSUM` was regenerated
    to
    `681022fbdd9a91d4e017c187884de7e6912409bee4267399464f8e4dc7d72e46`
    and the doc comment now describes the expanded corpus. New integration
    tests (`tests/normative_manifest.rs`) pin OA-14 coverage; the existing
    module unit tests were migrated off the narrower `ARTIFACTS` list to a
    single `artifact_entries()` source of truth.
  - **T1601–T1607:** the public format docs under `docs/public/format/`
    remain the authoritative normative reference; this PR reconciles
    cross-references (two broken `../../` paths in `format/canonical.md`
    corrected to `../../../`) and the "mimetype" → "Header/mimetype" naming
    in `index.md` and `concepts.md` to match the reader's bootstrap path
    constant.
  - **T1608:** crate-level rustdoc is present on every library crate, and
    `crates/elang-reader/examples/quickstart.rs` is a runnable end-to-end
    quickstart that exercises `inspect`/`verify`/`validate` plus random
    access on the published example archive. Run it with
    `cargo run --example quickstart -p elang-reader`.
  - **T1609:** `docs/public/cli.md` now matches the real CLI surface — the
    `pack --epf/--eld/--output/--profile/--format-version` flag set, the
    `unpack --output/--force` flags, the `canonicalize-eld` one-operand
    stdout-only behaviour, and the documented exit-code table (`0` success,
    `1` operation failure, `2` usage error). The CLI gained a global
    `--json` flag that emits a single stable JSON envelope on stdout;
    `crates/elang-cli/src/json.rs` carries the schema and
    `crates/elang-cli/tests/json_output.rs` pins it. The doc update also
    introduces the integrity-vs-authenticity distinction — the JSON
    envelope proves the CLI produced the output, not that the *package* is
    authentic.
  - **T1610:** `examples/hello-en/` is the published worked example: a
    minimal English phoneme inventory with one consonant and one vowel, the
    `package.epf` source, the canonical `.elang` archive, its `.sha256`
    sidecar and the conformance `expected.json`. The
    `gen_example_package` binary regenerates the archive from the sources
    byte-identically, and `tests/example_package.rs` pins the
    reproducibility invariant.
  - **T1611:** `docs/release/v0.1.0/migration-policy.md` codifies the v0.1.0
    contract and the procedure for any future breaking change.
  - **T1612:** `gen_conformance_corpus` publishes the in-crate conformance
    catalog onto disk under `fixtures/valid/...` and `fixtures/invalid/...`,
    with `corpus_index.json`, `expected-result.schema.json` and
    `fixtures/CORPUS.md` as the human-readable companion. The corpus drift
    test (`tests/corpus_publication.rs`) re-runs the generator for the
    metadata and fails if the committed corpus diverges.
  - **T1613, T1614:** `docs/benchmarks.md`/`docs/peak-memory.md` carry the
    deterministic benchmark results (5 ops × 50 iterations × 5 warm-ups,
    median wall-clock; Linux `VmHWM` peak memory) generated by
    `gen_benchmarks` and `gen_peak_memory`. The harness is hand-rolled and
    dependency-free; non-Linux peak-memory is documented as methodology
    only.
  - **T1615:** `.github/workflows/reproducible-build.yml` runs a
    clean-container double build, compares SHA-256 hashes of every
    generated artifact, and re-verifies the canonical goldens — pinned to
    `ubuntu-latest` with `CARGO_INCREMENTAL=0` and `cargo build --frozen`.
  - **T1618:** `docs/release/v0.1.0/README.md` is the release notes draft
    with a "prepared, not declared" status banner, the per-task landing
    table, the integrity-vs-authenticity distinction and the frozen-archive
    list. T1616, T1617 and T1620 are explicitly out of scope.
  - **T1619:** `gen_release_archive` writes the archival checksum metadata
    (`Cargo.lock.sha256`, `docs/release/v0.1.0/dependencies.txt`,
    `docs/release/v0.1.0/repertoire.sha256`,
    `docs/release/v0.1.0/conformance-hashes.txt`). The normative manifest
    is published separately at `specs/registry/normative-manifest.json`.

[Unreleased]: https://github.com/RockyDevWorks/elang-rs/compare/HEAD
