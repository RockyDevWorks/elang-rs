# CLI reference

The `elang` binary (from the `elang-cli` crate) is the command-line front end to
the reader, writer, and validator. It holds no validation logic itself — it is a
thin, safe wrapper over the library crates.

!!! note "Preview"
    The CLI is part of the in-progress v0.1.0 release. Command names and flags are
    documented here as the intended stable surface.

## Commands

```text
elang inspect   <archive>                                  stages 1–7  (structure only, no integrity)
elang verify    <archive>                                  stages 1–11 (all resources hashed)
elang validate  <archive>                                  stages 1–12 (full ELD semantic validation)
elang pack      --epf <file> --eld <file> --output <file>   build a package
                [--profile fast|compact|canonical]
                [--format-version <version>]
elang unpack    <archive> --output <dir> [--force]          extract safely (no traversal / symlink escape)
elang canonicalize-eld <ELD-file>                          rewrite an ELD into canonical form
elang repertoire info                                       show the pinned chart / Unicode / digest
```

Every subcommand accepts `-h` / `--help`, which prints that subcommand's usage
on stdout and exits **0**. Running `elang` with no subcommand prints the top-level
usage and exits **0**.

!!! warning "`pack` takes three explicit inputs, not a directory"
    The CLI does **not** accept a source directory. `pack` reads one EPF
    document and one ELD document and writes one archive. The EPF and the ELD
    are the *authoring* artifacts; the archive is the *published* artifact.

### `inspect`

Runs validation stages 1–7: header, footer, index-hash, index/paths, frame
layout, and per-frame headers. It lists the resources it finds **without
decompressing them**.

```sh
elang inspect examples/hello-en/hello-en.elang
```

!!! warning
    `inspect` is **not** a full integrity check. Use `verify` or `validate` to
    confirm contents.

### `verify`

Runs stages 1–11. Decompresses every resource within the size budgets and checks
each against the SHA-256 in the integrity manifest.

```sh
elang verify examples/hello-en/hello-en.elang
```

### `validate`

Runs the full pipeline, stages 1–12, including ELD phoneme semantics.

```sh
elang validate examples/hello-en/hello-en.elang
```

### `pack`

Builds an archive from an EPF document and an ELD document, canonicalizing
resources, computing the integrity manifest, and laying out the container.

```sh
elang pack \
  --epf examples/hello-en/package.epf \
  --eld examples/hello-en/Linguistics/phonemes.eld \
  --output hello-en.elang \
  --profile canonical
```

Writer profiles: **Fast** (Zstd 3), **Compact** (Zstd 9), and **Canonical**
(reproducible `elang-canonical-zstd-v1`). See
[Writer profiles](format/container.md#writer-profiles).

`--profile canonical` is the reproducible profile: packing the same EPF and ELD
twice on the same toolchain produces byte-identical archives. The
`fast` and `compact` profiles are **not** reproducible — they trade
canonical byte order for speed and size.

`--format-version` overrides the container format version written into the
header. It defaults to the crate's own version. Setting it to a version the
writer does not implement is refused.

### `unpack`

Extracts a package to a directory. Extraction refuses to overwrite files and
rejects path traversal and symlink-escape attempts.

```sh
elang unpack examples/hello-en/hello-en.elang --output ./out
```

`--force` permits overwriting files that already exist at the destination. It
does **not** relax the path-traversal or symlink checks.

### `canonicalize-eld`

Rewrites an ELD document into its canonical byte form and prints it to
**stdout** — it takes one operand and writes no file. Running it on
already-canonical input is a no-op (canonicalization is idempotent).

```sh
elang canonicalize-eld examples/hello-en/Linguistics/phonemes.eld > phonemes.canonical.eld
```

To write the canonical form to a file, redirect stdout. The CLI has no
`-o` flag for this subcommand.

### `repertoire info`

Prints the pinned IPA chart issue, the pinned Unicode version, and the pinned
repertoire digest.

```sh
elang repertoire info
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. The archive verified, validated, or was written. |
| `1` | Operation failure. The reader, writer, or the filesystem rejected the operation. |
| `2` | Usage error. A required flag was missing, an operand was missing or unexpected, or the subcommand was not recognised. |

The distinction between `1` and `2` is who is at fault: `2` means the
*invocation* was wrong, `1` means the *input* was wrong (or the write failed).
A CLI usage error also prints the subcommand's usage text.

## Diagnostics

Diagnostics are available in two forms.

### Human-readable (default)

Results go to **stdout**; errors go to **stderr**. Human-mode error text
carries the stage, the resource path, and a stable error code in square
brackets:

```text
verify /tmp/bad.elang: [FRAME:FRAME_DECOMPRESS_FAILED] libzstd rejected the frame body
```

The message itself is **not** a stable interface — it can be reworded, it
embeds absolute paths, and it may be localised in a future release. The stable
part is the **error code**.

### Machine-readable (`--json`)

`--json` is a **global** flag: it may appear before or after the subcommand.
In JSON mode the CLI writes exactly **one JSON object to stdout** and nothing
else — human-readable output is suppressed so the stream stays parseable.

Success:

```json
{
  "ok": true,
  "command": "verify",
  "exit_code": 0
}
```

Failure:

```json
{
  "ok": false,
  "command": "verify",
  "exit_code": 1,
  "error": {
    "code": "FRAME_DECOMPRESS_FAILED",
    "message": "verify /tmp/bad.elang: [FRAME:FRAME_DECOMPRESS_FAILED] libzstd rejected the frame body"
  }
}
```

Field by field:

- `ok` — `true` when the exit code is `0`.
- `command` — the subcommand that ran (`"pack"`, `"inspect"`, `"verify"`,
  `"validate"`, `"unpack"`, `"canonicalize-eld"`, `"repertoire"`).
- `exit_code` — the process exit code, duplicated here so a consumer that
  captures stdout does not also have to inspect `$?`.
- `error.code` — the stable OA-10 error-code registry code, or `null` when the
  failure was a CLI usage error (bad flags, missing operand) or a filesystem
  failure while reading an archive or writing output.
- `error.message` — the diagnostic text. Display only.

!!! warning "Key off `code`, not `message`"
    A machine consumer should branch on `error.code`. The codes are published in
    [`specs/registry/error-codes.json`](../../specs/registry/error-codes.json) and
    every published conformance fixture asserts one, so they are covered by the
    corpus. The message is not.

!!! note "`--json` is a diagnostic format, not an authenticity mechanism"
    A JSON envelope proves that the CLI produced the output. It says nothing
    about who produced the *package*. See the
    [v0.1.0 release notes](../release/v0.1.0/README.md) for the
    integrity-versus-authenticity distinction.

## Errors

Reader and writer diagnostics report a stable registry error code, with stage
and resource context where available. CLI usage errors and filesystem failures
while reading archives or writing output have no registry code (`error.code` is
`null` in JSON mode). See [Errors](errors.md).
