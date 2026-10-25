# ELANG v0.1.0 — Requirements

> **Status:** derived from the authoritative steering contract
> [`.kiro/steering/elang.md`](../../steering/elang.md). Each requirement has a
> stable ID, a normative level (`MUST`/`MUST NOT`/`SHOULD`/`MAY`), the steering
> section it comes from, and the implementation task(s) that satisfy it (from
> [`tasks.md`](tasks.md)).
>
> Requirement IDs are **stable**: never renumber. Add new IDs; mark removed ones
> `Withdrawn`. The requirement→code→test matrix lives in
> [`../../../docs/traceability.md`](../../../docs/traceability.md).

## Conventions

- **Level** uses RFC 2119 terms as locked by steering §0.2.
- **Owner** is the crate primarily responsible.
- "Tasks" are the checklist IDs that implement and verify the requirement.

---

## REQ-CORE-VERSION — Version domains (steering §1)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-CORE-VERSION-001 | MUST | Maintain independent version domains: spec `0.1.0`; container `0.1.0`; EPF `0.1.0`; ELD `0.1.0`; language release strict `MAJOR.MINOR.PATCH`; canonical profile `elang-canonical-zstd-v1`. | elang-core | T0201 |
| REQ-CORE-VERSION-002 | MUST | Parse strict semver-like `MAJOR.MINOR.PATCH` with no leading zeros. | elang-core | T0202 |
| REQ-CORE-VERSION-003 | MUST | `Header/format.version` first two components match header major/minor; patch MUST NOT change binary grammar. | elang-container | T1204 |
| REQ-CORE-VERSION-004 | MUST | Reader supports binary `0.1` only. | elang-container | T0301, T0302 |

## REQ-CORE-MODEL — Core types, limits, diagnostics, safety (steering §10, §11)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-CORE-MODEL-001 | MUST | Validated resource-path and indexed-resource types. | elang-core | T0203 |
| REQ-CORE-MODEL-002 | MUST | Validated IPA identifier, romanization, group-name and language-identifier types. | elang-core | T0204 |
| REQ-CORE-MODEL-003 | MUST | Immutable validated EPF and ELD AST/model structures. | elang-core | T0205 |
| REQ-CORE-MODEL-004 | MUST | Centralized resource/parser limits with documented units. | elang-core | T0206 |
| REQ-CORE-MODEL-005 | MUST | Checked integer conversion and checked offset arithmetic helpers. | elang-core | T0207 |
| REQ-CORE-MODEL-006 | MUST | Bounded counting reader and decompressed-byte accounting. | elang-core | T0208 |
| REQ-CORE-MODEL-007 | MUST | Distinguish `inspect`, `verify`, `validate` result types; never expose partially validated state as valid. | elang-core | T0212, T1211 |
| REQ-CORE-MODEL-008 | MUST NOT | No panics / unchecked indexing on untrusted data (lint-enforced). | all | T0215, T1517 |

## REQ-LIMIT — Parser budgets & defensive limits (steering §10)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-LIMIT-001 | MUST | KDL nesting depth ≤ 64. | elang-kdl | T0503 |
| REQ-LIMIT-002 | MUST | Encoded token ≤ 1 MiB; decoded string ≤ 1 MiB. | elang-kdl | T0504 |
| REQ-LIMIT-003 | MUST | Node/property name ≤ 256 UTF-8 bytes. | elang-kdl | T0505 |
| REQ-LIMIT-004 | MUST | ≤ 256 args and ≤ 256 properties per node. | elang-kdl | T0506 |
| REQ-LIMIT-005 | MUST | ≤ 2,000,000 nodes/resource; ≤ 10,000,000 nodes/archive. | elang-kdl | T0507 |
| REQ-LIMIT-006 | MUST | ≤ 1 MiB/comment; ≤ 16 MiB comment bytes/resource. | elang-kdl | T0508 |
| REQ-LIMIT-007 | MUST | ≤ 256 MiB decoded string bytes/resource; ≤ 1 GiB/archive. | elang-kdl | T0509 |
| REQ-LIMIT-008 | MUST | ≤ 10,000,000 semantic references/archive; traversal depth ≤ 128. | elang-eld | T0510 |
| REQ-LIMIT-009 | MUST | Retained diagnostics capped at 100 without turning failure into success. | elang-core | T0211 |
| REQ-LIMIT-010 | MUST | Enforce all budgets *during* parsing, before unbounded allocation. | elang-kdl | T0515 |

