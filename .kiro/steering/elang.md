# ELANG — Kiro Project Steering and Engineering Contract

> **Place this file at:** `.kiro/steering/elang.md` (or use as the authoritative input to generate topic-specific Kiro steering files).  
> **Specification release:** `0.0.1-1` · **Container:** `0.1.0` · **EPF:** `0.1.0` · **ELD:** `0.1.0`  
> **Status:** implementation steering based on confirmed decisions. A few enumerated normative artifacts remain to be produced and reviewed; do not fabricate them.  
> **Priority:** newest confirmed decisions in this file supersede older drafts, especially rules concerning `frequency`, `distribution`, allophone braces, IPA/Unicode and canonicalization.

## 0. Instructions to Kiro (non-negotiable)

1. Implement **ELANG alone**, a portable constructed-language definition and package format and its Rust reference tooling. Do **not** build HORIZON, AION, ARKHE, ELANG Studio or ELANG Hub in this project.
2. Treat **MUST/MUST NOT** as mandatory, **SHOULD** as preferred and **MAY** as optional. Never silently change a locked requirement.
3. Treat `OPEN ARTIFACT` as unfinished work requiring an explicit specification artifact, review and test vector. Do not guess the exact IPA inventory, SHA-256 digests, or an unspecified integrity-manifest grammar and then call them ratified.
4. When older ELANG documents conflict with this file, follow this file. Preserve an `ADRs/` or `docs/decisions/` record of resolutions and cite the requirement and test.
5. Favor secure, bounded, streaming, deterministic code; no silent normalization or repair; no acceptance of ambiguous syntax.
6. Produce code, tests, fixtures, schemas, CLI, documentation and release automation in small reviewable tasks. Keep a requirements-to-tests traceability matrix.
7. A `validate` success must mean all 12 validation stages pass, not merely that the archive opens.
8. A package digest verifies integrity, **not publisher authenticity**. Never claim a verified signer or ownership based solely on hashes or URN-like IDs.

## 1. Scope and boundaries

ELANG defines a `.elang` custom binary package, the KDL 2.0 `package.epf` descriptor, a KDL 2.0 phoneme inventory `.eld`, SHA-256 integrity, deterministic canonical encoding and a Rust reader/writer/validator SDK. Its first linguistic module is **phonemes** only. Do not invent syntax for lexicons, syllable inventories, phonotactics, morphology, syntax, semantics, prosody, grammar, dependency management, repository ownership, signatures, audio or GUI; these are future modules or separate projects. The phoneme `when` predicates may refer to generator-assigned syllable positions, but ELANG v0.1.0 does not define the syllabifier or generator.

**Version domains are independent:** spec release `0.0.1-1`; binary container `0.1.0` (header major=0, minor=1); EPF `0.1.0`; ELD `0.1.0`; individual language release is strict `MAJOR.MINOR.PATCH`; canonical compression profile `elang-canonical-zstd-v1`. Reader initially supports binary 0.1 only. `Header/format.version` first two components MUST match header major/minor; patch cannot change binary grammar.

### 1.1 Proposed Rust workspace (architecture recommendation, not a previously ratified crate naming contract)

```text
elang/
  Cargo.toml
  crates/
    elang-core/           # IDs, errors, limits, validated models
    elang-container/      # header/index/footer, frame I/O, reader/writer
    elang-kdl/            # constrained KDL 2.0 parsing and canonical emission
    elang-epf/            # package.epf schema and validation
    elang-eld/            # phoneme model, IPA, when AST, canonical ELD
    elang-repertoire/     # pinned Unicode/IPA allowlist and verification
    elang-integrity/      # integrity manifest and SHA-256 verification
    elang-conformance/    # fixture runner and interoperability checks
    elang-cli/            # inspect, verify, validate, pack, unpack, canonicalize
  specs/                  # authoritative schemas and registries
  fixtures/               # positive, negative, byte-exact, fuzz seeds
  docs/                   # normative spec, ADRs, security and CLI docs
  .kiro/steering/         # this file
  .kiro/specs/            # task/requirements/design breakdowns
```

