# Security Policy

## Threat model

ELANG parsers and readers consume **untrusted** input: arbitrary `.elang`
archives, KDL resources and compressed frames supplied by third parties. The
implementation is designed to fail closed against:

- **Malicious archives** — truncated/oversized headers, footers and indexes;
  wrong hashes; integer overflow in offsets/sizes; extra or missing bytes.
- **Decompression bombs** — bounded streaming decode with per-resource and
  aggregate (8 GiB) budgets, declared-size cross-checks and Zstd window caps
  (128 MiB).
- **Path traversal** — strict NFC UTF-8 relative paths only; no absolute/drive
  paths, no `.`/`..`/empty segments, no NUL or backslash; safe extraction that
  rejects overwrite, traversal and symlink escape.
- **Parser abuse** — nesting-depth, token, string, node, comment and reference
  budgets enforced *during* parsing, before unbounded allocation.
- **Frame abuse** — exactly one independent standard Zstd frame per resource;
  dictionary, skippable, concatenated, truncated and trailing-byte frames are
  rejected.

### Non-goals / trust boundaries

- A SHA-256 integrity match proves the archive's bytes are internally
  consistent. **It is not a publisher signature** and does not prove authorship,
  ownership or authenticity. URN-like language identifiers likewise do not prove
  ownership.
- The media type `application/vnd.elang` is *proposed*, not asserted as formally
  IANA-registered.

## Memory-safety policy

- Safe Rust by default. `unsafe` is denied by lint except at audited FFI
  boundaries (the pinned libzstd linkage); each such block is justified in code
  and reviewed.
- No `unwrap()`/`expect()`/`panic!`/unchecked indexing on untrusted input
  (enforced by workspace clippy lints).
- Checked integer and offset arithmetic throughout; `overflow-checks` are on in
  all profiles.

## Supply chain

- External dependencies are **pinned to exact versions** in the root
  `Cargo.toml` `[workspace.dependencies]` and committed in `Cargo.lock`.
- libzstd is pinned to **1.5.7** for canonical reproducibility.
- CI runs dependency vulnerability and license audits (`cargo audit`,
  `cargo deny`). Dependency updates go through review and, when they affect
  canonical bytes or schema semantics, an ADR.

## Reporting a vulnerability

Please report suspected vulnerabilities privately to the maintainers rather than
opening a public issue. Include a minimal reproducing input where possible.
Reproducing archives should be shared as attachments, not pasted inline. We aim
to acknowledge reports promptly and will coordinate disclosure.

## Security testing

The fuzzing, property-based, resource-exhaustion and extraction-security
testing strategy — including the `cargo-fuzz` (libFuzzer) harnesses, the
proptest property tests, the per-target smoke budgets and the nightly
campaign schedule — is documented in
[`docs/security-testing.md`](docs/security-testing.md). The end-to-end
fuzz-finding triage procedure (finding → reproduction → regression test →
audit-trail index) is documented in
[`docs/fuzz-triage.md`](docs/fuzz-triage.md). The explicit numeric
limits enforced by the parser, the limits the parser does *not*
enforce, and the authenticity/non-repudiation gaps a defensive
deployment must layer above the reader are documented in
[`docs/security-limits.md`](docs/security-limits.md).

## Security limits and remaining trust limitations (T1520)

The numeric limits the reader enforces (6 GiB compressed / 8 GiB
uncompressed aggregate, 25,000 resources, 64-deep KDL nesting,
2 M nodes per resource, etc.) live in
[`crates/elang-core/src/limits.rs`](crates/elang-core/src/limits.rs)
and are documented exhaustively in
[`docs/security-limits.md`](docs/security-limits.md). The reader
also has explicit non-goals: no rate limiting, no sandboxing, no
streaming partial verification, no persistence of rejected inputs.
A defensive deployment that wants any of those must layer them
above the reader. See the "Defensive deployment checklist" section
of `security-limits.md` for the full list.

The reader proves the archive's bytes are internally consistent;
it does **not** prove the archive came from a trusted publisher.
SHA-256 integrity matches without signatures is tamper-evidence,
not tamper-prevention. URN-like language identifiers are naming
conventions, not verified ownership claims. The media type
`application/vnd.elang` is *proposed*, not IANA-registered. All
four limitations are stated explicitly in `security-limits.md` so
a reader cannot miss them.