## REQ-BIN — Binary container header/index/footer/paths/sizes (steering §3)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-BIN-001 | MUST | 32-byte header; magic `45 4C 41 4E 47 0D 0A 1A`. | elang-container | T0301 |
| REQ-BIN-002 | MUST | Header major=0, minor=1, flags=0, 16 reserved bytes zero; reject otherwise. | elang-container | T0302 |
| REQ-BIN-003 | MUST | Deterministic 32-byte header encoder, exact little-endian fields. | elang-container | T0303 |
| REQ-BIN-004 | MUST | 64-byte terminal footer with magic `ELEND001`. | elang-container | T0304 |
| REQ-BIN-005 | MUST | Validate footer reserved bytes; checked index bounds. | elang-container | T0305 |
| REQ-BIN-006 | MUST | Enforce `index_offset + index_size == file_size − 64` (checked). | elang-container | T0306 |
| REQ-BIN-007 | MUST | Index size range `16..=33,554,432`. | elang-container | T0307 |
| REQ-BIN-008 | MUST | Footer SHA-256 equals hash of exact serialized index bytes. | elang-container, elang-integrity | T0308, T0708 |
| REQ-BIN-009 | MUST | 16-byte index header `ELIDX001`; version=1, entry format=1, count ≤ 25,000. | elang-container | T0309, T0310 |
| REQ-BIN-010 | MUST | Index entry = `u16 LE` path len + UTF-8 path + three `u64 LE` + `u8` compression=1. | elang-container | T0311, T0312 |
| REQ-BIN-011 | MUST | Path ≤ 1024 bytes, strict UTF-8, NFC. | elang-container | T0313 |
| REQ-BIN-012 | MUST NOT | Reject absolute paths, empty/`.`/`..` segments, NUL and backslashes. | elang-container | T0314 |
| REQ-BIN-013 | MUST | Strict ascending bytewise path order; no duplicates. | elang-container | T0315 |
| REQ-BIN-014 | MUST | Frames contiguous from byte 32; no gaps/overlaps/unindexed/trailing bytes; last frame ends at index offset. | elang-container | T0316, T0317, T0318 |
| REQ-BIN-015 | MUST | Deterministic index serializer + index-hash computation. | elang-container | T0319 |
| REQ-BIN-016 | MUST | Footer serializer with golden byte comparison. | elang-container | T0320 |
| REQ-BIN-017 | MUST | Index-only archive listing without decompressing resources. | elang-container | T0324 |
| REQ-BIN-018 | MUST NOT | Arbitrary untrusted bytes cannot cause panic or OOB reads. | elang-container | T0321, T0322, T1501 |

## REQ-FRAME — Zstandard frame reader & canonical encoder (steering §3.6, §3.7)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-FRAME-001 | MUST | Pin libzstd 1.5.7 via supported `zstd-sys` build strategy. | elang-container | T0401 |
| REQ-FRAME-002 | MUST | Exactly one independent standard Zstd frame per resource. | elang-container | T0402, T0406 |
| REQ-FRAME-003 | MUST | Require declared content size and frame checksum for every resource. | elang-container | T0403 |
| REQ-FRAME-004 | MUST NOT | Reject window > 128 MiB. | elang-container | T0404 |
| REQ-FRAME-005 | MUST NOT | Reject dictionary dependencies and external/shared dictionaries. | elang-container | T0405 |
| REQ-FRAME-006 | MUST NOT | Reject skippable, concatenated, truncated and trailing-byte frames. | elang-container | T0406, T0407 |
| REQ-FRAME-007 | MUST | Verify frame content size equals index `uncompressed_size`. | elang-container | T0408 |
| REQ-FRAME-008 | MUST | Stream decompress with per-resource limits and aggregate 8 GiB budget; verify checksum and exact byte count. | elang-container | T0409, T0410, T0411 |
| REQ-FRAME-009 | MUST | `Fast`=level 3, `Compact`=level 9 encoders. | elang-container | T0412, T0413 |
| REQ-FRAME-010 | MUST | Canonical encoder: level 9, fresh CCtx/resource, `nbWorkers=0`, `checksumFlag=1`, `contentSizeFlag=1`, `dictIDFlag=0`, `windowLog=27`, LDM off, no dictionary, single `ZSTD_compress2()`. | elang-container | T0414–T0418 |
| REQ-FRAME-011 | MUST | Canonical frames byte-identical across repeated runs/platforms. | elang-container | T0419 |
| REQ-FRAME-012 | MUST | Compressed archive ≤ 6 GiB; recommended ≤ 2 GiB warning. | elang-container | T0420 |

