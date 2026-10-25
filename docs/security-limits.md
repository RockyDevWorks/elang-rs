# Security limits and trust boundaries (Phase 15 / Issue #25, T1520)

This document is the **explicit scope of what ELANG v0.1.0 enforces**
and **what it does not enforce**. It complements:

- [`SECURITY.md`](../SECURITY.md) — the threat model in narrative
  form.
- [`docs/security-testing.md`](./security-testing.md) — how the
  threat model is exercised by automated tests.

A reader should be able to answer these questions from this
document alone:

1. What is the maximum-size, maximum-count, or maximum-depth input
   the parser will accept?
2. What does the parser do when an input exceeds that maximum?
3. What does the parser *not* check, even though an attacker
   might want to abuse it?
4. What proof of authenticity does the parser provide, and what
   proof does it *not* provide?

## How the limits are organised

The limits live in one place — `crates/elang-core/src/limits.rs`,
prefixed `MAX_*` / `MIN_*` / `RECOMMENDED_*`. Every parser reads from
this module rather than hard-coding magic numbers. The values are
asserted by `tests::ceilings_have_expected_values` so a stealth
change to a constant fails CI before merge.

There are five families of limits:

| Family | Concern | Source |
|---|---|---|
| Container ceilings | total archive size, resource count, index size | §3.5 |
| Per-resource ceilings | per-resource uncompressed size, declared vs. observed | §2, §3.5 |
| Path constraints | resource-path length, forbidden characters | §3.4 |
| KDL parser budgets | nesting, tokens, strings, nodes, comments | §10 |
| Semantic budgets | references, traversal depth, diagnostics | §8, §10 |

## Container ceilings

A `.elang` archive is bounded at the container level before any
resource is decoded.

| Constant | Value | Source | What happens when exceeded |
|---|---|---|---|
| `MAX_COMPRESSED_ARCHIVE_BYTES` | 6 GiB (6 × 2³⁰) | §3.5 | `Category::Container` + `*_TOO_LARGE` (the container crate uses `elang_core::Error` codes rather than a per-crate enum; the specific codes are recorded in `crates/elang-container/src/header.rs::decode` etc.) |
| `RECOMMENDED_COMPRESSED_ARCHIVE_BYTES` | 2 GiB (warning, not hard) | §3.5 | Diagnostic, not rejection |
| `MAX_UNCOMPRESSED_AGGREGATE_BYTES` | 8 GiB (8 × 2³⁰) | §3.5 | `BoundedCounter::record` clamps to ceiling and returns `Category::Limit` |
| `MAX_RESOURCES` | 25,000 entries | §3.5 | `Category::Index` + index-too-large code |
| `MAX_INDEX_BYTES` | 32 MiB (32 × 2²⁰) | §3.5 | `Category::Index` + index-too-large code |
| `MIN_INDEX_BYTES` | 16 bytes | §3.5 | `Category::Index` + index-too-small code |
| `MAX_ZSTD_WINDOW_BYTES` | 128 MiB (128 × 2²⁰) | §3.5 | `Category::Zstd` + window-too-large code |

The 6 GiB / 8 GiB split is the documented decompression-bomb
mitigation: even at the worst-case compression ratio of ~1.34, a
maliciously small compressed payload can blow up to at most 8 GiB
of total uncompressed bytes before the archive-level budget trips.

## Per-resource ceilings

Each resource is bounded independently of the archive-wide budget.

| Resource | Limit | Constant | Rationale |
|---|---|---|---|
| `Header/format.version` | 64 B | `MAX_BOOTSTRAP_RESOURCE_BYTES` | small fixed fields |
| `mimetype` | 64 B | `MAX_BOOTSTRAP_RESOURCE_BYTES` | small fixed field |
| `package.epf` | 1 MiB | `MAX_EPF_BYTES` | schema grammar is small |
| `META-INF/integrity.kdl` | 16 MiB | `MAX_INTEGRITY_BYTES` | entries grow linearly with resource count |
| phoneme `.eld` | 1 GiB | `MAX_ELD_BYTES` | full phonological inventory + conditions |
| any undeclared generic | 1 GiB | `MAX_GENERIC_RESOURCE_BYTES` | arbitrary opaque payload |

A resource whose **declared** uncompressed size already exceeds the
ceiling is rejected *before* decoding starts — `ResourceEntry::reserve`
runs the cross-check with `checked_add`, so an `offset +
compressed_size` overflow is impossible. The aggregate budget is
primed with the declared size; overproduction during decoding is
caught separately by `BoundedCounter::record` so a liar (declares
small, produces large) cannot exceed the budget.

