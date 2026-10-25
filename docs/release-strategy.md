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

## Building the release artifact

The distributable source artifact is assembled by
`scripts/build-release-artifacts.sh`:

```sh
# Build the artifact and its SHA-256 sidecar into
# target/release-artifacts/elang-v0.1.0.tar.gz.
scripts/build-release-artifacts.sh

# Build twice and assert the two artifacts are byte-identical.
scripts/build-release-artifacts.sh --verify
```

The artifact's bytes depend on the file **contents** only: entries are
sorted by path, ownership is forced to `0:0`, every timestamp is forced to
the Unix epoch, the per-entry mode is forced to `u=rwX,go=rX` so the
archive does not inherit the building machine's umask (the executable bit
is preserved for files that already have one), and the gzip header carries
neither a filename nor a timestamp. Two builds of the same tree therefore
produce the same SHA-256, and the script refuses to report success unless
the archive's member names match the file list it was asked to pack.

The script **creates no tag, publishes nothing, signs nothing and makes no
network call**. The tag, the signature and the publication are the human
release step (T1617 / T1620); this script exists so that step is one
command and one decision rather than a hand-assembled directory.

**Platform: Linux.** The script requires GNU `tar`, because GNU tar and
BSD tar (macOS) do not write identical archives for the same inputs. It
refuses to run elsewhere rather than silently emitting a
platform-dependent artifact. Linux is the release platform — the
`reproducible-build` CI workflow and the release itself run on ubuntu.

`crates/elang-conformance/tests/release_packaging.rs` asserts the same
property in CI: two builds are byte-identical, the archive contains every
file the release claims to ship, and building it leaves the working tree
untouched.

## Release gate (v0.1.0)

A release is cut only when the final checklist in
[`.kiro/specs/elang-v0.1.0/tasks.md`](../.kiro/specs/elang-v0.1.0/tasks.md)
passes: no open normative artifact remains, every `MUST` has passing test
coverage, canonical bytes reproduce on all platforms, and malformed inputs fail
closed.
