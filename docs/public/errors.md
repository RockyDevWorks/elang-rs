# Errors & diagnostics

ELANG validation is fail-closed and reports failures with **stable, machine-
readable** identifiers so that tooling and tests can rely on them.

## What every error carries

A failure reports, at minimum:

- the **validation stage** (1–12) at which it occurred,
- the **resource** involved (where applicable),
- a **stable error code**.

Where possible, errors also carry the **byte offset** and the **KDL source span**
so a problem can be located precisely in the offending document.

## Error categories

Error codes are grouped into stable categories, each covering one area of the
format:

| Category | Area |
|----------|------|
| `CONTAINER` | Header, overall container structure. |
| `INDEX` | Index header, entries, ordering. |
| `FRAME` | Zstd frame structure and limits. |
| `RESOURCE_PATH` | Path syntax, NFC, traversal. |
| `INTEGRITY` | Index hash and resource integrity. |
| `EPF_SCHEMA` | `package.epf` schema violations. |
| `ELD_SCHEMA` | ELD structural violations. |
| `UNICODE` | NFC, rejected categories, noncharacters. |
| `IPA` | IPA identifier and repertoire rules. |
| `ROMAN` | Romanization regex and uniqueness. |
| `WHEN` | `when` arity, depth, references. |
| `LIMIT` | Any exceeded parser or resource budget. |
| `CANONICAL` | Canonical serialization issues. |

## The error-code registry

The exact set of error codes is maintained as a generated specification artifact —
a stable registry with metadata — rather than invented ad hoc in each crate. The
registry is the authoritative list of codes and their meanings, and it backs both
the human-readable and JSON diagnostics emitted by the CLI.

## Diagnostics output

- **Human-readable** output is stable and intended to be read directly.
- **JSON** output is intended for tooling and CI.
- Retained diagnostics are capped (at 100). This cap exists to bound memory on
  hostile input; it never turns a failed validation into a success.

## No panics on bad input

Panic-prone constructs are lint-denied across the workspace. Malformed or
malicious input produces a typed error, never a crash or an out-of-bounds read.