## REQ-READER — Reader modes & 12 validation stages (steering §4)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-READER-001 | MUST | Implement exactly the 12 logical stages, fail-closed, typed errors. | elang-container | T1201–T1203 |
| REQ-READER-002 | MUST | `inspect` = stages 1–7; `verify` = 1–11; `validate` = 1–12. | elang-container | T1201–T1203 |
| REQ-READER-003 | MUST NOT | `inspect` MUST NOT be presented as full integrity verification. | elang-container | T1211, SDK docs |
| REQ-READER-004 | MUST | Verify EPF-declared phoneme resource exists in index and ends `.eld`. | elang-epf, elang-container | T1205 |
| REQ-READER-005 | MUST | Failures report stage, resource and stable error code. | elang-core | T0209, T0210, T1207 |
| REQ-READER-006 | MUST | Bounded random-access resource retrieval by index. | elang-container | T1208 |

## REQ-PKG — Package logical resources (steering §2)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-PKG-001 | MUST | Contain `Header/format.version`, `mimetype`, `package.epf`, `META-INF/integrity.kdl`, one `.eld`. | elang-container | T1204, T1205 |
| REQ-PKG-002 | MUST | `Header/format.version` = UTF-8 `0.1.0`, optional single trailing LF, no BOM, ≤ 64 bytes. | elang-container | T1204 |
| REQ-PKG-003 | MUST | `mimetype` = exact ASCII `application/vnd.elang`, no LF/BOM, ≤ 64 bytes. | elang-container | T1204 |
| REQ-PKG-004 | MUST | Resource-size caps: EPF ≤ 1 MiB, integrity ≤ 16 MiB, `.eld` ≤ 1 GiB. | elang-container | T0411 |
| REQ-PKG-005 | MAY | Extra indexed resources only if a finalized resource/manifest policy allows; no invented module semantics. | elang-epf | T0121, T0615 |

## REQ-EPF — `package.epf` schema (steering §5)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-EPF-001 | MUST | Exactly five top-level nodes: `epf`, `eld`, `language-version`, `metadata`, `manifest`, each once. | elang-epf | T0601 |
| REQ-EPF-002 | MUST NOT | Reject unknown/duplicate top-level nodes and properties. | elang-epf | T0602 |
| REQ-EPF-003 | MUST | `epf`/`eld`/`language-version`: exactly one string arg, no block. | elang-epf | T0603 |
| REQ-EPF-004 | MUST | `epf` and `eld` values exactly `0.1.0`. | elang-epf | T0604 |
| REQ-EPF-005 | MUST | `metadata`/`manifest`: zero args/properties, mandatory blocks. | elang-epf | T0605 |
| REQ-EPF-006 | MUST | `metadata.identifier` once, nonempty, ≤ 512 bytes; URN `urn:elang:language:<ns>:<name>:<version>`. | elang-epf | T0606, T0607 |
| REQ-EPF-007 | MUST | URN namespace/name lowercase ASCII slugs, hyphens, alphanumeric ends; strict version. | elang-epf | T0608, T0609 |
| REQ-EPF-008 | MUST | `language-version` equals URN version suffix exactly. | elang-epf | T0609 |
| REQ-EPF-009 | MUST | `metadata.name` once, nonempty, ≤ 256 bytes. | elang-epf | T0610 |
| REQ-EPF-010 | MAY | `metadata.description` ≤ 1, ≤ 4096 bytes, may be empty. | elang-epf | T0611 |
| REQ-EPF-011 | MUST NOT | Reject duplicate metadata fields, unknown fields, extra args, nested blocks. | elang-epf | T0612 |
| REQ-EPF-012 | MUST | Exactly one manifest `phonemes` entry, one nonempty `.eld` path, validated + index-present. | elang-epf | T0613, T0614 |
| REQ-EPF-013 | MUST NOT | Reject `modules` and any other manifest entry. | elang-epf | T0615 |
| REQ-EPF-014 | MUST | Canonical top-level EPF order and ratified string/whitespace serialization. | elang-epf | T0616, T0617 |

