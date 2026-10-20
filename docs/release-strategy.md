# Release strategy, versioning and provenance

Implements REQ-GOV-RELEASE-001 (T0014).

## Version domains

ELANG keeps independent version domains (spec, container, EPF, ELD, language
release, canonical profile). The **Rust SDK/crate version** is separate and
follows SemVer. A change in a format domain does not force an SDK major bump
unless it breaks the public Rust API.

## SemVer policy (SDK)

- **MAJOR** — breaking public API change, or a change that alters previously
  valid acceptance/rejection behavior.
- **MINOR** — backward-compatible features (new CLI flags, new read APIs).
- **PATCH** — backward-compatible fixes that do not change canonical bytes or
  accept/reject decisions.

Any change to **canonical bytes**, a **locked rule**, or **normative data**
requires an approved ADR and, where relevant, regenerated golden hashes — even
for an otherwise "patch" fix.

## Changelog

Every PR updates [`CHANGELOG.md`](../CHANGELOG.md) under `[Unreleased]`. At
release, the section is dated and a compare link added.

## Release artifacts and provenance

1. Release builds use the pinned toolchain and committed `Cargo.lock`
   (reproducible profile: `lto=thin`, `codegen-units=1`).
2. CI builds on Linux, Windows and macOS and verifies that canonical golden
   hashes reproduce bit-for-bit.
3. Release tarballs include the full Apache-2.0 license text and the pinned
   Unicode 18.0.0 / IPA repertoire data with their SHA-256 digests.
4. Artifacts are checksummed; signing is used where release infrastructure
   permits. **A build signature attests the artifact, not any ELANG package's
   publisher.**
5. The exact dependency set and pinned data hashes are archived with the release
   (T1619).

## Release gate (v0.1.0)

A release is cut only when the final checklist in
[`.kiro/specs/elang-v0.1.0/tasks.md`](../.kiro/specs/elang-v0.1.0/tasks.md)
passes: no open normative artifact remains, every `MUST` has passing test
coverage, canonical bytes reproduce on all platforms, and malformed inputs fail
closed.
