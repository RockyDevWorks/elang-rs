# Versioning

ELANG keeps several **independent** version domains. A change in one does not
imply a change in another. This separation lets the format, its schemas, each
language release, and the tooling evolve at their own pace.

## Version domains

| Domain | Value | Governs |
|--------|-------|---------|
| Specification release | `0.1.0` | The normative spec this code targets. |
| Binary container | `0.1.0` | Header major `0`, minor `1`. |
| EPF (`package.epf`) | `0.1.0` | The descriptor schema. |
| ELD (phoneme inventory) | `0.1.0` | The `.eld` schema. |
| Language release | strict `MAJOR.MINOR.PATCH` | Chosen by each language author. |
| Canonical compression profile | `elang-canonical-zstd-v1` | Reproducible Zstd settings. |

The reader supports binary `0.1` only. `Header/format.version`'s first two
components **must** match the header major/minor, and the **patch** component
cannot change the binary grammar.

The language version string is parsed as strict `MAJOR.MINOR.PATCH` with **no
leading zeros**, and must equal the version suffix of the `metadata.identifier`
URN exactly.

## SDK / crate version

The Rust reference implementation's crate version is **separate** from the format
domains and follows [SemVer](https://semver.org):

- **MAJOR** — a breaking public API change, or a change that alters previously
  valid accept/reject behavior.
- **MINOR** — backward-compatible features (new CLI flags, new read APIs).
- **PATCH** — backward-compatible fixes that do not change canonical bytes or
  accept/reject decisions.

A change in a format domain does not force an SDK major bump unless it breaks the
public Rust API.

## Changes that require extra care

Any change to **canonical bytes**, a **locked rule**, or **normative data**
requires an approved design decision and, where relevant, regenerated golden
hashes — even for an otherwise "patch" fix. Locked rules include the binary
layout, the EPF/ELD grammars, the IPA/Unicode rules, and the canonicalization
rules.

## Pinned dependencies

For reproducibility, several inputs are pinned and do not drift with the host
environment:

- **Unicode** 18.0.0 (first-party tables generated from the official UCD).
- **IPA** 2026 chart issue.
- **libzstd** 1.5.7 for canonical frames.
- The Rust toolchain and the full dependency set, via a committed lock file.
