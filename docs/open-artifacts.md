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
| OA-1 | Exact approved IPA allowlist (base symbols, combining diacritics, spacing modifiers) for the pinned 2026 IPA chart, with classification and canonical-decomposition handling | §7, §12.1 | IPA validator, full ELD validation, conformance | T0106–T0110 | **Generated & verified** — ADR 0008; `specs/schema/ipa-v1.json` (107/27/9) from UNITIPA 2020; base set cross-checked. Freeze pending Unicode 18.0 + 2nd review. |
| OA-2 | Ambiguous-symbol resolutions (standalone base vs modifier vs excluded prosody/tone) | §7, §12.1 | OA-1 finalization | T0109 | **Resolved** — prosody excluded (see `SOURCES.md` §6); script-g `ɡ` U+0261 chosen over ASCII `g`. |
| OA-3 | Strict JSON Schema for the repertoire (version metadata + three inventories) | §7, §12.5 | repertoire loader freeze | T0111 | **Ratified** — [`specs/schema/elang-ipa-repertoire-v1.schema.json`](../specs/schema/elang-ipa-repertoire-v1.schema.json) (shape only; inventories still OA-1/OA-4) |
| OA-4 | Sorted, deduplicated `ipa-v1.json` generated from OA-1 | §7, §12.1 | OA-5 | T0113, T0114 | **Generated** — ADR 0008; `specs/schema/ipa-v1.json`; reproducible via `gen_repertoire`. Independent review of base set automated; 2nd review of diacritics/modifiers pending. |
| OA-5 | Canonical (RFC 8785 JCS) SHA-256 digest of OA-4 | §7, §12.2 | IPA verification constant | T0115 | **Computed & pinned** — `190aae7b…64aa` (`PINNED_JCS_SHA256`), asserted by test; method in [`json-encoding-policy.md`](../specs/schema/json-encoding-policy.md). |
| OA-6 | Complete `META-INF/integrity.kdl` normative grammar + canonical serialization + self-exclusion rule | §2, §12.3 | Phase 7 integrity reader/writer, verify, pack | T0103, T0105 | **Ratified** — ADR 0004, [`specs/grammar/integrity-0.1.0.md`](../specs/grammar/integrity-0.1.0.md) |
| OA-7 | Canonical EPF KDL serializer profile (whitespace, string encoding, field order, comments, newline) | §5, §12.4 | EPF canonical writer | T0104 | **Ratified** — ADR 0005, [`specs/canonical/epf-canonical-serialization.md`](../specs/canonical/epf-canonical-serialization.md) |
| OA-8 | Machine-checkable EPF v0.1.0 grammar (exact KDL 2.0 semantics) | §5, §12.5 | EPF parser freeze | T0101 | **Ratified** — [`specs/grammar/epf-0.1.0.md`](../specs/grammar/epf-0.1.0.md) + ABNF |
| OA-9 | Machine-checkable ELD v0.1.0 grammar (bare allophones, nonempty blocks) | §6, §12.5 | ELD parser freeze | T0102 | **Ratified** — [`specs/grammar/eld-0.1.0.md`](../specs/grammar/eld-0.1.0.md) + ABNF |
| OA-10 | Stable machine-readable error-code registry + metadata schema | §11 | error model freeze, diagnostics | T0119 | **Ratified** — [`specs/registry/error-codes.md`](../specs/registry/error-codes.md) + [`.json`](../specs/registry/error-codes.json) |
| OA-11 | Binary header/index/footer reference vectors and offsets | §3 | container golden fixtures | T0118 | Open |
| OA-12 | Tie-bar equivalence and IPA comparison-key test vectors | §7 | IPA uniqueness tests | T0117 | Open |
| OA-13 | Unicode 18.0 NFC + General Category reference test data | §7 | Unicode validation tests | T0116 | **Ratified** — `elang-unicode` generated from pinned UCD 18.0.0; NFC passes official `NormalizationTest.txt` (20,171 rows); ADR 0007. |
| OA-14 | Canonical ELD golden files + SHA-256 | §9 | canonical writer conformance | T1121 | Open |
| OA-15 | Policy for extra indexed resources / manifest coverage in v0.1.0 (no invented modules) | §2 | REQ-PKG-005 | T0121 | **Ratified** — ADR 0006, [`specs/registry/resource-policy-v0.1.0.md`](../specs/registry/resource-policy-v0.1.0.md) |
| OA-16 | Residual KDL canonicalization edge cases ratified as ADRs | §12.4 | canonical serialization freeze | T0122 | Open (EPF/integrity profiles ratified via ADR 0004/0005; residual ELD edge cases revisited in Phase 11) |
| OA-17 | Frozen reviewed schema/repertoire versions + spec checksum/release tag | §12 | conformance freeze / v0.1.0 | T0123 | Open |

## Rules of engagement

- An artifact is **Ratified** only when it exists, is reproducible, and — where
  it depends on an external pinned source (IPA 2026 chart / Unicode 18.0.0) — is
  independently reviewed against that source and has test vectors. Decision-only
  artifacts (grammars, canonical profiles, policies) are ratified by an ADR
  and/or a published spec file.
- **Ratified so far (2026-10-20):** OA-3 (schema shape), OA-6 (integrity grammar,
  ADR 0004), OA-7 (canonical EPF, ADR 0005), OA-8 (EPF grammar), OA-9 (ELD
  grammar), OA-10 (error registry), OA-15 (resource policy, ADR 0006).
- **IPA inventory (OA-1/OA-2/OA-4/OA-5) is generated, verified and pinned**
  (ADR 0008): `specs/schema/ipa-v1.json` (107 base / 27 combining / 9 spacing)
  from the UNITIPA 2020 chart, with the real JCS digest pinned and the base set
  cross-checked against an independent chart transcription. Unicode 18.0 data
  is now ratified (OA-13). It is **not yet frozen** for release pending a second
  human review of the diacritic/modifier inventories.
- **Still open — blocked on external source data or later phases:** OA-11
  (binary vectors), OA-12 (tie-bar vectors — partially covered by `ipa.rs`
  tests), OA-14 (canonical ELD goldens), OA-16 (residual ELD canonical edge
  cases), OA-17 (freeze/tag). These MUST NOT be fabricated.
- **OA-13 (Unicode data) is ratified:** `elang-unicode` provides Unicode 18.0.0
  General_Category + NFC from the pinned UCD, and NFC passes the official 18.0.0
  conformance suite (ADR 0007). `unicode_data_matches_pin()` is now `true`.
- OA-6 is ratified (ADR 0004); the integrity reader/writer may be implemented in
  Phase 7 against that grammar, but its conformance goldens still depend on the
  IPA/Unicode artifacts for a full package.
- v0.1.0 cannot be declared done while any row above is Open (final release
  checklist).