## REQ-INTEGRITY — Integrity manifest & verification (steering §2, §4)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-INTEGRITY-001 | MUST | (BLOCKER) Implement only after `META-INF/integrity.kdl` schema is frozen. | elang-integrity | T0701 |
| REQ-INTEGRITY-002 | MUST | Parse manifest with KDL 2.0 under 16 MiB cap; validate entries/SHA-256 encoding; reject unknown/duplicate fields. | elang-integrity | T0702, T0703 |
| REQ-INTEGRITY-003 | MUST | Cover every indexed resource **except** `integrity.kdl` itself. | elang-integrity | T0704 |
| REQ-INTEGRITY-004 | MUST NOT | Reject entries for missing resources and missing hashes for indexed resources. | elang-integrity | T0705 |
| REQ-INTEGRITY-005 | MUST | Stream SHA-256 over exact uncompressed bytes; compare (constant-time where appropriate); report per-resource failures. | elang-integrity | T0706, T0707 |
| REQ-INTEGRITY-006 | MUST | Verify footer index hash separately from resource integrity hashes. | elang-integrity | T0708 |
| REQ-INTEGRITY-007 | MUST | Canonical integrity manifest writer from the finalized grammar. | elang-integrity | T0709 |
| REQ-INTEGRITY-008 | MUST | Document that SHA-256 integrity is NOT a publisher signature. | docs | T0711, T1618 |

## REQ-UNICODE — Unicode 18.0.0 handling (steering §7)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-UNICODE-001 | MUST | Pin Unicode 18.0.0 data; not runtime-dependent tables. | elang-repertoire | T0801 |
| REQ-UNICODE-002 | MUST | Strict UTF-8 decode; reject invalid scalar sequences/surrogates. | elang-repertoire | T0802 |
| REQ-UNICODE-003 | MUST NOT | Reject non-NFC ELD strings without rewriting input. | elang-repertoire | T0803 |
| REQ-UNICODE-004 | MUST NOT | Reject controls (`Cc`), `Cf`, `Co`, `Cn` and all noncharacters (incl. U+FDD0..FDEF and plane-end FFFE/FFFF) in every ELD string. | elang-repertoire | T0804, T0805 |

## REQ-IPA — IPA repertoire & identifier parsing (steering §7, §7.1)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-IPA-001 | MUST | (BLOCKER) Exact approved IPA allowlist (base/combining/spacing) from pinned 2026 chart. | elang-repertoire | T0106–T0109 |
| REQ-IPA-002 | MUST | Strict UTF-8 JSON repertoire; reject BOM, duplicate keys, unknown fields; metadata = repertoire v1, Unicode 18.0.0, IPA chart 2026. | elang-repertoire | T0111, T0112, T0806, T0807 |
| REQ-IPA-003 | MUST | Inventory uniqueness, classification, sorted order. | elang-repertoire | T0808 |
| REQ-IPA-004 | MUST | (BLOCKER) Canonicalize JSON via RFC 8785 JCS; compare pinned SHA-256. | elang-repertoire | T0115, T0809 |
| REQ-IPA-005 | MUST NOT | Reject any base/diacritic/modifier not in the approved allowlist. | elang-repertoire | T0810 |
| REQ-IPA-006 | MUST | Identifier 1–8 Unicode scalars AND ≤ 4 logical base units (both enforced). | elang-eld | T0811, T0812 |
| REQ-IPA-007 | MUST | ≤ 1 modification per logical unit (one combining OR one spacing modifier, never both). | elang-eld | T0813 |
| REQ-IPA-008 | MUST | Interpret NFC precomposed symbols by canonical decomposition for counting. | elang-eld | T0814 |
| REQ-IPA-009 | MUST NOT | Modifiers/combining marks must attach to a valid base; reject standalone/leading marks. | elang-eld | T0815 |
| REQ-IPA-010 | MUST | Tie bars U+0361/U+035C join exactly two base symbols into one unit; no overlap/chaining. | elang-eld | T0816, T0817 |
| REQ-IPA-011 | MUST | Allow mixed tied/untied; tied-unit modifier only after second base. | elang-eld | T0818, T0819 |
| REQ-IPA-012 | MUST | Uniqueness key treats upper/lower tie bar as equivalent; tied≠untied are distinct identities. | elang-eld | T0820, T0822 |
| REQ-IPA-013 | MUST | Preserve authored tie-bar variant and NFC bytes in canonical output. | elang-eld | T0821 |
| REQ-IPA-014 | MUST NOT | Reject standalone stress, syllable-boundary and tone notation identifiers. | elang-eld | T0823 |

