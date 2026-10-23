# ADR 0011: Normative artifact manifest and pinned spec checksum (OA-17 mechanism)

- **Status:** Approved
- **Date:** 2026-10-23
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-GOV-OPENART-001, REQ-FREEZE-001 (T0123), REQ-IPA-014
- **Related tasks:** T0123 (resolves the *mechanism* half of OA-17; tag half stays blocked by OA-14 / T1121 / issue #21)

## Context

Steering §12 makes v0.1.0 conformance contingent on a **frozen reviewed
schema/repertoire version with a pinned spec checksum and a release tag**
(OA-17, T0123). OA-14 (canonical ELD goldens, T1121, issue #21) is still
open, so the release-tag half of OA-17 is **not** declared in this PR.
What is in scope here is the *mechanism* for the freeze:

1. a generated normative-artifact manifest, listing every ratified
   normative artifact in the v0.1.0 corpus with its SHA-256;
2. deterministic RFC 8785 JCS serialization of that manifest;
3. a pinned SHA-256 of the canonical manifest bytes;
4. a round-trip/verification test that catches any drift in the corpus,
   the manifest, or the JCS writer;
5. ADR 0011 (this document), recording the mechanism and its
   regeneration rule.

ADR 0003 requires an ADR for every normative freeze; ADR 0008 freezes
the IPA repertoire; this ADR freezes the manifest mechanism that the
later T0123 step will use to declare the final v0.1.0 freeze.

## Decision

1. **Artifact corpus (`ARTIFACTS`).** A static, reviewable list of the
   normative artifact files covered by the freeze, declared in
   `crates/elang-conformance/src/normative_manifest.rs`. The list
   spans every file that actually defines the v0.1.0 format:

   - `specs/schema/*` — IPA repertoire JSON, schema, JCS policy;
   - `specs/grammar/*` — `.md` + `.abnf` for ELD, EPF, integrity;
   - `specs/canonical/*` — canonical EPF profile;
   - `specs/registry/*` — binary vectors, error codes, tie-bar vectors,
     KDL compatibility note, resource policy;
   - `specs/sources/*` — provenance record, pinned UNITIPA 2020 chart
     and IPA Association full-chart page, pinned UCD 18.0.0 files;
   - the OA-11 byte-exact fixture files under
     `fixtures/valid/container/minimal-archive/` (the `.sha256` sidecars
     are derived and excluded).

   Governance and prose outside `specs/` (ADRs, traceability, public
   docs, the manifest itself) are explicitly excluded from the
   freeze. The manifest self-excludes.

2. **Generator.** `cargo run -p elang-conformance --bin
   gen_normative_manifest` reads every artifact from the working tree,
   computes its SHA-256, sorts the entries by path (UTF-16 code unit
   order) and writes the canonical manifest to
   `specs/registry/normative-manifest.json`. Nothing is hand-written:
   the manifest is fully reproducible.

3. **JCS writer.** The manifest is emitted as RFC 8785 JSON Canonical
   Scheme: object keys sorted by UTF-16 code unit order, no
   insignificant whitespace, UTF-8, integers without exponent/decimal,
   quotes and backslashes escaped, control characters below U+0020
   escaped using `\b`, `\t`, `\n`, `\f`, `\r` or lowercase `\uXXXX`,
   and all other characters (including non-BMP) emitted directly as UTF-8,
   per RFC 8785 §3.2.2.2.
   The writer is implemented in
   `elang-conformance::normative_manifest::jcs_canonicalize`.

4. **Pinned checksum.** `PINNED_SPEC_CHECKSUM` holds the SHA-256 of the
   canonical JCS bytes of the committed manifest. The current value
   (`f50bf407…5a8fa`) covers the ratified corpus **excluding OA-14**:
   the IPA/Unicode side of the freeze is ready; the canonical ELD side
   is not.

5. **Round-trip verification.** The integration test
   `crates/elang-conformance/tests/normative_manifest.rs` asserts:

   - regenerating the manifest is byte-identical to the committed
     one (the generator is deterministic);
   - the SHA-256 of the committed manifest equals
     `PINNED_SPEC_CHECKSUM`;
   - every artifact's recorded `sha256` matches the SHA-256 of its
     bytes on disk;
   - the manifest excludes itself and covers `ARTIFACTS` exactly.

6. **Regeneration rule (the OA-14 bridge).** The current
   `PINNED_SPEC_CHECKSUM` is *provisional*. It is **not** a declaration
   that v0.1.0 is frozen. Final T0123 — the freeze-and-tag step —
   must re-run `gen_normative_manifest` after OA-14 lands, so that the
   OA-14 canonical ELD goldens enter `ARTIFACTS` and the manifest
   re-digests; only then is `PINNED_SPEC_CHECKSUM` treated as the
   authoritative v0.1.0 spec checksum, and only then is the release
   tag created.

7. **Release tag is out of scope for this PR.** Creating a `v0.1.0`
   release tag requires OA-14, which is in issue #21 (Phase 11) and
   has not started. The mechanism landed here is the precondition; the
   tag is the consequence.

## Consequences

- The freeze *mechanism* is in place and verified. Any change to a
  covered file, to `ARTIFACTS`, or to the JCS writer is caught by the
  test before the freeze declaration, by design.
- The OA-1/OA-4 sign-off of 2026-10-23 (ADR 0008 §6) is unchanged by
  this ADR: the composition-pair interpretation that landed alongside
  this ADR (see ADR 0008 §7) is a *derived validation rule*, not a
  change to the ratified inventories, so the inventory sign-off stands.
- The release tag is **not** created here. Final T0123 (after OA-14)
  regenerates the manifest, replaces `PINNED_SPEC_CHECKSUM`, declares
  the freeze, and only then creates the `v0.1.0` tag. ADR 0003 still
  applies: a fabricated freeze would void the freeze.
- A future change to any covered file requires (i) updating the
  artifact's bytes, (ii) regenerating the manifest, (iii) updating
  `PINNED_SPEC_CHECKSUM`, and (iv) — if the change is normative — a
  new ADR that supersedes the one that froze the artifact.

## Alternatives considered

- **Single-file checksum of the whole repo.** Rejected — coarse and
  conflates normative artifacts with governance prose, build outputs,
  and the `.git` directory. The corpus-aware manifest is the
  reviewable freeze unit.
- **Hand-written manifest.** Rejected — violates ADR 0003 and the
  pinned-digest precedent in ADR 0008. A hand-written digest is the
  fabrication this repo's gates exist to prevent.
- **Defer the mechanism until OA-14 lands.** Rejected — the
  mechanism is independent of OA-14's content; landing it now means
  final T0123 is a regeneration + tag, not a from-scratch design plus
  a regeneration plus a tag.
