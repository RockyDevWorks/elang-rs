# Binary container

A `.elang` file is a custom binary container — **not a ZIP**. It stores each
logical resource as exactly one independent Zstandard frame, followed by an index
and a footer. Everything in the file is treated as untrusted input and validated
with checked arithmetic and hard size limits.

## Physical layout

```text
[32-byte header]
[resource Zstd frame 0]
[resource Zstd frame 1]
...
[resource Zstd frame N-1]
[16-byte index header]
[index entries]
[64-byte footer]
```

Resource frames are **contiguous**, start immediately after the 32-byte header,
and run in index order with no gaps, overlaps, unindexed data, or trailing bytes.
The index immediately follows the final frame, and the footer is the only thing
after the index.

## Header — exactly 32 bytes

| Offset | Width | Encoding | Requirement |
|-------:|------:|----------|-------------|
| 0  | 8  | raw     | Magic `45 4C 41 4E 47 0D 0A 1A` (`ELANG\r\n\x1A`) |
| 8  | 2  | u16 LE  | Major version — `0` |
| 10 | 2  | u16 LE  | Minor version — `1` |
| 12 | 4  | u32 LE  | Flags — `0`; unknown/nonzero flags are rejected |
| 16 | 16 | raw     | Reserved — all zero |

The reader supports binary `0.1` only.

## Index

The index header is exactly 16 bytes:

| Field | Width | Encoding | Value |
|-------|------:|----------|-------|
| Magic | 8 | ASCII | `ELIDX001` |
| Index version | 2 | u16 LE | `1` |
| Entry format | 2 | u16 LE | `1` |
| Resource count | 4 | u32 LE | `≤ 25,000` |

Each index entry is **27 fixed bytes plus its path bytes**:

| Field | Width | Encoding |
|-------|------:|----------|
| `path_length` | 2 | u16 LE |
| `path` | variable | UTF-8, exactly `path_length` bytes |
| `frame_offset` | 8 | u64 LE |
| `compressed_size` | 8 | u64 LE |
| `uncompressed_size` | 8 | u64 LE |
| `compression` | 1 | u8 — must equal `1` (Zstd) |

Entries are **strictly sorted by raw UTF-8 path bytes**, with no duplicate paths.
Frames are contiguous in index order.

## Footer — exactly 64 bytes

| Offset | Width | Encoding | Requirement |
|-------:|------:|----------|-------------|
| 0  | 8  | raw    | Magic `ELEND001` |
| 8  | 8  | u64 LE | `index_offset` |
| 16 | 8  | u64 LE | `index_size` (includes the 16-byte index header) |
| 24 | 32 | raw    | SHA-256 of the exact serialized index bytes |
| 56 | 8  | raw    | Reserved — zero |

The footer is the only terminal footer. The reader enforces, using checked
arithmetic, that:

```text
index_offset + index_size == file_size - 64
```

The index size must fall in the range `16 ..= 33,554,432` bytes (16 B to 32 MiB).
Truncation, wrong hashes, overflow, inconsistent counts, and extra trailing bytes
are all rejected.

## Resource paths

Resource paths are strict **NFC UTF-8**, relative, slash-separated, and at most
**1024 UTF-8 bytes**. The following are rejected:

- a leading `/` (absolute paths),
- a backslash `\`, a NUL byte, or an empty segment,
- a `.` or `..` segment (no traversal),
- invalid UTF-8 or non-NFC text,
- drive letters and platform-specific path tricks.

Invalid paths are never silently normalized — they are rejected.

## Size ceilings

| Limit | Maximum |
|-------|--------:|
| Compressed archive | 6 GiB (2 GiB recommended) |
| Aggregate uncompressed resource bytes | 8 GiB |
| Resources per archive | 25,000 |
| Index size | 32 MiB |
| Zstd window | 128 MiB |
| `package.epf` (uncompressed) | 1 MiB |
| `META-INF/integrity.kdl` (uncompressed) | 16 MiB |
| `.eld` phoneme inventory (uncompressed) | 1 GiB |

During decompression the reader bounds output to the declared
`uncompressed_size`, enforces the per-resource maximum and the aggregate budget,
and compares the actual decompressed byte count against the index.

## Zstandard frame contract

Every resource is exactly **one independent, standard Zstd frame** with the frame
**content size present** and the frame **checksum present**. The following are
rejected:

- external or shared dictionaries, or any dictionary-ID dependency,
- concatenated frames, skippable frames, incomplete frames, trailing bytes,
- a window larger than 128 MiB.

The reader compares the declared frame content size against the index
`uncompressed_size`, verifies the checksum, and never trusts frame metadata
without bounded decoding.

## Writer profiles

| Profile | Zstd level | Notes |
|---------|-----------:|-------|
| **Fast** | 3 | Quickest to produce. |
| **Compact** | 9 | Smaller output. |
| **Canonical** | 9 | Reproducible `elang-canonical-zstd-v1` settings. |

The **Canonical** profile pins **libzstd 1.5.7** and encodes each resource with a
fresh compression context and a single `ZSTD_compress2()` call using fixed
settings: level 9, `nbWorkers=0`, `checksumFlag=1`, `contentSizeFlag=1`,
`dictIDFlag=0`, `windowLog=27`, long-distance matching off, and no dictionary.
This makes canonical frames byte-identical across runs and platforms. The
canonical writer may buffer a single resource to perform that one call; the
reader always stays streaming.
