# Getting started

This guide walks through installing the reference tooling, inspecting a package,
and running the full validator.

## Install

The reference implementation is a Rust workspace (`elang-rs`). Building it
requires a Rust toolchain.

!!! info "Toolchain and MSRV"
    The build pins `channel = "stable"` via `rust-toolchain.toml`. The minimum
    supported Rust version (MSRV) is **1.90**, declared on every crate and
    enforced in CI.

Clone and build the workspace:

```sh
git clone https://github.com/RockyDevWorks/elang-rs.git
cd elang-rs
cargo build --workspace
```

Run the test suite and lints:

```sh
cargo test   --workspace
cargo clippy --workspace --all-targets
```

The CLI binary is produced by the `elang-cli` crate. You can run it through
Cargo during development:

```sh
cargo run -p elang-cli -- --help
```

## The three reader modes

ELANG's reader runs a fail-closed pipeline of **12 validation stages**. Three
commands expose increasing depth:

| Command    | Stages | What it proves |
|------------|--------|----------------|
| `inspect`  | 1–7    | The container structure is well formed. **No integrity check.** |
| `verify`   | 1–11   | Every resource decompresses within budget and matches its SHA-256. |
| `validate` | 1–12   | Everything above, plus full ELD phoneme semantics. |

!!! warning "`inspect` is not a trust check"
    `inspect` deliberately stops before any resource is hashed. Only `verify`
    and `validate` confirm a package's contents match its integrity manifest.
    Never treat a successful `inspect` as integrity verification.

## Inspect a package

```sh
elang inspect mylang.elang
```

This decodes the header, footer, and index, checks the physical frame layout,
and reports the resources it finds — without decompressing them.

## Verify integrity

```sh
elang verify mylang.elang
```

This decompresses every resource within the size budgets and checks each one
against the SHA-256 recorded in `META-INF/integrity.kdl`.

## Validate fully

```sh
elang validate mylang.elang
```

This runs the complete pipeline, including the `.eld` `phonemes` module
semantics (the only linguistic module v0.1.0 defines): group and cardinality
rules, IPA identifier validity, romanization uniqueness, and `when` condition
structure.

A non-zero exit code means validation failed. The CLI prints a stable error code
and the stage and resource where the failure occurred, and can emit
machine-readable JSON diagnostics.

## Build a package

```sh
elang pack ./mylang-src -o mylang.elang
```

The writer canonicalizes each resource, computes the integrity manifest over the
exact uncompressed bytes, sorts resources by canonical path, and lays out the
container. Choose a writer profile depending on your goal:

- **Fast** — Zstd level 3, quickest to produce.
- **Compact** — Zstd level 9, smaller output.
- **Canonical** — Zstd level 9 with the locked, reproducible
  `elang-canonical-zstd-v1` settings, producing byte-identical output across runs
  and platforms.

## Extract a package

```sh
elang unpack mylang.elang -o ./out
```

Extraction is safe by design: it refuses to overwrite files and rejects path
traversal and symlink-escape attempts.

## Next steps

- Learn the [concepts and vocabulary](concepts.md).
- Read the [ELD reference](format/eld.md) to author your own phoneme
  inventory.
- Browse the full [CLI reference](cli.md).
