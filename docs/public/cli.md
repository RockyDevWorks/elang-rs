# CLI reference

The `elang` binary (from the `elang-cli` crate) is the command-line front end to
the reader, writer, and validator. It holds no validation logic itself — it is a
thin, safe wrapper over the library crates.

!!! note "Preview"
    The CLI is part of the in-progress v0.1.0 release. Command names and flags are
    documented here as the intended stable surface.

## Commands

```text
elang inspect   <file>                stages 1–7  (structure only, no integrity)
elang verify    <file>                stages 1–11 (all resources hashed)
elang validate  <file>                stages 1–12 (full ELD semantic validation)
elang pack      <dir>  -o <file>      build a package
elang unpack    <file> -o <dir>       extract safely (no traversal / symlink escape)
elang canonicalize-eld <in> -o <out>  rewrite an ELD into canonical form
elang repertoire info                 show the pinned chart / Unicode / digest
```

### `inspect`

Runs validation stages 1–7: header, footer, index-hash, index/paths, frame
layout, and per-frame headers. It lists the resources it finds **without
decompressing them**.

```sh
elang inspect mylang.elang
```

!!! warning
    `inspect` is **not** a full integrity check. Use `verify` or `validate` to
    confirm contents.

### `verify`

Runs stages 1–11. Decompresses every resource within the size budgets and checks
each against the SHA-256 in the integrity manifest.

```sh
elang verify mylang.elang
```

### `validate`

Runs the full pipeline, stages 1–12, including ELD phoneme semantics.

```sh
elang validate mylang.elang
```

### `pack`

Builds a package from a source directory, canonicalizing resources, computing the
integrity manifest, and laying out the container.

```sh
elang pack ./mylang-src -o mylang.elang
```

Writer profiles: **Fast** (Zstd 3), **Compact** (Zstd 9), and **Canonical**
(reproducible `elang-canonical-zstd-v1`). See
[Writer profiles](format/container.md#writer-profiles).

### `unpack`

Extracts a package to a directory. Extraction refuses to overwrite files and
rejects path traversal and symlink-escape attempts.

```sh
elang unpack mylang.elang -o ./out
```

### `canonicalize-eld`

Rewrites an ELD document into its canonical byte form. Running it on
already-canonical input is a no-op (canonicalization is idempotent).

```sh
elang canonicalize-eld phonemes.eld -o phonemes.canonical.eld
```

### `repertoire info`

Prints the pinned IPA chart issue, the pinned Unicode version, and the pinned
repertoire digest.

```sh
elang repertoire info
```

## Output and exit codes

- Diagnostics are available in a stable human-readable form and as machine-readable
  **JSON**.
- The CLI exits **non-zero** on any validation failure, with documented exit
  codes.
- Every failure reports the stage, the resource, and a stable error code. See
  [Errors](errors.md).
