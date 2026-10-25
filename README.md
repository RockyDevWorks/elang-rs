
<div align="center">

# ELANG

**Electronic Language — An open, standardized digital format for defining, preserving, and exchanging human and constructed languages.**

<br>

[![CI](https://img.shields.io/github/actions/workflow/status/RockyDevWorks/elang-rs/ci.yml?branch=main&label=CI&style=flat-square)](https://github.com/RockyDevWorks/elang-rs/actions/workflows/ci.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/RockyDevWorks/elang-rs/security.yml?branch=main&label=Security&style=flat-square)](https://github.com/RockyDevWorks/elang-rs/actions/workflows/security.yml)
[![Coverage](https://img.shields.io/codecov/c/github/RockyDevWorks/elang-rs/main?label=Coverage&style=flat-square)](https://codecov.io/gh/RockyDevWorks/elang-rs)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue?style=flat-square)](https://github.com/RockyDevWorks/elang-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?logo=rust&style=flat-square)](https://www.rust-lang.org/)
[![MSRV](https://img.shields.io/badge/MSRV-1.90.0-blue?logo=rust&style=flat-square)](https://www.rust-lang.org/)
[![Clippy](https://img.shields.io/badge/Clippy-Enabled-blue?style=flat-square)](https://github.com/RockyDevWorks/elang-rs/actions/workflows/ci.yml)

</div>


ELANG is a portable, deterministic definition and packaging format for
constructed languages, together with its Rust reference tooling. This repository
is the reference implementation: the `.elang` binary container, the KDL 2.0
`package.epf` descriptor, the KDL 2.0 `.eld` Linguistic Definition extension
(v0.1.0 ships the `phonemes` module only), SHA-256 integrity, canonical
reproducible encoding, and a reader/writer/validator SDK plus a CLI.

## Documentation

The public documentation site is built with [Zensical](https://zensical.org) from
the Markdown sources in [`docs/public/`](docs/public/) and configured by
[`zensical.toml`](zensical.toml). It is published to GitHub Pages by
[`.github/workflows/docs.yml`](.github/workflows/docs.yml) on pushes to `main`
that touch the docs, and is served at
<https://rockydevworks.github.io/elang-rs/>.

The internal normative material (the steering contract, ADRs, requirements, and
the open-artifact tracker) lives under [`docs/`](docs/) and `.kiro/` and is the
source of truth that the public site is derived from.

## Status

Pre-release, tracking **v0.1.0**. Several normative artifacts (the exact approved
IPA allowlist, its canonical SHA-256, and the `META-INF/integrity.kdl` grammar)
are *open artifacts* that must be produced and reviewed before the conformance
suite is frozen. See [`docs/specification.md`](docs/specification.md) and the
open-artifact tracker in
[`.kiro/specs/elang-v0.1.0/requirements.md`](.kiro/specs/elang-v0.1.0/requirements.md).

## Version domains

ELANG keeps several **independent** version domains. A change in one does not
imply a change in another.

| Domain | Value | Notes |
|---|---|---|
| Specification release | `0.1.0` | The normative spec this code targets |
| Binary container | `0.1.0` | Header major `0`, minor `1` |
| EPF (`package.epf`) | `0.1.0` | Descriptor schema |
| ELD (phoneme inventory) | `0.1.0` | `.eld` schema |
| Language release | strict `MAJOR.MINOR.PATCH` | Chosen by each language author |
| Canonical compression profile | `elang-canonical-zstd-v1` | Reproducible Zstd settings |

The reader supports binary `0.1` only. `Header/format.version`'s first two
components MUST match the header major/minor; the patch component cannot change
the binary grammar.

## Supported scope

A compliant phoneme-only package contains exactly these logical resources:

```text
Header/format.version
mimetype
package.epf
META-INF/integrity.kdl
Linguistics/phonemes.eld   # path configurable via the EPF manifest
```

ELANG does **not** define lexicons, phonotactics, morphology, syntax, semantics,
prosody, audio, GUI, dependency management, repository ownership or publisher
signatures. A verified package digest proves **integrity, not authorship**.

## Workspace layout

```text
crates/
  elang-core/         IDs, errors, limits, validated models
  elang-container/    header/index/footer, frame I/O, reader/writer
  elang-kdl/          constrained KDL 2.0 parsing and canonical emission
  elang-epf/          package.epf schema and validation
  elang-eld/          phoneme model, IPA, `when` AST, canonical ELD
  elang-repertoire/   pinned Unicode/IPA allowlist and verification
  elang-integrity/    integrity manifest and SHA-256 verification
  elang-conformance/  fixture runner and interoperability checks
  elang-cli/          inspect, verify, validate, pack, unpack, canonicalize
specs/                authoritative schemas and registries
fixtures/             positive, negative, byte-exact, fuzz seeds
docs/                 normative spec, ADRs, security and CLI docs
.kiro/                steering contract and the v0.1.0 spec breakdown
```

## Building

```sh
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets
```

### Toolchain and MSRV

The build pins `channel = "stable"` via [`rust-toolchain.toml`](rust-toolchain.toml).
The **minimum supported Rust version (MSRV)** is **1.90**, declared as
`rust-version` in every crate and enforced in CI. Raising the MSRV requires an
ADR under [`docs/decisions/`](docs/decisions/).

## CLI (preview)

The `elang` binary will expose:

```text
elang inspect   <file>              stages 1–7  (structure only, no integrity)
elang verify    <file>              stages 1–11 (all resources hashed)
elang validate  <file>              stages 1–12 (full ELD semantic validation)
elang pack      <dir> -o <file>     build a package
elang unpack    <file> -o <dir>     extract safely (no traversal/symlink escape)
elang canonicalize-eld <in> -o <out>
elang repertoire info               show pinned chart/Unicode/digest
```

`inspect` is **not** a full integrity check; only `verify` and `validate` hash
every resource. The CLI exits non-zero on any validation failure.

## Security and trust model

See [`SECURITY.md`](SECURITY.md). In short: ELANG defends against malicious
archives (decompression bombs, path traversal, parser abuse) with bounded,
streaming, checked-arithmetic code, and it never treats a hash or URN-like
identifier as proof of publisher identity.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE).