## Resource-path constraints

Every resource path is validated before being used as a filesystem
target during extraction or as a lookup key.

| Constraint | Source | What happens when violated |
|---|---|---|
| UTF-8 only (no NUL, no truncated multi-byte) | §3.4 | `Category::ResourcePath` + `PATH_NUL` (NUL byte); non-UTF-8 sequences are filtered upstream by `std::str::from_utf8` before this constructor is called |
| NFC normalised | §3.4 | Rejected upstream by `elang_unicode::is_nfc`; this constructor's doc comment asserts the input is already NFC-checked |
| Non-empty | §3.4 | `Category::ResourcePath` + `PATH_EMPTY_SEGMENT` |
| Not absolute (`/foo`, `C:\foo`) | §3.4 | `Category::ResourcePath` + `PATH_ABSOLUTE` |
| No backslash (`\`) | §3.4 | `Category::ResourcePath` + `PATH_BACKSLASH` |
| No NUL byte (`\0`) | §3.4 | `Category::ResourcePath` + `PATH_NUL` |
| No `.` / `..` / empty segment | §3.4 | `Category::ResourcePath` + `PATH_EMPTY_SEGMENT` (covers all three) |
| Length ≤ 1024 UTF-8 bytes | §3.4 | `Category::ResourcePath` + `PATH_TOO_LONG` |

The path chain runs **before** any I/O: `is_nfc` first (cheapest
trap for a long malformed input), `ResourcePath::new` second. A
panic in either is a regression — the fuzz target `resource_path`
covers both.

The extraction CLI additionally:

- Rejects overwrite of an existing file (no clobber).
- Rejects traversal escape (`../` in a relative path is rejected by
  the validator above, but a symlink in the output directory is not).
- Does not follow symlinks during extraction (output is created
  with `O_NOFOLLOW` semantics where the platform supports it).
- Uses `Path::join` + canonicalization check before any `create` call.

## KDL parser budgets

Every KDL parser (EPF, ELD, integrity) charges the caller's budget
**during** parsing, before unbounded allocation. The per-counter
budgets live in `elang-core::limits` and are aggregated into a
[`BudgetTracker`](https://github.com/RockyDevWorks/elang-rs/blob/main/crates/elang-kdl/src/budget.rs)
in the KDL crate, so EPF and ELD cannot diverge.

| Constant | Value | What happens when exceeded |
|---|---|---|
| `MAX_KDL_NESTING_DEPTH` | 64 | `ErrorCode::NestingTooDeep` (`KDL_NESTING_TOO_DEEP`) |
| `MAX_ENCODED_TOKEN_BYTES` | 1 MiB | `ErrorCode::TokenTooLong` (`KDL_TOKEN_TOO_LONG`) |
| `MAX_DECODED_STRING_BYTES` | 1 MiB | `ErrorCode::StringTooLong` (`KDL_STRING_TOO_LONG`) |
| `MAX_NAME_BYTES` | 256 B | `ErrorCode::NameTooLong` (`KDL_NAME_TOO_LONG`) |
| `MAX_ARGS_PER_NODE` | 256 | `ErrorCode::TooManyArgs` (`KDL_TOO_MANY_ARGS`) |
| `MAX_PROPS_PER_NODE` | 256 | `ErrorCode::TooManyProps` (`KDL_TOO_MANY_PROPS`) |
| `MAX_NODES_PER_RESOURCE` | 2,000,000 | `ErrorCode::TooManyNodesResource` (`KDL_TOO_MANY_NODES_RESOURCE`) |
| `MAX_NODES_PER_ARCHIVE` | 10,000,000 | `ErrorCode::TooManyNodesArchive` (`KDL_TOO_MANY_NODES_ARCHIVE`) |
| `MAX_COMMENT_BYTES` | 1 MiB | `ErrorCode::CommentTooLong` (`KDL_COMMENT_TOO_LONG`) |
| `MAX_COMMENT_BYTES_PER_RESOURCE` | 16 MiB | `ErrorCode::CommentBudgetExceeded` |
| `MAX_DECODED_STRING_BYTES_PER_RESOURCE` | 256 MiB | `ErrorCode::StringBudgetResourceExceeded` |
| `MAX_DECODED_STRING_BYTES_PER_ARCHIVE` | 1 GiB | `ErrorCode::StringBudgetArchiveExceeded` |

The fuzz target `kdl_parser` enforces these via the
`BudgetTracker::nodes_archive()` counter — every successful node
construction increments the counter and asserts the ceiling before
returning.

## Semantic budgets

ELD's semantic phase (condition evaluation, semantic-reference
traversal) is bounded separately from the syntactic phase.

| Constant | Value | What happens when exceeded |
|---|---|---|
| `MAX_SEMANTIC_REFERENCES` | 10,000,000 | reserved for ELD's semantic phase; the corresponding error code is wired into `elang-core::Error` and any ELD surface that violates the bound will surface as `Category::Semantic` + the limit code |
| `MAX_SEMANTIC_TRAVERSAL_DEPTH` | 128 | reserved for ELD's semantic phase; same error shape as above |
| `MAX_RETAINED_DIAGNOSTICS` | 100 | warnings beyond the cap are *dropped*, errors are *always* retained |

The retained-diagnostics cap is the one place where a budget
overflow produces a different behaviour for errors vs. warnings.
Errors must never be truncated, so the cap is *diagnostic-only*.

## What the parser does NOT enforce

The limits above are what the parser enforces. The list below is
**equally important**: the limits the parser does *not* enforce,
and which a defensive deployment must layer above the ELANG reader.

### No rate limiting

The reader has no built-in rate limit. A single client can submit
arbitrarily many archives per second, each up to 6 GiB. A defensive
deployment must add a per-client quota (token bucket, leaky bucket,
or fixed window) **before** the reader is invoked.

### No resource-type detection beyond the path

A resource at `META-INF/integrity.kdl` is parsed as KDL because of
its path. The reader does not content-sniff. An attacker who can
write a KDL 2.0 syntax-error payload to a path *not* on the
intended list (e.g., `phonemes/foo.eld`) gets the payload rejected
as "not a recognised resource type", but the parser still ran.

### No streaming verification

The reader ingests the entire archive header + index + every frame
before reporting verification. A 6 GiB archive takes at least as
long to read as its compressed size / disk bandwidth. The reader
has no streaming partial-verification mode. For large archives,
the caller should hash incrementally (e.g., with a side-channel
SHA-256) if early-rejection matters.

### No sandboxing

The reader runs in the caller's process with the caller's
permissions. A successful exploit (i.e., a parser bug, which the
fuzz targets aim to make unreachable) would run with whatever the
caller has. The reader does not `seccomp`, does not drop
capabilities, does not chroot. A defensive deployment should
invoke the reader in a confined environment if the input is
untrusted (a sandbox, a container with no network, a VM).

### No resource budget at the API layer

The reader allocates the per-resource buffers and aggregate
counters based on the archive's *declared* sizes. A caller who
has a tighter budget (e.g., "no archive over 100 MiB for this
endpoint") must pre-check the `Header::archive_size()` and reject
*before* invoking the reader. The reader itself honours only the
documented `MAX_*` ceilings.

### No concurrency model

The reader is single-threaded. There is no parallel parser, no
thread-pool ingestion. A defensive deployment that needs
throughput must run multiple reader instances, each on its own
input. Sharing a single reader across concurrent calls is
unsupported.

### No persistence of rejected inputs

When the reader rejects an input, it does not save the rejected
bytes anywhere. A defensive deployment that wants to log the
rejected input (for incident analysis) must buffer or stream the
input itself and dump it after the reader returns. The reader
only returns the structured error and the byte offset / span of
the rejection point.

## What the parser does NOT prove about authenticity

This is the most important section. **The reader proves the
archive's bytes are internally consistent. It does not prove the
archive came from who you think it came from.**

### SHA-256 is not a signature

The integrity manifest (`META-INF/integrity.kdl`) records the
SHA-256 of every resource body. Verification confirms that the
bytes the reader received match the bytes the publisher declared.
This proves:

- The archive is not truncated mid-frame.
- The archive is not corrupted in transit.
- The archive's declared resource set is the actual resource set.

It does **not** prove:

- That the publisher is who they claim to be (no PKI, no chain of
  trust, no public-key infrastructure).
- That the archive was not created by an attacker who also
  computed the SHA-256 over their own bytes (an integrity manifest
  is a tamper-evidence tool, not a tamper-prevention tool).
- That the SHA-256 was computed over the *intended* bytes (a
  confused-deputy attack that swaps the canonical bytes before
  hashing defeats the manifest).

### URN-like language identifiers are not claims of ownership

The `package.epf` file contains a language identifier like
`urn:elang-lang:lang:en:ipa:phonemic`. This is a **naming
convention**, not a verified claim. An attacker can publish a
`.elang` archive with `urn:elang-lang:lang:en:ipa:phonemic` even
if they have no relationship to English phonology. The identifier
must be matched against a separately trusted source of language
names — e.g., a registry the deployment maintains, a DNS name
under the publisher's control, or a manually pinned identifier
list.

### The media type is not IANA-registered

`application/vnd.elang` is *proposed*, not registered with IANA.
It serves as a content-type hint in HTTP responses, but the IANA
registry does not yet list it. Until it does, a defensive
deployment should accept the type with the same care as any
unknown content type.

### The libzstd version is pinned, not the libzstd build

The reader pins libzstd 1.5.7 as the canonical reference for
byte-reproducibility of compressed frames. This is not a
security claim about libzstd itself — it is a claim that *every*
ELANG archive that was compressed with libzstd 1.5.7 will be
decompressed byte-exactly. A defensive deployment that runs the
reader linked against a different libzstd gets correct
decompression (the algorithm is stable across 1.5.x) but cannot
use the ELANG archive as a byte-reproducible format.

### The conformance corpus is not exhaustive

The conformance corpus (`crates/elang-conformance/`) covers the
KDL 2.0 spec surfaces that ELANG relies on, the IPA tie-bar
grammar, the canonicalisation rules, and the canonical
serialisation. It is not an exhaustive test of every possible
input — only the documented shapes. The fuzz targets extend the
surface with coverage-guided adversarial input, but the corpus
+fuzz is bounded by the wall-clock budget the CI matrix runs.

## Defensive deployment checklist

A defensive deployment should:

1. **Pre-check archive size** against the endpoint's own budget
   *before* invoking the reader. The reader's 6 GiB ceiling is
   the upper bound, not the operational budget.
2. **Run the reader in a confined environment** (sandbox,
   container, VM). The reader does not sandbox itself.
3. **Add rate limiting** at the network layer. The reader does
   not rate-limit.
4. **Match the language identifier** against a separately trusted
   registry. The reader does not verify the identifier.
5. **Match the media type** against the deployment's own content
   policy. `application/vnd.elang` is not IANA-registered.
6. **Stream-hash rejected inputs** if you want an audit trail of
   what was rejected. The reader does not persist rejections.
7. **Read the threat model in [`SECURITY.md`](../SECURITY.md)**
   before integrating. The threat model is the contract.

## Operational notes

### Single-threaded ingestion

The reader ingests one archive at a time. Concurrent reads require
multiple reader instances, each on its own input. Sharing a single
reader across concurrent calls is unsupported and may produce
race conditions in the budget counters.

### Memory budget

Peak RSS for a worst-case accepted archive is dominated by the
per-frame buffers. A 6 GiB compressed archive that decompresses
to 8 GiB aggregate holds ~8 GiB resident at peak (plus the
decompressor window, ~256 KiB per frame). A defensive deployment
should provision at least **archive-ceiling + 512 MiB** of RSS per
concurrent reader.

### Wall-clock budget

The reader's wall-clock cost is dominated by decompression. A
modern x86-64 core decompresses Zstd at ~500 MiB/s single-threaded.
A worst-case 6 GiB compressed → 8 GiB uncompressed archive takes
~16 seconds to decompress on one core. The reader adds a few
hundred milliseconds of overhead for parse + canonical emission.
A defensive deployment with a hard latency budget should pre-check
the declared size and reject before invocation.

### Cold-start cost

The first reader invocation in a process pays the IPA validator's
cold-start cost (~50 ms to load the pinned repertoire and build
the comparison-key table). Subsequent invocations are amortised
to ~negligible.

## See also

- [`SECURITY.md`](../SECURITY.md) — threat model and reporting.
- [`docs/security-testing.md`](./security-testing.md) — how the
  limits are exercised by automated tests.
- [`crates/elang-core/src/limits.rs`](../crates/elang-core/src/limits.rs)
  — the source of truth for every constant above. If this file
  changes, this document changes; `tests::ceilings_have_expected_values`
  enforces the values.
- [`crates/elang-kdl/src/budget.rs`](../crates/elang-kdl/src/budget.rs)
  — the `Budget` and `BudgetTracker` types that every parser
  charges.
- [`crates/elang-core/src/checked.rs`](../crates/elang-core/src/checked.rs)
  — the checked-arithmetic helpers used at every boundary.