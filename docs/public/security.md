# Security model

Every byte in an `.elang` package — the container bytes, the KDL text, and the
compressed frames — is treated as **untrusted input**. The reader is built to
open hostile archives safely: bounded, streaming, checked-arithmetic code with
hard limits and no panics on malformed input.

## Threats and defenses

| Threat | Vector | Defense |
|--------|--------|---------|
| Decompression bomb | Huge declared or real output | Bounded streaming decode; per-resource caps; 8 GiB aggregate budget; declared-size cross-check; window ≤ 128 MiB. |
| Integer overflow | Crafted offsets/sizes | Checked arithmetic throughout; range validation; overflow checks on in all profiles. |
| Out-of-bounds / panic | Truncated or garbage structures | No unchecked indexing; panic-prone constructs lint-denied; fuzzing. |
| Path traversal | `..`, absolute paths, backslashes, symlinks | Strict NFC relative-path validation; safe unpack policy. |
| Parser abuse | Deep nesting, huge tokens/strings/nodes | Budgets enforced **during** parsing, before unbounded allocation. |
| Frame abuse | Dictionary, skippable, concatenated, trailing frames | One-frame contract; disallowed frame features rejected. |
| False trust | Treating a hash or URN as authorship | Documented non-goal; integrity ≠ signature. |

## Parser budgets

Limits are enforced **during** parsing, not merely checked after allocation:

| Limit | Maximum |
|-------|--------:|
| KDL nesting depth | 64 |
| Encoded token bytes | 1 MiB |
| Decoded string bytes | 1 MiB |
| Node/property name bytes | 256 |
| Arguments per node | 256 |
| Properties per node | 256 |
| Nodes per resource | 2,000,000 |
| Nodes per archive | 10,000,000 |
| Comment bytes (single) | 1 MiB |
| Comment bytes per resource | 16 MiB |
| Decoded string bytes per resource | 256 MiB |
| Decoded string bytes per archive | 1 GiB |
| Semantic references per archive | 10,000,000 |
| Semantic traversal depth | 128 |
| Retained diagnostics | 100 |

Diagnostic truncation never turns a failure into a success.

## Container-level ceilings

The binary container adds its own ceilings on top of the parser budgets. These
are the container-level limits defined in `elang-core`'s `limits` module and
enforced by the crate that owns them:

| Limit | Maximum | Applies to | Enforced by |
|-------|--------:|------------|-------------|
| `MAX_COMPRESSED_ARCHIVE_BYTES` | 6 GiB | Total compressed archive size | Writer — `elang-writer` refuses to build an archive over the ceiling. The reader does not apply this ceiling. |
| `MAX_RESOURCES` | 25,000 | Resources per archive | Reader — stage 5 (index) rejects a declared count over the ceiling; writer — rejects the input set before building. |
| `MAX_INDEX_BYTES` | 32 MiB | Serialized index size | Reader — stage 3 (footer) rejects an index region outside the permitted range; writer — the footer it builds must fit the same range. |
| `MAX_GENERIC_RESOURCE_BYTES` | 1 GiB | Uncompressed bytes per resource | Reader — stage 5 rejects an over-ceiling declared per-resource size, and the stage 11 decode budget caps each decoded resource; writer — per-resource check before encoding. |
| `MAX_EPF_BYTES` | 1 MiB | `package.epf` uncompressed size | Reader — stage 10 decode budget plus the EPF parser's input-size guard; writer — per-resource check before encoding. |
| `MAX_INTEGRITY_BYTES` | 16 MiB | `META-INF/integrity.kdl` uncompressed size | Reader — stage 9 decode budget plus the integrity parser's input-size guard; writer — per-resource check before encoding. |
| `MAX_ELD_BYTES` | 1 GiB | Phoneme `.eld` uncompressed size | Reader — stage 12 decode budget; writer — per-resource check before encoding. |

!!! note "The CLI adds no ceilings of its own"
    The CLI delegates to the reader and the writer, so each ceiling is enforced
    wherever the crate that owns it runs. `inspect`, `verify`, and `validate`
    report reader-side ceilings with the failing stage; `pack` reports
    writer-side ceilings as writer errors.

## Memory-safety posture

- Safe Rust by default. The workspace sets `unsafe_code = "warn"` and CI
  escalates it to a hard error via `RUSTFLAGS = "-D warnings"`; six crates
  additionally carry a crate-level `#![deny(unsafe_code)]`. The one FFI
  boundary (the pinned libzstd linkage in `elang-container`) is covered by
  that CI escalation and a scoped, justified `#[allow]`. **The guarantee is
  CI-enforced, not compile-time-denied locally.**
- No panics or unchecked indexing on untrusted input: enforced by the
  workspace clippy lints `unwrap_used`, `expect_used`, `panic` and
  `indexing_slicing` (also `warn` locally, escalated by CI). Four production
  modules that parse untrusted bytes carry a scoped, justified
  `#![allow(clippy::indexing_slicing)]`; there the property is upheld by
  review and bounded-slice style rather than by the lint.
- Checked integer and offset arithmetic everywhere the container layer computes
  positions and sizes.

## Fail-closed behavior

Validation either passes **every** applicable stage or fails with a precise,
stable error. Nothing is silently normalized or repaired, and ambiguous syntax is
rejected rather than guessed. A `validate` success means all 12 stages passed —
not merely that the archive opened.

## Integrity is not authorship

A verified digest proves a package's contents are intact. It does **not** prove
who published it. URN-like identifiers do not establish ownership, and ELANG
v0.1.0 defines no publisher signatures. See [Integrity](format/integrity.md).

## Reporting a vulnerability

Security policy and private reporting instructions live in the repository's
`SECURITY.md`. Please report suspected vulnerabilities privately rather than in a
public issue.
