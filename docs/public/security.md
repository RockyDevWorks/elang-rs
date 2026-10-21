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

## Memory-safety posture

- Safe Rust by default. `unsafe` is lint-denied except at the single audited
  libzstd FFI boundary.
- No panics or unchecked indexing on untrusted input (lint-enforced across the
  workspace).
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