Prefer small stable public interfaces, typed validated models, `Result` errors, explicit byte-order helpers, bounded allocations, no `unwrap()` on untrusted input, safe Rust as default, streaming decompression and testable pure canonicalization. Separate parsing from semantic validation and serialization.

## 2. Package logical resources

Every compliant phoneme-only package contains:

```text
Header/format.version
mimetype
package.epf
META-INF/integrity.kdl
Linguistics/phonemes.eld  # path configurable via EPF manifest
```

- `Header/format.version`: UTF-8 `0.1.0` with optional single trailing LF, no BOM; max 64 bytes.
- `mimetype`: **exact** ASCII `application/vnd.elang`, no LF/BOM; max resource budget 64 bytes.
- `package.epf`: KDL 2.0; max 1 MiB uncompressed.
- `META-INF/integrity.kdl`: KDL 2.0 SHA-256 manifest; max 16 MiB uncompressed.
- Exactly one `.eld` phoneme resource, path declared by `manifest { phonemes "..." }`; max 1 GiB uncompressed. Other resources may exist only if allowed by finalized resource/manifest policy; do not invent module semantics.
- The integrity manifest hashes the **uncompressed bytes** of every other indexed resource, excluding itself. Define the final integrity KDL schema before implementing normative pack/verify behavior; an older draft may contain a *proposal*, not automatically a locked grammar.
- Media type string is proposed, not asserted as formally IANA-registered. URN-like language identifiers do not prove ownership.

## 3. Binary `.elang` container: physical contract

**Not ZIP.** Exactly one independent standard Zstandard frame per logical resource, followed by an index and footer. Physical order:

```text
[32-byte header][resource Zstd frame 0]...[resource Zstd frame N-1]
[16-byte index header][index entries][64-byte footer]
```

### 3.1 Header: exactly 32 bytes

| Offset | Width | Encoding | Requirement |
|---:|---:|---|---|
| 0 | 8 | raw | `45 4C 41 4E 47 0D 0A 1A` (`ELANG\r\n\x1A`) |
| 8 | 2 | u16 LE | major `0` |
| 10 | 2 | u16 LE | minor `1` |
| 12 | 4 | u32 LE | flags `0`; reject unknown/nonzero |
| 16 | 16 | raw | reserved all zero |

### 3.2 Index

Index header exactly 16 bytes: 8-byte ASCII `ELIDX001`; index version `u16 LE = 1`; entry format `u16 LE = 1`; resource count `u32 LE <= 25,000`.

Each entry has **27 fixed bytes plus its path bytes**:

| Field | Width | Encoding |
|---|---:|---|
| path_length | 2 | u16 LE |
| path | variable | UTF-8, exact `path_length` bytes |
| frame_offset | 8 | u64 LE |
| compressed_size | 8 | u64 LE |
| uncompressed_size | 8 | u64 LE |
| compression | 1 | u8, must equal `1` (Zstd) |

Index entries MUST be strictly sorted by raw UTF-8 path bytes; no duplicate paths. Frames MUST be contiguous in index order, starting immediately after the 32-byte header, with no holes, overlaps, unindexed data or trailing frame bytes. Index immediately follows the final frame. `index_size` includes the 16-byte index header.

### 3.3 Footer: exactly 64 bytes

| Offset within footer | Width | Encoding | Requirement |
|---:|---:|---|---|
| 0 | 8 | raw | ASCII `ELEND001` |
| 8 | 8 | u64 LE | `index_offset` |
| 16 | 8 | u64 LE | `index_size` |
| 24 | 32 | raw | SHA-256 of exact serialized index bytes |
| 56 | 8 | raw | reserved zero |

Footer is the **only terminal footer**; enforce `index_offset + index_size == file_size - 64` using checked arithmetic. Index size range `16..=33,554,432` bytes. Reject truncation, wrong hashes, overflow, inconsistent count/length and extra bytes.

### 3.4 Resource paths