## REQ-ELD — ELD structural schema (steering §6)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-ELD-001 | MUST | Exactly one top-level `phonemes` node with explicit nonempty block, zero args/props. | elang-eld | T0901 |
| REQ-ELD-002 | MUST NOT | Reject empty/whitespace/comment-only ELD and extra top-level nodes. | elang-eld | T0902 |
| REQ-ELD-003 | MUST | Exactly one nonempty `consonants` and one `vowels` group. | elang-eld | T0903 |
| REQ-ELD-004 | MUST | ≤ 16 total groups, ≤ 14 custom; custom name regex `[a-z]+(?:-[a-z]+)*`, 1–32 ASCII. | elang-eld | T0904, T0905 |
| REQ-ELD-005 | MUST NOT | Reject duplicate/reserved group names. | elang-eld | T0906 |
| REQ-ELD-006 | MUST | Groups: explicit nonempty blocks, zero args/props, ≥ 1 root phoneme. | elang-eld | T0907, T0908 |
| REQ-ELD-007 | MUST | `phoneme`: one IPA string arg, zero props, explicit nonempty block; ≤ 256 roots globally. | elang-eld | T0909, T0910 |
| REQ-ELD-008 | MUST | Root: exactly one `roman`, optional ≤ 1 `when`, ≤ 16 direct allophones. | elang-eld | T0911, T0912 |
| REQ-ELD-009 | MUST | `allophone`: one IPA string arg, zero props, only direct child of root. | elang-eld | T0913 |
| REQ-ELD-010 | MUST | Bare allophone allowed when fieldless; explicit `{}` invalid; nonempty block when fields exist. | elang-eld | T0914, T0915, T0916 |
| REQ-ELD-011 | MUST | Allophone: ≤ 1 explicit `roman`, ≤ 1 `when`; reject nested allophones/unknown children. | elang-eld | T0917, T0918 |
| REQ-ELD-012 | MUST | Explicit `roman` matches `[A-Za-z]{1,8}`; scalar leaf (one string arg, no props/block). | elang-eld | T0919, T0925 |
| REQ-ELD-013 | MUST | Explicit romanizations globally unique, ASCII case-insensitive; inherited exempt. | elang-eld | T0920, T0922 |
| REQ-ELD-014 | MUST | Omitted allophone `roman` inherits parent's exact-case value. | elang-eld | T0921 |
| REQ-ELD-015 | MUST | Global IPA identifier uniqueness across roots and allophones incl. tie-bar aliases. | elang-eld | T0923 |
| REQ-ELD-016 | MUST NOT | Reject `frequency`, `distribution`, `description` and any undefined field anywhere. | elang-eld | T0924 |
| REQ-ELD-017 | MUST | Root/allophone `when` independent, no inheritance; overlapping eligibility valid; no fallback/priority. | elang-eld | T0926, T0927, T0928 |

