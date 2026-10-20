# Resource and manifest-coverage policy — v0.1.0

> **Task:** T0121 (open artifact OA-15). **Ratified by:**
> [ADR 0006](../../docs/decisions/0006-resource-and-manifest-policy.md).
> **Normative source:** steering `.kiro/steering/elang.md` §2, §3.5, §12.
> **Requirements:** REQ-PKG-005.

## Allowed resource set (v0.1.0, phonemes-only)

The archive index MUST contain **exactly** these five logical resources and no
others:

| Path (logical) | Role | Max uncompressed |
|---|---|---|
| `Header/format.version` | format version marker, UTF-8 `0.1.0` | 64 bytes |
| `mimetype` | exact ASCII `application/vnd.elang` | 64 bytes |
| `package.epf` | EPF descriptor (KDL 2.0) | 1 MiB |
| `META-INF/integrity.kdl` | SHA-256 integrity manifest | 16 MiB |
| `<manifest.phonemes>` (ends `.eld`) | the one phoneme inventory | 1 GiB |

Rules:

- **No extra indexed resources.** Any additional indexed path ⇒ validation
  failure.
- **No invented modules.** EPF `manifest` has exactly one `phonemes` entry; no
  `modules` node exists in v0.1.0.
- **Integrity coverage** (ADR 0004) covers every indexed resource except
  `integrity.kdl` itself — i.e. the four non-manifest resources above.
- The generic 1 GiB default ceiling for undeclared resources (steering §3.5)
  remains only as a defensive decode-time guard; it is not an acceptance path,
  because undeclared resources are rejected.

A future version adding modules must supersede ADR 0006.