Strict NFC UTF-8 relative slash-separated resource paths, at most **1024 UTF-8 bytes**; no leading `/`, no `\`, NUL, empty segment, `.` or `..` segment; unique and sorted by UTF-8 bytes. No filesystem extraction traversal. Reject invalid UTF-8, non-NFC, absolute/drive paths and platform-specific path tricks. Never normalize invalid paths silently.

### 3.5 Size ceilings and decompression

- Compressed archive maximum **6 GiB**; recommended **2 GiB**.
- Aggregate uncompressed resource bytes maximum **8 GiB**.
- At most **25,000** resources; index at most **32 MiB**.
- Zstd window at most **128 MiB**.
- Phoneme `.eld` maximum **1 GiB**; undeclared generic resource default max **1 GiB**, subject to final resource policy.
- Bound streaming output to declared `uncompressed_size`, per-resource maximum and aggregate budget; compare actual decompressed byte count with index.
- Use checked integer arithmetic, allocation caps and rejection on every overrun.

### 3.6 Zstandard frame contract

Every resource is exactly **one independent standard Zstd frame**, with frame content size **present** and frame checksum **present**. No external/shared dictionaries, dictionary ID dependency, concatenated frames, skippable frames, incomplete frames or trailing bytes. Compare declared frame content size to index `uncompressed_size`, verify checksum, reject window >128 MiB. Do not trust frame metadata without actual bounded decoding.

### 3.7 Writer profiles

- `Fast`: Zstd compression level **3**.
- `Compact`: level **9**.
- `Canonical`: level **9**, strict reproducible `elang-canonical-zstd-v1`.
- Canonical pinned **libzstd 1.5.7**, through `zstd-sys`, fresh `CCtx` per resource, exactly one `ZSTD_compress2()` over full resource bytes, level=9, `nbWorkers=0`, `checksumFlag=1`, `contentSizeFlag=1`, `dictIDFlag=0`, `windowLog=27`, long-distance matching disabled, no dictionary; remaining defaults pinned. Test byte reproducibility across runs/platforms. Canonical writer may need to buffer one resource to call `compress2`; reader remains streaming.
- Canonical container order uses sorted resource paths and contiguous frames; index/footer derived from exact emitted bytes. Other KDL canonical serialization rules are separate from compression profile.

## 4. Reader modes and validation order

Implement **exactly these 12 logical stages**, with typed errors and fail-closed semantics:

1. Archive preflight: file size, bounds, minimal structure.
2. Validate binary header.
3. Validate terminal footer and offsets.
4. SHA-256 verify index bytes.
5. Parse/validate index and paths.
6. Validate physical frame layout.
7. Inspect every Zstd frame header/size/window/checksum requirement.
8. Decode/validate `Header/format.version` and `mimetype`.
9. Parse/validate integrity manifest structure.
10. Parse/validate EPF.
11. Bounded-decompress **all** resources and verify SHA-256 integrity.
12. Parse and semantically validate ELD phoneme inventory.

Mode contracts: `inspect` completes stages 1–7; `verify` completes 1–11; `validate` completes 1–12. Do not call `inspect` a full integrity verification. A reader MAY reuse safely cached results but cannot skip the checks semantically.

## 5. EPF `package.epf` v0.1.0

KDL **2.0**, exactly **five** top-level nodes, each exactly once; source order arbitrary; canonical order: `epf`, `eld`, `language-version`, `metadata`, `manifest`. Reject unknown top-level nodes, duplicates, properties, extra arguments and disallowed child blocks.

```kdl
epf "0.1.0"
eld "0.1.0"
language-version "1.0.0"
metadata {
    identifier "urn:elang:language:rocky:aelith:1.0.0"
    name "Aelith"
    description "A small constructed language."
}
manifest {
    phonemes "Linguistics/phonemes.eld"
}
```

- `epf`, `eld`, `language-version`: exactly one string argument, no child block.
- `metadata`, `manifest`: no arguments, mandatory child blocks.
- `metadata.identifier`: required once, nonempty, <=512 UTF-8 bytes; exact `urn:elang:language:<namespace>:<language-name>:<version>`; namespace/name ASCII lowercase slugs, letters/digits/hyphens, begin/end alphanumeric; version strict `MAJOR.MINOR.PATCH` without leading zeros.
- `metadata.name`: required once, nonempty, <=256 UTF-8 bytes.
- `metadata.description`: optional at most once, <=4096 UTF-8 bytes; may be empty.
- `language-version` MUST equal URN suffix exactly.
- `manifest`: exactly one `phonemes` child, one nonempty string argument referencing a safe package-relative `.eld` path indexed in archive. No `modules` node in EPF v0.1.0. No unknown fields/properties/extra args/nesting.
- `epf` and `eld` node values must each be `0.1.0` for this implementation.
- Decide and publish exact canonical EPF whitespace/escaping and integrity-manifest KDL schema as explicit **open artifacts**; do not infer them from ELD canonical rules without a documented decision.

## 6. ELD phoneme inventory v0.1.0 — grammar and cardinalities

An ELD is KDL 2.0 containing **exactly one top-level `phonemes` node**, which has an explicit nonempty block, zero args and zero properties. Reject additional top-level nodes and empty/whitespace/comment-only files.

```kdl
phonemes {
    consonants {
        phoneme "t" {
            roman "t"
            allophone "ɾ"
            allophone "tʰ" {
                roman "th"
                when {
                    word-position "initial"
                }
            }
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
    diphthongs {
        phoneme "aɪ" {
            roman "ai"
        }
    }
}
```

### 6.1 Groups

- Exactly one `consonants` and one `vowels`, each with **>=1** root phoneme.
- Up to **16 groups total**, thus up to 14 custom groups; every custom group nonempty.
- Group names unique; custom group names ASCII regex **`[a-z]+(?:-[a-z]+)*`**, length **1–32 ASCII characters**; `consonants` and `vowels` reserved.
- Group nodes have zero args/properties and **mandatory** child blocks; only direct `phoneme` children allowed.
- Group source order has no semantics. Canonical: `consonants`, `vowels`, then custom groups in ASCII bytewise lexicographic order.
- Root IPA identifiers globally unique across groups and allophones; a sound cannot be duplicated by moving it into another group.

### 6.2 Root phonemes

- `phoneme "<IPA>" { ... }`: exactly **one IPA string argument**, **zero properties**, mandatory **nonempty** child block.
- Exactly **one** explicit `roman` node; optional **zero or one** `when`; zero to **16** direct `allophone` declarations.
- Root phonemes across all groups maximum **256**.
- Only these children allowed; reject `frequency`, `distribution`, `description`, arbitrary attributes, unknown nodes, duplicate singleton nodes, and nested unsupported nodes.
- Canonical child order: `roman`, optional `when`, allophones sorted by IPA NFC UTF-8 bytes.
- Root source declaration order has no semantic priority; canonical sort by IPA NFC UTF-8 bytes.

### 6.3 Allophones

- `allophone "<IPA>"`: exactly **one IPA string argument**, **zero properties**; must be direct child of root phoneme (never nested under an allophone).
- Optional zero-or-one explicit `roman`; optional zero-or-one `when`.
- **If no fields are present, use a bare leaf declaration** `allophone "ɾ"`; **`allophone "ɾ" {}` is invalid**. If fields exist, require a nonempty `{ ... }` child block.
- Omitted `roman` inherits parent's exact original-case romanization; inheritance does not count as an explicit duplicate.
- Omitted `when` means unconditional eligibility; explicit empty `when {}` is invalid.
- Parent root `when` and child allophone `when` are evaluated **independently**, not inherited. Multiple eligible pronunciations can coexist; generator selects, no weighting or declaration priority.
- If none eligible in context, definition remains valid; generator must avoid placement or choose a different root; no invented fallback.
- Canonical allophone order by IPA NFC UTF-8 bytewise, no semantic priority.

### 6.4 Romanization

- `roman` is a scalar leaf node: exactly one string argument, no properties, no child block, even empty.
- Explicit value must match **`[A-Za-z]{1,8}`** (1–8 ASCII letters), preserving case exactly in canonical output.
- Every **explicit** `roman` across all roots and allophones must be **globally unique using ASCII case-insensitive comparison**; e.g. `th` and `TH` collide. Inherited romanizations exempt.
- No `frequency` field exists anywhere in ELD v0.1.0.

## 7. Unicode 18.0.0 and ELANG IPA Repertoire v1

- **Pinned Unicode Standard/UCD:** **18.0.0** for all NFC and general-category behavior. Reject non-NFC input; never silently normalize. Pinned version must not drift with OS or dependency updates.
- **Pinned IPA source:** International Phonetic Association **2026 chart issue** (2015/2005 underlying revision). ELANG IPA Repertoire v1 is a *narrow explicit allowlist* of approved IPA base symbols, combining diacritics and spacing modifiers; the exact inventory remains an **OPEN ARTIFACT**.
- Repertoire source file **strict UTF-8 JSON**, no BOM, canonicalize using **RFC 8785 JCS**, SHA-256 hash canonical JSON bytes; distribute with validator and verify its pinned digest. Reject duplicate JSON keys, unknown fields and duplicate inventory entries. Inventory entries unique and sorted deterministically. Version metadata and arrays `base_symbols`, `combining_diacritics`, `spacing_modifiers`; exact schema must be published.
- Reject Unicode controls (`Cc`), `Cf` format characters, `Co` private-use, `Cn` unassigned/noncharacters and **all Unicode noncharacters** in every ELD string. NFC required for every ELD string, not just IPA. Unicode surrogates are not valid Unicode scalar values or valid UTF-8.
- Standalone stress marks, syllable boundary markers and tone notation excluded from phoneme/allophone identifiers; reserve for future linguistic/prosody features.
- Approved spacing modifiers and combining diacritics **must modify a valid base**; standalone modifiers/diacritics or combining mark before a base invalid.
- Identifier length **1–8 Unicode scalar values** AND maximum **4 logical IPA base units**; both limits independently enforced.
- Each logical IPA base unit allows at most **one modification**, either one combining diacritic or one spacing modifier, never both. NFC precomposed characters must be interpreted by canonical decomposition for base+modifier counting where applicable.
- Tie bars **U+0361** (upper) and **U+035C** (lower) connect **exactly two** IPA base symbols into **one logical base unit**; no overlap or chaining. Modifier for tied unit may occur **only after second base**. Multiple independent tied units allowed; mixed tied/untied units allowed.
- Untied `tʃ` and tied `t͡ʃ` are **distinct** IPA identities; `t͡ʃ` and `t͜ʃ` are **equivalent for uniqueness**. Preserve authored tie-bar code point in source/canonical output, but normalize upper/lower tie-bar only for comparison keys. No other phonetic/linguistic equivalence beyond Unicode canonical equivalence under strict NFC and explicit tie-bar equivalence.
- IPA identifier uniqueness is global across **all root and allophone** declarations, including tie-bar-equivalent forms. No case folding for IPA.
- `previous-phoneme`/`next-phoneme` references use the same validated IPA identifier syntax; resolve any allophone reference to its owning root.

### 7.1 IPA examples (conditional on final approved allowlist)

| Identifier | Rule outcome |
|---|---|
| `tʃ` | untied, 2 logical units |
| `t͡ʃ` | tied, 1 logical unit |
| `t͜ʃ` | tie-equivalent to preceding identifier |
| `t͡ʃa` | mixed, 2 logical units |
| `t͡ʃd͡ʒ` | independent tied units, 2 logical units |
| `t͡ʃʰ` | modifier correctly after tied second base |
| `tʰ͡ʃ` | invalid modifier placement |
| `tʰʷ` | invalid: 2 modifications on one base |
| `ʰ` | invalid standalone modifier |
| isolated U+0303 | invalid standalone combining mark |

## 8. `when` condition expression language

A root/allophone may contain **zero or one** `when` block; if present it MUST have explicit braces and **>=1** child expression, zero args/properties. No inheritance between root and allophone conditions.

Four atomic leaf predicates, each exactly **one string argument**, zero properties, **no child block**:

| Predicate | Allowed values / reference |
|---|---|
| `word-position` | `initial`, `medial`, `final` |
| `syllable-position` | `onset`, `nucleus`, `coda` |
| `previous-phoneme` | declared root or allophone IPA identifier |
| `next-phoneme` | declared root or allophone IPA identifier |

`when { expr ... }` and `group { expr ... }` implicitly AND direct children. `group` requires **>=2 direct expressions**. `or { expr expr }` requires **exactly 2** direct expressions; nested `or` permitted, including cross-category alternatives. No explicit `and` or `not` node. All `group`/`or` structural nodes: mandatory braces, zero args/properties. Unknown nodes invalid. Repeated predicates allowed, even contradictory; duplicate conditions across allophones allowed.

Limits: **<=16 atomic predicates per `when`** across all branches; **<=8 nested `group`/`or` levels** along any expression path (root `when` and leaves excluded). Independent generic KDL parser max depth 64.

**Semantics:**
- `initial` first underlying root phoneme in word; `final` last; `medial` neither. A single-root-phoneme word is **both initial and final**, never medial.
- Syllable positions refer to generator-assigned onset/nucleus/coda components, not inferred from vowel/consonant group; all phonemes inside multi-phoneme components match that component.
- `previous-phoneme` and `next-phoneme` refer to immediately adjacent **underlying root identity**, across syllable boundaries but **not word boundaries**. Missing neighbor => false, not validation error. References to an allophone resolve to parent root. Forward/self references allowed after global inventory resolution; no circularity from surface pronunciation.
- Root/allophone eligibility independent; declaration order and overlapping conditions do not imply priority or probabilities. No fallback if none eligible.

```kdl
when {
    or {
        group {
            word-position "final"
            syllable-position "coda"
        }
        previous-phoneme "s"
    }
}
```

## 9. Canonical ELD serialization (byte-deterministic intent)

- Input declaration order unrestricted; canonical group order: mandatory `consonants`, `vowels`, then custom groups sorted ASCII bytewise.
- Within each group, roots sorted by NFC IPA identifier UTF-8 bytewise. Within each root, allophones sorted by NFC IPA identifier UTF-8 bytewise.
- Root child order: `roman`, `when` (if any), allophones. Allophone child order: explicit `roman` (if any), `when` (if any). Bare allophone has no braces; allophone with fields has a nonempty block.
- In `when`/`group`/`or`, sort direct child expressions recursively by **Canonical Condition Sorting Key v1** (defined below). Preserve tree structure and duplicate predicates; no Boolean simplification, no flattening, no changing references to owning roots.
- Indent exactly **4 ASCII spaces per nesting level**, no tabs; one declaration per line; **zero blank lines**; LF only; **exactly one final LF**; omit all comments. Source may use comments and KDL 2.0 whitespace.
- Canonical strings always standard double-quoted; escape embedded `"` and `\\`; preserve permitted Unicode directly in UTF-8 and authored ASCII romanization case. ELD string controls, Cf, Co, Cn/noncharacters prohibited, so no canonical control escape is needed for valid values.
- Preserve authored validated NFC strings; never silently normalize.

### 9.1 Canonical Condition Sorting Key v1 — exact binary format

Rank `u8`: `word-position=0`, `syllable-position=1`, `previous-phoneme=2`, `next-phoneme=3`, `group=4`, `or=5`.

```text
atomic_key = rank:u8 || value_utf8_length:u16BE || exact_validated_value_utf8
composite_key = rank:u8 || child_count:u16BE ||
                repeated(sorted_child_key_length:u32BE || sorted_child_key)
```

Sort children recursively before constructing parent key; compare keys as **unsigned lexicographic byte strings**. Apply to children of `when` (AND), `group` (AND), `or` (two operands). No reordering across semantically different parent levels. No simplification or alias rewriting. `when` itself has no key rank because it is a root expression container.

## 10. Parser budgets and defensive limits

All sizes measured on input bytes or decoded values as stated; enforce *during* parsing, not only after allocation:

| Limit | Maximum |
|---|---:|
| KDL syntax nesting depth | 64 |
| Encoded token bytes | 1 MiB |
| Decoded string bytes | 1 MiB |
| Node/property name UTF-8 bytes | 256 |
| Arguments per node | 256 |
| Properties per node | 256 |
| Nodes per resource | 2,000,000 |
| Nodes per archive | 10,000,000 |
| Single comment bytes | 1 MiB |
| Comment bytes per resource | 16 MiB |
| Decoded string bytes per resource | 256 MiB |
| Decoded string bytes per archive | 1 GiB |
| Semantic references per archive | 10,000,000 |
| Semantic traversal depth | 128 |
| Retained diagnostics | 100 |

Use checked counters and cap decompression and memory; prevent quadratic nested-condition and large-string behavior. Diagnostic truncation must not turn a failure into success.

## 11. Error and conformance design

Define stable machine-readable error identifiers and typed variants (proposed categories: `CONTAINER`, `INDEX`, `FRAME`, `RESOURCE_PATH`, `INTEGRITY`, `EPF_SCHEMA`, `ELD_SCHEMA`, `UNICODE`, `IPA`, `ROMAN`, `WHEN`, `LIMIT`, `CANONICAL`). Preserve byte offset/resource path/node span where available; no panics on malicious input. Define exact error-code registry as a generated spec artifact, not invented ad hoc in each crate.

Conformance suite name: **`elang-canonical-zstd-v1-conformance`**. Include:
- Golden minimal valid package, multi-resource valid package, canonical reproducibility and fixed SHA-256 byte hashes.
- Header/index/footer offset and boundary vectors, endian checks, malformed/truncated/overflowed index, bad footer hash, bad path ordering, frame gaps/overlaps, unindexed bytes.
- Frame checksum/content-size/window/dictionary/skippable/concatenated-frame negatives; decompression bombs and budgets.
- EPF valid/invalid metadata, URN slugs/version mismatches, duplicate/unknown fields, missing/invalid manifest path.
- ELD group/phoneme/allophone limits, required fields, empty-block rules, strict unknown fields, Unicode NFC/categories/noncharacters, IPA lengths/ties/modifiers/uniqueness, romanization case collisions and inheritance.
- `when` arity, AND/OR, depth/predicate counts, forward/self references, allophone alias references, cross-syllable but not cross-word neighbors, single-phoneme initial+final, contradictory predicates, no eligible realization.
- Canonical byte-for-byte ordering/formatting/string cases and condition-key vectors.
- Differential parsing and property-based tests; fuzz KDL, index, footer, Zstd, IPA parser, condition evaluator, canonical writer.
- CI: Linux/Windows/macOS; pinned toolchains/dependencies; deterministic release artifacts; dependency/license audits.

## 12. Open artifacts and release gates (do not pretend these are done)

1. **Exact approved IPA allowlist** for the pinned 2026 chart, with classification and canonical-decomposition handling; machine-readable JSON schema and published file.
2. **Actual SHA-256** of canonical IPA JSON; cannot be chosen before the exact bytes exist.
3. **Complete `META-INF/integrity.kdl` normative grammar**, including exact canonical serialization and whether/how it records per-resource sizes/algorithms.
4. **Exact canonical EPF and integrity KDL serializer profiles** (ELD rules are locked; do not assume they apply automatically).
5. **Complete machine-checkable ELD/EPF grammars**, exact JSON Schema and edge-case handling (e.g. classification of any approved spacing IPA symbol that could otherwise be interpreted as a base).
6. **Reference implementation and binary golden fixtures**, reproducibility hashes, versioned conformance corpus and stable error registry.
7. **Security review** and conformance evidence; performance and memory benchmarks.

**Definition of done for v0.1.0:** all required artifacts exist, no normative ambiguity remains, pack→inspect→verify→validate→unpack roundtrip passes, golden bytes/hashes reproducible, malformed inputs fail closed, fuzz/property tests pass, docs match implementation, CI release builds are green. If a normative gap cannot be resolved without product input, create a specific blocking ADR rather than inventing an unreviewed rule.

## 13. Recommended execution sequence for Kiro

1. Convert this steering contract into versioned `requirements.md`, `design.md`, `tasks.md` and traceability table.
2. Ratify outstanding schemas/data files (IPA, integrity KDL, canonical EPF) and publish test vectors **before** freezing APIs.
3. Build foundational typed models, limits, errors, Unicode/IPA validator and KDL adapters.
4. Implement binary header/footer/index and bounded Zstd reader; validate each layer independently.
5. Implement EPF, integrity and ELD validators; condition AST/evaluator and canonical ELD writer.
6. Implement canonical container writer, CLI and fixtures; integrate 12-stage modes.
7. Fuzz, benchmark, security-review, document and release a versioned Rust SDK.

---

**Reference:** ELANG Granular Normative Technical Specification `0.0.1-1` and confirmed design decisions through October 2026. This steering document resolves conflicts in older drafts; it does not certify that open artifacts have already been completed.
