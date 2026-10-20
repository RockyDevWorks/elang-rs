# Open normative artifacts (release blockers)

> Governed by [ADR 0003](decisions/0003-open-normative-artifacts.md) and
> REQ-GOV-OPENART-001. These artifacts are **not yet ratified**. Code MUST NOT
> embed guessed inventories, placeholder/fake hashes or inferred grammars as if
> they were normative (steering §0.3, §12).
>
> Each row should become a `blocker`-labelled issue using the
> [normative-artifact issue template](../.github/ISSUE_TEMPLATE/normative_artifact.md).

| # | Artifact | Steering | Blocks | Tasks | Status |
|---|---|---|---|---|---|
| OA-1 | Exact approved IPA allowlist (base symbols, combining diacritics, spacing modifiers) for the pinned 2026 IPA chart, with classification and canonical-decomposition handling | §7, §12.1 | IPA validator, full ELD validation, conformance | T0106–T0110 | Open |
| OA-2 | Ambiguous-symbol resolutions (standalone base vs modifier vs excluded prosody/tone) | §7, §12.1 | OA-1 finalization | T0109 | Open |
| OA-3 | Strict JSON Schema for the repertoire (version metadata + three inventories) | §7, §12.5 | repertoire loader freeze | T0111 | Open |
| OA-4 | Sorted, deduplicated `elang-ipa-repertoire-v1.json` generated from OA-1 | §7, §12.1 | OA-5 | T0113, T0114 | Open |
| OA-5 | Canonical (RFC 8785 JCS) SHA-256 digest of OA-4 | §7, §12.2 | IPA verification constant | T0115 | Open |
| OA-6 | Complete `META-INF/integrity.kdl` normative grammar + canonical serialization + self-exclusion rule | §2, §12.3 | Phase 7 integrity reader/writer, verify, pack | T0103, T0105 | Open |
| OA-7 | Canonical EPF KDL serializer profile (whitespace, string encoding, field order, comments, newline) | §5, §12.4 | EPF canonical writer | T0104 | Open |
| OA-8 | Machine-checkable EPF v0.1.0 grammar (exact KDL 2.0 semantics) | §5, §12.5 | EPF parser freeze | T0101 | Open |
| OA-9 | Machine-checkable ELD v0.1.0 grammar (bare allophones, nonempty blocks) | §6, §12.5 | ELD parser freeze | T0102 | Open |
| OA-10 | Stable machine-readable error-code registry + metadata schema | §11 | error model freeze, diagnostics | T0119 | Open |
| OA-11 | Binary header/index/footer reference vectors and offsets | §3 | container golden fixtures | T0118 | Open |
| OA-12 | Tie-bar equivalence and IPA comparison-key test vectors | §7 | IPA uniqueness tests | T0117 | Open |
| OA-13 | Unicode 18.0 NFC + General Category reference test data | §7 | Unicode validation tests | T0116 | Open |
| OA-14 | Canonical ELD golden files + SHA-256 | §9 | canonical writer conformance | T1121 | Open |
| OA-15 | Policy for extra indexed resources / manifest coverage in v0.1.0 (no invented modules) | §2 | REQ-PKG-005 | T0121 | Open |
| OA-16 | Residual KDL canonicalization edge cases ratified as ADRs | §12.4 | canonical serialization freeze | T0122 | Open |
| OA-17 | Frozen reviewed schema/repertoire versions + spec checksum/release tag | §12 | conformance freeze / v0.1.0 | T0123 | Open |

## Rules of engagement

- An artifact is **Ratified** only when it exists, is independently reviewed
  against its pinned source (IPA 2026 chart / Unicode 18.0.0), is reproducible,
  has test vectors, and is recorded by an ADR.
- Until OA-1/OA-4/OA-5 are ratified, the IPA validator ships no inventory.
- Until OA-6 is ratified, the integrity reader/writer is not implemented
  (REQ-INTEGRITY-001 BLOCKER).
- v0.1.0 cannot be declared done while any row above is Open (final release
  checklist).