## REQ-WHEN — `when` condition language (steering §8)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-WHEN-001 | MUST | AST: atomic predicates, AND containers (`when`/`group`), binary `or`. | elang-eld | T1001 |
| REQ-WHEN-002 | MUST | ≤ 1 `when` per root/allophone; explicit nonempty block; zero args/props. | elang-eld | T1002, T1003 |
| REQ-WHEN-003 | MUST | `group` ≥ 2 children; `or` exactly 2 children. | elang-eld | T1004, T1005 |
| REQ-WHEN-004 | MUST NOT | Reject explicit `and`/`not` and unknown predicates. | elang-eld | T1006 |
| REQ-WHEN-005 | MUST | Atomic predicates: exactly one string arg, no block/props. | elang-eld | T1007 |
| REQ-WHEN-006 | MUST | `word-position` ∈ {initial, medial, final}; `syllable-position` ∈ {onset, nucleus, coda}. | elang-eld | T1008, T1009 |
| REQ-WHEN-007 | MUST | `previous-phoneme`/`next-phoneme` use IPA identifier rules. | elang-eld | T1010 |
| REQ-WHEN-008 | MUST | ≤ 16 atomic predicates per `when`; ≤ 8 nested `group`/`or` levels per path. | elang-eld | T1011, T1012 |
| REQ-WHEN-009 | MUST | Resolve references after full inventory collection; forward/self refs allowed; allophone refs resolve to owning root; reject missing refs. | elang-eld | T1013–T1016 |
| REQ-WHEN-010 | MUST | AND for `when`/`group` siblings; OR for `or` incl. nested/cross-category. | elang-eld | T1017, T1018 |
| REQ-WHEN-011 | MUST | Permit repeated/contradictory predicates; no simplification. | elang-eld | T1019 |
| REQ-WHEN-012 | MUST | Word position over underlying root sequence; single-phoneme word is initial+final, never medial. | elang-eld | T1020, T1021 |
| REQ-WHEN-013 | MUST | Syllable position from generator-provided assignment; all phonemes in a component match it. | elang-eld | T1022, T1023 |
| REQ-WHEN-014 | MUST | Neighbor matching across syllable but not word boundary; missing neighbor ⇒ false. | elang-eld | T1024, T1025 |
| REQ-WHEN-015 | MUST | Evaluator returns eligibility set (no probability/weight/priority); empty set when none eligible, no fallback. | elang-eld | T1026, T1027, T1028 |

