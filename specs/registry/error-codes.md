# ELANG error-code registry and diagnostic metadata schema

> **Task:** T0119 (open artifact OA-10). **Normative source:** steering
> `.kiro/steering/elang.md` §11. **Requirements:** REQ-ERR-001..003.
> Machine-readable companion: [`error-codes.json`](error-codes.json).

Error codes are **stable** machine-readable identifiers. The reference
implementation and the conformance corpus both refer to codes by these exact
strings. Adding a code is a backward-compatible change; renaming or removing a
code requires an ADR.

## Diagnostic metadata schema

Every diagnostic a reader/validator produces carries:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `code` | string | yes | A stable code from the registry (e.g. `FOOTER_INDEX_HASH_MISMATCH`). |
| `category` | string | yes | One of the categories below. |
| `stage` | integer 1–12 | where applicable | Validation stage at which the failure occurred (steering §4). |
| `resourcePath` | string | where applicable | The indexed resource the diagnostic concerns. |
| `byteOffset` | integer ≥ 0 | where applicable | Byte offset into the archive/resource. |
| `kdlSpan` | object `{line, column, length}` | where applicable | KDL source span for schema errors. |
| `message` | string | yes | Human-readable, non-authoritative description. |

Diagnostics are capped at **100** retained entries (steering §10); truncation
MUST NOT turn a failure into a success (REQ-ERR-003).

## Categories (steering §11)

`CONTAINER`, `INDEX`, `FRAME`, `RESOURCE_PATH`, `INTEGRITY`, `EPF_SCHEMA`,
`ELD_SCHEMA`, `UNICODE`, `IPA`, `ROMAN`, `WHEN`, `LIMIT`, `CANONICAL`.

## Codes

Naming: `SCREAMING_SNAKE_CASE`, prefixed by concept. The list below is the
initial registry; it grows as phases land. Each code maps to a category and the
primary stage(s) at which it can fire.

### CONTAINER (stages 1–3, 8)

| Code | Stage | Meaning |
|---|---|---|
| `ARCHIVE_TOO_SMALL` | 1 | File smaller than minimal header+index+footer. |
| `ARCHIVE_SIZE_INCONSISTENT` | 1 | Declared offsets/sizes vs file size inconsistent. |
| `HEADER_BAD_MAGIC` | 2 | First 8 bytes ≠ `ELANG\r\n\x1A`. |
| `HEADER_BAD_VERSION` | 2 | major≠0 or minor≠1. |
| `HEADER_NONZERO_FLAGS` | 2 | flags field nonzero/unknown. |
| `HEADER_RESERVED_NONZERO` | 2 | reserved 16 bytes not all zero. |
| `FOOTER_BAD_MAGIC` | 3 | Footer not `ELEND001`. |
| `FOOTER_RESERVED_NONZERO` | 3 | Footer reserved bytes nonzero. |
| `FOOTER_OFFSET_INCONSISTENT` | 3 | `index_offset + index_size ≠ file_size − 64`. |
| `FORMAT_VERSION_INVALID` | 8 | `Header/format.version` not UTF-8 `0.1.0`. |
| `CONTAINER_BAD_VERSION` | 8 | Binary container version string not strict `MAJOR.MINOR.PATCH`. |
| `MIMETYPE_INVALID` | 8 | `mimetype` ≠ exact `application/vnd.elang`. |

### INDEX (stages 4–6)

| Code | Stage | Meaning |
|---|---|---|
| `FOOTER_INDEX_HASH_MISMATCH` | 4 | SHA-256 of index bytes ≠ footer hash. |
| `INDEX_BAD_MAGIC` | 5 | Index header not `ELIDX001`. |
| `INDEX_BAD_VERSION` | 5 | index version≠1 or entry format≠1. |
| `INDEX_COUNT_OUT_OF_RANGE` | 5 | resource count 0 or >25,000. |
| `INDEX_SIZE_OUT_OF_RANGE` | 5 | index size outside 16..=33,554,432. |
| `INDEX_ENTRY_MALFORMED` | 5 | Truncated/overlong entry, bad compression code. |
| `INDEX_PATH_NOT_SORTED` | 5 | Entries not strictly ascending by UTF-8 bytes. |
| `INDEX_DUPLICATE_PATH` | 5 | Duplicate path. |
| `FRAME_LAYOUT_GAP` | 6 | Gap/hole between frames. |
| `FRAME_LAYOUT_OVERLAP` | 6 | Overlapping frames. |
| `FRAME_LAYOUT_UNINDEXED_BYTES` | 6 | Bytes not covered by any frame. |
| `FRAME_LAYOUT_TRAILING_BYTES` | 6 | Frames do not end exactly at index offset. |
| `INDEX_RANGE_OUT_OF_BOUNDS` | 5 | `offset + compressed_size` exceeds the archive, or overflows `u64`. |
| `INDEX_SIZE_OVER_CEILING` | 5 | A resource's `uncompressed_size` exceeds the 1 GiB per-resource ceiling. |

