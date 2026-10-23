# Specification status

ELANG is pre-release, tracking **v0.1.0**. This page summarizes what is **locked**
(stable, change only through an approved design decision) and what remains
**open** (unfinished normative work that must be produced and reviewed before the
conformance suite is frozen).

## Locked rules

The following are stable. Changing any of them requires an approved design
decision and, where applicable, regenerated golden hashes:

- The 32-byte header, index-entry layout, and 64-byte footer.
- Resource-path rules and size ceilings.
- The one-frame Zstd contract and the `elang-canonical-zstd-v1` canonical
  profile.
- The EPF five-node grammar, URN format, and field cardinalities.
- The ELD group/root/allophone grammar, cardinalities, and the bare-allophone
  rule.
- The IPA tie-bar, modifier, and length rules, including tie-bar equivalence for
  uniqueness.
- The `when` grammar and semantics.
- The canonical ELD serialization and the Canonical Condition Sorting Key v1.
- Pinned Unicode 18.0.0, the IPA 2026 chart, and libzstd 1.5.7.

## Ratified normative artifacts

These decision-derived artifacts have been produced and reviewed:

- The complete `META-INF/integrity.kdl` grammar and canonical serialization.
- The canonical EPF serializer profile.
- Machine-checkable EPF and ELD grammars, and the repertoire JSON Schema (shape).
- The stable error-code registry.
- The resource/manifest policy for extra indexed resources (no invented modules).
- Unicode 18.0.0 data: General_Category + NFC generated from the pinned UCD, with
  NFC passing the official 18.0.0 conformance suite.
- Binary header/index/footer reference vectors and offsets: the exact §3 layout
  with header, index and footer field tables and byte-exact fixtures
  ([`specs/registry/binary-reference-vectors.json`](../../specs/registry/binary-reference-vectors.json),
  `fixtures/valid/container/minimal-archive/`). All offsets, sizes and SHA-256
  digests are computed by the deterministic `elang-conformance::binary_vectors`
  builder and asserted by tests.
- Tie-bar equivalence and IPA comparison-key test vectors: 12 published vectors
  ([`specs/registry/ipa-tie-bar-vectors.json`](../../specs/registry/ipa-tie-bar-vectors.json))
  covering positive, equivalence and negative cases from §7.1, driven by the
  `elang-repertoire` integration test `tie_bar_vectors.rs`.
- Residual KDL canonicalization edge cases for ELD: ADR 0009 pins the
  authored-byte vs comparison-key views for ordering (E-1), the condition sort
  key (E-2), the exact `"`/`\` escaping set (E-3), bare-vs-block allophone
  emission (E-4/E-5) and layout invariants (E-6).

## IPA inventory — generated, verified, pinned (not yet frozen)

The IPA inventory has been generated, cross-checked, and pinned:

- An allowlist of base symbols, combining diacritics, and spacing modifiers drawn
  from the pinned chart.
- The base set is cross-checked against an independent chart transcription.
- The canonical (RFC 8785 JCS) SHA-256 digest is computed and pinned, and asserted
  by test.

It is **not yet frozen for release**, pending a second human review of the
diacritic and modifier inventories.

## Still open — must not be fabricated

The following remain open and are release blockers. They must be produced and
reviewed rather than guessed:

- Canonical ELD golden files and their SHA-256 digests (OA-14) — produced by
  the Phase 11 canonical writer (T1121).
- The frozen, reviewed schema/repertoire versions plus the spec checksum and
  release tag (OA-17) — depends on OA-14 and the second human review of the
  IPA diacritic/modifier inventories.

!!! danger "No fabricated normative data"
    Until each open item is ratified, no code path embeds a guessed inventory, a
    placeholder hash, or an inferred grammar as if it were normative. This is a
    hard project rule.

## Definition of done for v0.1.0

v0.1.0 is cut only when:

- no open normative artifact remains,
- every normative `MUST` has passing test coverage,
- a `pack → inspect → verify → validate → unpack` round-trip passes,
- canonical bytes and hashes reproduce on all supported platforms,
- malformed inputs fail closed, and fuzz/property tests pass.