## REQ-CANON — Canonical ELD serialization (steering §9)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-CANON-001 | MUST | Exactly one `phonemes` root; groups ordered `consonants`, `vowels`, then ASCII-lexicographic custom. | elang-eld | T1101, T1102 |
| REQ-CANON-002 | MUST | Roots and allophones sorted by NFC IPA UTF-8 bytewise. | elang-eld | T1103, T1104 |
| REQ-CANON-003 | MUST | Root child order `roman`, `when`, allophones; allophone child order `roman`, `when`. | elang-eld | T1105, T1106 |
| REQ-CANON-004 | MUST | Fieldless allophones emitted as bare leaves, never `{}`. | elang-eld | T1107 |
| REQ-CANON-005 | MUST | Canonical Condition Sorting Key v1 ranks 0..5; atomic/composite key formats exactly as specified. | elang-eld | T1108–T1111 |
| REQ-CANON-006 | MUST | Preserve duplicate predicates, contradictory conditions, tree nesting, authored IPA refs, tie-bar variant, roman case. | elang-eld | T1112, T1113 |
| REQ-CANON-007 | MUST | 4-space indent, one declaration/line, LF only, no blank lines/comments, exactly one final LF. | elang-eld | T1114, T1115 |
| REQ-CANON-008 | MUST | Standard double-quoted strings escaping `"`/`\`; preserve permitted Unicode as UTF-8; no silent normalization. | elang-eld | T1116, T1117 |
| REQ-CANON-009 | MUST | Idempotent: `canon(canon(x)) == canon(x)`; equivalent orders ⇒ identical bytes; different trees stay different. | elang-eld | T1118–T1120 |

## REQ-WRITER — Package writer & CLI (steering §3.7, §4)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-WRITER-001 | MUST | Builder from validated EPF/ELD (+ permitted resources); canonicalize EPF/ELD/integrity per their profiles. | elang-container | T1301, T1302 |
| REQ-WRITER-002 | MUST | Integrity manifest over exact final uncompressed bytes; sort resources by canonical UTF-8 path. | elang-container, elang-integrity | T1303, T1304 |
| REQ-WRITER-003 | MUST | One independent Zstd frame/resource; header + contiguous frames + index + footer with exact offsets and index SHA-256. | elang-container | T1305–T1307 |
| REQ-WRITER-004 | MUST | Enforce all resource/archive ceilings before/during write. | elang-container | T1308 |
| REQ-WRITER-005 | MUST | Support Fast/Compact/Canonical profiles. | elang-container | T1309 |
| REQ-CLI-001 | MUST | `inspect`/`verify`/`validate`/`pack`/`unpack`/`canonicalize-eld`/`repertoire info` commands. | elang-cli | T1310–T1316 |
| REQ-CLI-002 | MUST | Stable human-readable and JSON diagnostics; non-zero exit on any validation failure; documented exit codes. | elang-cli | T1317, T1318 |
| REQ-CLI-003 | MUST NOT | Unpack must prevent overwrite/traversal/symlink escape. | elang-cli | T1319 |

## REQ-ERR — Errors (steering §11)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-ERR-001 | MUST | Error categories and stable codes; `Result`-based propagation. | elang-core | T0209 |
| REQ-ERR-002 | MUST | Attach resource path, byte offset and KDL source span where possible. | elang-core | T0210 |
| REQ-ERR-003 | MUST | Stable machine-readable error-code registry (generated spec artifact). | elang-core | T0119, T1606 |

## REQ-CONF — Conformance & test infrastructure (steering §11)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-CONF-001 | MUST | `elang-canonical-zstd-v1-conformance` fixture manifest + runner. | elang-conformance | T1401 |
| REQ-CONF-002 | MUST | Golden valid/invalid fixtures with exact bytes/hashes for every normative MUST. | elang-conformance | T1402–T1415 |
| REQ-CONF-003 | MUST | Assert expected stage + error code for each fixture under inspect/verify/validate. | elang-conformance | T1416 |
| REQ-CONF-004 | MUST | Canonical write→read→write and shuffled-source determinism. | elang-conformance | T1417, T1418 |
| REQ-CONF-005 | MUST | Run corpus on Linux/Windows/macOS in CI. | ci | T1419, T0011 |
| REQ-SEC-001 | MUST | Fuzz + property + security hardening across all parsers/decoders. | all | T1501–T1520 |

## REQ-GOV — Governance & traceability (steering §0, §12, §13)

| ID | Level | Requirement | Owner | Tasks |
|---|---|---|---|---|
| REQ-GOV-WORKSPACE-001 | MUST | Rust workspace + pinned toolchain; documented MSRV; builds on Linux/Windows/macOS. | repo | T0001 |
| REQ-GOV-WORKSPACE-002 | MUST | Crate decomposition per ADR 0002. | repo | T0002 |
| REQ-GOV-DOCS-001 | MUST | README, LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, issue templates, `docs/specification.md`. | repo | T0003, T0004, T0005 |
| REQ-GOV-ADR-001 | MUST | ADR process with statuses; no locked-rule change without an approved ADR. | repo | T0006, T0015 |
| REQ-GOV-SPEC-001 | MUST | `requirements.md`, `design.md`, `tasks.md`, traceability matrix; every MUST has test coverage. | repo | T0007, T0008, T0009, T0010 |
| REQ-GOV-CI-001 | MUST | CI for fmt, clippy, tests, docs, audit, cross-platform builds. | ci | T0011 |
| REQ-GOV-SUPPLY-001 | MUST | Pin dependency versions; record supply-chain update policy. | repo | T0012 |
| REQ-GOV-FIXTURE-001 | MUST | Fixture naming conventions, expected-result schema, corpus directory. | repo | T0013 |
| REQ-GOV-RELEASE-001 | MUST | Changelog, semver and release-artifact provenance strategy. | repo | T0014 |
| REQ-GOV-OPENART-001 | MUST | Track every open normative artifact with blocking labels; never fabricate normative data. | repo | T0016, T0101–T0123 |