### RESOURCE_PATH (stage 5)

| Code | Stage | Meaning |
|---|---|---|
| `PATH_NOT_UTF8` | 5 | Path bytes not valid UTF-8. |
| `PATH_NOT_NFC` | 5 | Path not in NFC. |
| `PATH_ABSOLUTE` | 5 | Leading `/` or drive-letter path. |
| `PATH_BACKSLASH` | 5 | Contains `\`. |
| `PATH_NUL` | 5 | Contains NUL. |
| `PATH_EMPTY_SEGMENT` | 5 | Empty, `.`, or `..` segment. |
| `PATH_TOO_LONG` | 5 | > 1024 UTF-8 bytes. |

### FRAME (stage 7, 11)

| Code | Stage | Meaning |
|---|---|---|
| `FRAME_BAD_MAGIC` | 7 | Fewer than four bytes, incorrect frame magic, or an Unused/Reserved descriptor bit is set. |
| `FRAME_MISSING_CONTENT_SIZE` | 7 | Frame content size not present. |
| `FRAME_MISSING_CHECKSUM` | 7 | Frame checksum not present. |
| `FRAME_WINDOW_TOO_LARGE` | 7 | Window > 128 MiB. |
| `FRAME_DICTIONARY_DEPENDENCY` | 7 | Dictionary ID / external dictionary. |
| `FRAME_SKIPPABLE` | 7 | Skippable frame present. |
| `FRAME_CONCATENATED` | 7 | More than one frame per resource. |
| `FRAME_TRUNCATED` | 7/11 | Incomplete/truncated frame. |
| `FRAME_CONTENT_SIZE_MISMATCH` | 11 | Declared size ≠ index `uncompressed_size`. |
| `FRAME_CHECKSUM_MISMATCH` | 11 | Zstd checksum fails. |
| `FRAME_DECOMPRESSED_SIZE_MISMATCH` | 11 | Actual decompressed bytes ≠ index. |
| `FRAME_DECOMPRESS_FAILED` | 11 | libzstd rejected the frame body during streaming decode. |
| `FRAME_ENCODE_FAILED` | — | libzstd rejected an encode request (parameter or compress call). |
| `DECOMPRESSION_BUDGET_EXCEEDED` | 11 | Per-resource or 8 GiB aggregate budget exceeded. |

### INTEGRITY (stages 9, 11)

| Code | Stage | Meaning |
|---|---|---|
| `INTEGRITY_STRUCTURE_INVALID` | 9 | integrity.kdl violates its grammar (ADR 0004). |
| `INTEGRITY_BAD_ALGORITHM` | 9 | `algorithm` ≠ `sha-256`. |
| `INTEGRITY_BAD_HEX` | 9 | `sha256` not 64 lowercase hex. |
| `INTEGRITY_MISSING_ENTRY` | 9 | Indexed resource (≠ self) has no entry. |
| `INTEGRITY_EXTRA_ENTRY` | 9 | Entry for a non-indexed path or for integrity.kdl itself. |
| `INTEGRITY_DUPLICATE_ENTRY` | 9 | Duplicate resource path. |
| `INTEGRITY_HASH_MISMATCH` | 11 | Resource digest ≠ declared digest. |

### EPF_SCHEMA (stage 10)

| Code | Stage | Meaning |
|---|---|---|
| `EPF_UNKNOWN_NODE` | 10 | Unknown top-level/child node. |
| `EPF_DUPLICATE_NODE` | 10 | Duplicate singleton node. |
| `EPF_MISSING_NODE` | 10 | Missing required node. |
| `EPF_BAD_ARITY` | 10 | Wrong arg/property count or block presence. |
| `EPF_BAD_VERSION` | 10 | `epf`/`eld` ≠ `0.1.0`, or bad semver. |
| `EPF_BAD_URN` | 10 | Identifier URN malformed. |
| `EPF_VERSION_MISMATCH` | 10 | `language-version` ≠ URN version suffix. |
| `LANG_BAD_VERSION` | 10 | `language-version` not strict `MAJOR.MINOR.PATCH`. |
| `EPF_BAD_METADATA_FIELD` | 10 | Field length/content violation. |
| `EPF_MANIFEST_INVALID` | 10 | Missing/duplicate `phonemes`, `modules` present, bad path. |
| `EPF_MANIFEST_PATH_NOT_INDEXED` | 10 | `phonemes` path not in index or not `.eld`. |

### ELD_SCHEMA (stage 12)

| Code | Stage | Meaning |
|---|---|---|
| `ELD_EMPTY` | 12 | Empty/whitespace/comment-only file. |
| `ELD_BAD_VERSION` | 12 | `eld` schema version not strict `MAJOR.MINOR.PATCH`. |
| `ELD_TOPLEVEL_INVALID` | 12 | Missing/duplicate/extra top-level node. |
| `ELD_GROUP_INVALID` | 12 | Missing `consonants`/`vowels`, bad custom name, empty group, >16 groups. |
| `ELD_GROUP_DUPLICATE` | 12 | Duplicate group name / reserved reuse. |
| `ELD_PHONEME_INVALID` | 12 | Bad root arity/children, >256 roots. |
| `ELD_ALLOPHONE_INVALID` | 12 | Empty `{}`, nesting, >16 per root, bad arity. |
| `ELD_UNKNOWN_FIELD` | 12 | `frequency`/`distribution`/`description`/unknown child. |
| `ELD_IPA_DUPLICATE` | 12 | Global IPA identifier collision (incl. tie-bar aliases). |

### UNICODE (stage 12)

| Code | Stage | Meaning |
|---|---|---|
| `UNICODE_NOT_NFC` | 12 | ELD string not NFC. |
| `UNICODE_FORBIDDEN_CATEGORY` | 12 | Cc/Cf/Co/Cn character. |
| `UNICODE_NONCHARACTER` | 12 | Unicode noncharacter. |
| `UNICODE_INVALID_SCALAR` | 12 | Surrogate / invalid scalar / invalid UTF-8. |

### IPA (stage 12)

| Code | Stage | Meaning |
|---|---|---|
| `IPA_NOT_IN_REPERTOIRE` | 12 | Symbol not in approved allowlist. |
| `IPA_TOO_MANY_SCALARS` | 12 | > 8 Unicode scalars. |
| `IPA_TOO_MANY_UNITS` | 12 | > 4 logical base units. |
| `IPA_TOO_MANY_MODIFICATIONS` | 12 | > 1 modification on a unit. |
| `IPA_STANDALONE_MODIFIER` | 12 | Modifier/combining mark without a base. |
| `IPA_TIE_BAR_INVALID` | 12 | Overlapping/chained tie, wrong operand count, bad modifier placement. |
| `IPA_EXCLUDED_NOTATION` | 12 | Stress/syllable-boundary/tone notation used as identifier. |

### ROMAN (stage 12)

| Code | Stage | Meaning |
|---|---|---|
| `ROMAN_BAD_FORMAT` | 12 | Not `[A-Za-z]{1,8}`. |
| `ROMAN_DUPLICATE` | 12 | Case-insensitive collision among explicit romanizations. |
| `ROMAN_INVALID_NODE` | 12 | Missing/extra arg, property, or block on `roman`. |

### WHEN (stage 12)

| Code | Stage | Meaning |
|---|---|---|
| `WHEN_EMPTY` | 12 | `when {}` with no child expression. |
| `WHEN_BAD_ARITY` | 12 | `group` <2, `or` ≠2, args/props on structural nodes. |
| `WHEN_UNKNOWN_NODE` | 12 | `and`/`not`/unknown predicate. |
| `WHEN_BAD_VALUE` | 12 | Bad `word-position`/`syllable-position` value. |
| `WHEN_PREDICATE_ARITY` | 12 | Atomic predicate wrong arg count or has a block. |
| `WHEN_REFERENCE_UNRESOLVED` | 12 | `previous`/`next-phoneme` references undeclared id. |
| `WHEN_TOO_MANY_PREDICATES` | 12 | > 16 atomic predicates per `when`. |
| `WHEN_TOO_DEEP` | 12 | > 8 nested group/or levels. |

### LIMIT (any parse stage)

| Code | Meaning |
|---|---|
| `LIMIT_NESTING_DEPTH` | KDL nesting > 64. |
| `LIMIT_TOKEN_BYTES` | Encoded token > 1 MiB. |
| `LIMIT_STRING_BYTES` | Decoded string > 1 MiB (or per-resource/archive cap). |
| `LIMIT_NAME_BYTES` | Node/property name > 256 bytes. |
| `LIMIT_ARGS` | > 256 args per node. |
| `LIMIT_PROPS` | > 256 properties per node. |
| `LIMIT_NODES_RESOURCE` | > 2,000,000 nodes/resource. |
| `LIMIT_NODES_ARCHIVE` | > 10,000,000 nodes/archive. |
| `LIMIT_COMMENT_BYTES` | Comment byte caps exceeded. |
| `LIMIT_REFERENCES` | > 10,000,000 semantic references/archive. |
| `LIMIT_TRAVERSAL_DEPTH` | Semantic traversal depth > 128. |
| `LIMIT_RESOURCES_ARCHIVE` | > 25,000 resources in one archive. |
| `LIMIT_UNCOMPRESSED_AGGREGATE` | Aggregate declared uncompressed bytes > 8 GiB. |

### CANONICAL (writer/roundtrip)

| Code | Meaning |
|---|---|
| `CANONICAL_NOT_IDEMPOTENT` | `canon(canon(x)) ≠ canon(x)` (internal invariant failure). |
| `CANONICAL_MISMATCH` | Emitted canonical bytes ≠ expected golden. |
| `CANONICAL_PROFILE_UNKNOWN` | Canonical-profile identifier is not the locked `elang-canonical-zstd-v1` (or whatever profile the steering committee has ratified). |

### KDL PARSER (Phase 5, stage 10)

KDL 2.0 parser diagnostic codes (see ADR 0010, steering §10). The parser
is hand-rolled and reports every structural or budget violation through
this stable set so the schema layer can route them through the unified
error model.

| Code | Category | Meaning |
|---|---|---|
| `KDL_INVALID_UTF8` | CONTAINER | Input bytes fail UTF-8 validation before parsing. |
| `KDL_TOKEN_TOO_LONG` | LIMIT | A single token (identifier, string, number, comment) exceeded `MAX_ENCODED_TOKEN_BYTES` (1 MiB). |
| `KDL_STRING_TOO_LONG` | LIMIT | Decoded string exceeded `MAX_DECODED_STRING_BYTES` (1 MiB). |
| `KDL_NAME_TOO_LONG` | LIMIT | Node or property name exceeded `MAX_NAME_BYTES` (256 B). |
| `KDL_TOO_MANY_ARGS` | LIMIT | Node has more positional arguments than `MAX_ARGS_PER_NODE`. |
| `KDL_TOO_MANY_PROPS` | LIMIT | Node has more properties than `MAX_PROPS_PER_NODE`. |
| `KDL_NESTING_TOO_DEEP` | LIMIT | `children` block nesting exceeded `MAX_KDL_NESTING_DEPTH` (64). |
| `KDL_TOO_MANY_NODES_RESOURCE` | LIMIT | Per-resource node count exceeded `MAX_NODES_PER_RESOURCE`. |
| `KDL_TOO_MANY_NODES_ARCHIVE` | LIMIT | Archive-wide node count exceeded `MAX_NODES_PER_ARCHIVE` (10 M). |
| `KDL_COMMENT_TOO_LONG` | LIMIT | A single comment exceeded `MAX_COMMENT_BYTES` (1 MiB). |
| `KDL_COMMENT_BUDGET_EXCEEDED` | LIMIT | Cumulative comment bytes per resource exceeded `MAX_COMMENT_BYTES_PER_RESOURCE` (16 MiB). |
| `KDL_STRING_BUDGET_RESOURCE_EXCEEDED` | LIMIT | Cumulative decoded string bytes per resource exceeded `MAX_DECODED_STRING_BYTES_PER_RESOURCE` (256 MiB). |
| `KDL_STRING_BUDGET_ARCHIVE_EXCEEDED` | LIMIT | Archive-wide decoded string bytes exceeded `MAX_DECODED_STRING_BYTES_PER_ARCHIVE` (1 GiB). |
| `KDL_UNTERMINATED_STRING` | LIMIT | String literal missing closing `"`. |
| `KDL_UNTERMINATED_COMMENT` | LIMIT | Block comment missing `*/`. |
| `KDL_UNTERMINATED_RAW_STRING` | LIMIT | Raw string `r#…#` missing matching closing fence. |
| `KDL_INVALID_ESCAPE` | LIMIT | Unrecognised escape sequence in a string literal. |
| `KDL_INVALID_UNICODE_ESCAPE` | LIMIT | Malformed `\u{…}` escape (missing `}`, non-hex digit, surrogate, out-of-range). |
| `KDL_INVALID_NUMBER` | LIMIT | Numeric literal malformed (no digits after base prefix, missing exponent, etc.). |
| `KDL_UNEXPECTED_BYTE` | LIMIT | Byte outside the KDL grammar where a token was expected. |
| `KDL_UNEXPECTED_EOF` | LIMIT | End-of-input reached where a token was expected. |
| `KDL_INVALID_IDENTIFIER` | LIMIT | Bare identifier contains an invalid character (e.g. reserved punctuation). |
| `KDL_MISSING_VERSION_PRAGMA` | LIMIT | Reserved for consumers requiring the optional `/kdl-version "2.0"` pragma; parsing accepts its absence and does not emit this code. |
| `KDL_UNSUPPORTED_VERSION` | LIMIT | `/kdl-version` value is not the pinned `"2.0"`. |
| `KDL_OTHER` | LIMIT | Catch-all for unmodelled failures. |

## Stability note

This registry is the single source of truth for codes. Crates MUST NOT invent
ad-hoc code strings outside it (steering §11). The JSON companion is the form
consumed by the conformance runner and by fixtures' `errorCode` field.
