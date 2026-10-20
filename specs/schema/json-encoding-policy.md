# Repertoire JSON encoding and canonicalization policy

> **Task:** T0112. **Normative source:** steering `.kiro/steering/elang.md` §7.
> **Requirements:** REQ-IPA-004, REQ-IPA-005.
> Applies to `elang-ipa-repertoire-v1.json`
> ([schema](elang-ipa-repertoire-v1.schema.json)).

## Source-file encoding (as distributed)

1. **Strict UTF-8**, no BOM. A BOM (`EF BB BF`) at the start is rejected.
2. **No duplicate object keys.** A JSON object with a repeated key is rejected
   (not last-wins). Loaders MUST detect duplicates, not silently collapse them.
3. **No unknown fields.** Validated against the schema with
   `additionalProperties: false` at every level.
4. **No duplicate inventory entries.** Each array
   (`base_symbols`, `combining_diacritics`, `spacing_modifiers`) contains
   unique `symbol` values.
5. **Deterministic sort.** Entries in each array are sorted ascending by the
   raw UTF-8 bytes of the `symbol` string. Ties are impossible because symbols
   are unique (rule 4).

## Canonicalization for hashing (RFC 8785 JCS)

The pinned digest (T0115) is the SHA-256 of the **RFC 8785 JSON Canonicalization
Scheme (JCS)** form of the repertoire object:

1. Produce the JCS canonical bytes: UTF-8, object keys sorted by UTF-16 code
   unit, no insignificant whitespace, JCS number/string forms.
2. `digest = SHA-256(jcs_bytes)`, published as lowercase hex.
3. The validator recomputes the JCS bytes of the loaded object and compares the
   digest to the pinned constant; a mismatch rejects the repertoire.

The distributed source file (above) and the JCS form can differ in whitespace
and key order; the **digest is defined over the JCS form only**, so it is stable
regardless of how the source file is formatted.

## Open-artifact note

The digest constant itself (T0115) cannot exist until the reviewed inventory
(T0106–T0110, T0113) produces real bytes. No placeholder/fake digest is stored
anywhere (steering §0.3; ADR 0003). This policy defines *how* the digest is
computed so that step is mechanical once the data is ratified.
