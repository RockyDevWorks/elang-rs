# Supply-chain and dependency policy

Implements REQ-GOV-SUPPLY-001 (steering §13; T0012). Complements
[SECURITY.md](../SECURITY.md).

## Pinning

- **Toolchain:** pinned via [`rust-toolchain.toml`](../rust-toolchain.toml)
  (`channel = "stable"`); MSRV `1.90` declared per crate and enforced in CI.
- **Direct dependencies:** declared once in the root `Cargo.toml`
  `[workspace.dependencies]` using **exact** (`=x.y.z`) versions. Crates opt in
  with `dep.workspace = true`.
- **Transitive dependencies:** locked via a committed `Cargo.lock`. The lock
  file is **not** git-ignored (this repo ships binaries and needs reproducible
  builds).
- **libzstd:** pinned to **1.5.7** for canonical frame reproducibility; see
  Phase 4 tasks.
- **Unicode:** pinned to **18.0.0**, implemented in the first-party
  `elang-unicode` crate from the official UCD 18.0.0 files (hashes in
  `specs/sources/ucd-18.0.0/SOURCES.md`). General_Category and NFC tables are
  generated from those pinned inputs; NFC passes the official 18.0.0
  `NormalizationTest.txt` (20,171 rows). No third-party Unicode data crate and
  no host-OS Unicode behavior. See
  [ADR 0007](decisions/0007-unicode-engine-icu4x.md).

## Update process

1. Open a PR that bumps exactly one dependency (or a coherent group).
2. CI must pass `cargo audit` and `cargo deny` (advisories, licenses, bans,
   duplicate versions).
3. If the bump can change **canonical bytes, schema semantics or a locked rule**
   (notably libzstd, Unicode data, or any serializer dependency), it requires an
   approved ADR and a re-generation of affected golden hashes.
4. Record the change in [`CHANGELOG.md`](../CHANGELOG.md).

## Allowed licenses

Permissive licenses compatible with Apache-2.0 (e.g. MIT, Apache-2.0,
BSD-2/3-Clause, Unicode-DFS). Copyleft dependencies are disallowed without an
ADR. `cargo deny` enforces the allowlist.

## Typosquatting / provenance

New dependencies are reviewed for maintenance status and name plausibility
before being added. Unusual or look-alike crate names are flagged in review.
Release artifacts archive the exact dependency set and pinned data hashes
(T1619).
