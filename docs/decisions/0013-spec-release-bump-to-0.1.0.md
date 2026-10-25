# ADR 0013: Bump specification release to `0.1.0` for the v0.1.0 first release

- **Status:** Approved
- **Date:** 2026-10-25
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-CORE-VERSION-001, REQ-GOV-ADR-001
- **Related tasks:** T0201, T0123, T1617, T1620

## Context

The ELANG steering contract pins **seven independent version domains**
(REQ-CORE-VERSION-001). The first six — binary container `0.1.0`,
EPF schema `0.1.0`, ELD schema `0.1.0`, language release, canonical
compression profile `elang-canonical-zstd-v1`, and the workspace crate
versions — are already aligned on `0.1.0`. The seventh, the **specification
release**, has remained at the draft counter `0.0.1-1` since steering §1 was
first written. That counter was set when the v0.1.0 spec was still a working
draft; the dash-suffix signalled "first revision of the 0.0.1 line".

That signal is no longer accurate. The full normative corpus for v0.1.0 is
now ratified (ADR 0004–0012), the OA-14 canonical ELD goldens are in place
(T1121), the normative manifest mechanism has landed (ADR 0011, T0123), and
the workspace is on the merged PR #71 release-packaging branch. The
specification release identifier should therefore match the release it ships
in: `0.1.0`. The independent-domain rule in REQ-CORE-VERSION-001 explicitly
permits this — the spec release is **not** tied 1:1 to the binary container
or to any of the format-owned identifiers.

This is a locked normative change. ADR 0001 requires an `Approved` ADR for
every change to a locked normative rule, and REQ-CORE-VERSION-001 is a
`MUST` requirement. So the rename is gated on this ADR before the source
flip and is committed together with it.

## Decision

1. **Bump the specification release identifier from `0.0.1-1` to `0.1.0`**
   everywhere it is recorded as a domain identifier. The dash-suffix is
   dropped; the new value follows the same `MAJOR.MINOR.PATCH` shape used by
   the other version domains and accepted by `SemVer::parse`. The identifier
   remains a `&'static str` constant in `elang-core::version_domains` and
   `elang-core::version::domains`, and the existing
   `version_domains_are_locked` test continues to assert it.
2. **Update REQ-CORE-VERSION-001 in place** to record the new identifier in
   its requirement text. The requirement's normative level remains
   `MUST`; the requirement body changes only to substitute the spec-release
   literal. No new requirement is added and no existing requirement is
   renumbered (REQ-GOV-OPENART-001).
3. **Update the normative sources** that pin the identifier:
   `.kiro/steering/elang.md` (header callout, §1 domain statement, and the
   closing reference line), `.kiro/specs/elang-v0.1.0/tasks.md` baseline
   note, `specs/registry/error-codes.json` `spec_release` field, and the
   three public/docs surfaces (`README.md`, `docs/specification.md`,
   `docs/public/versioning.md`).
4. **Regenerate the normative manifest and refresh the pinned checksum**
   (`PINNED_SPEC_CHECKSUM` in `crates/elang-conformance/src/normative_manifest.rs`)
   because `specs/registry/error-codes.json` is part of the `ARTIFACTS` list
   (ADR 0011 §Decision 1) and its bytes change with the field rename. The
   `gen_normative_manifest` binary emits the new manifest and prints the new
   digest; the digest is then committed in place. The corpus drift test in
   `crates/elang-conformance/tests/normative_manifest.rs` is the verification
   gate.
5. **The other six version domains are not touched.** Binary container
   `0.1.0`, EPF `0.1.0`, ELD `0.1.0`, language release, canonical compression
   profile `elang-canonical-zstd-v1`, and workspace crate versions
   (`0.1.0`) all remain unchanged. The independence rule is preserved.

## Consequences

- Every place that cited the spec release now cites `0.1.0`; the README and
  the public versioning page are self-consistent with the steering contract
  and with the normative manifest.
- `error-codes.json`'s declared `spec_release` matches the constant in
  `elang-core`; consumers of the registry that read `spec_release` see the
  same value the implementation targets.
- `PINNED_SPEC_CHECKSUM` is updated in the same change; the corpus drift test
  asserts the new checksum matches the regenerated manifest bytes.
- The dash-suffix is gone. Any external document that parsed the spec
  release as a semver-with-prerelease (`0.0.1-1`) will need to accept the
  clean `0.1.0` form. No external document is known to do so today; the only
  known parsers are the project's own `SemVer::parse` (which accepts both)
  and the registry JSON reader (which treats the value as an opaque
  identifier).
- The CHANGELOG entry under `[Unreleased]` records the bump and cites this
  ADR so reviewers can trace the rationale.

## Alternatives considered

- **Keep `0.0.1-1` and bump it to `0.1.0-0` / `0.1.0-1` to keep signalling
  "draft of the 0.1.0 release".** Rejected: the v0.1.0 spec is no longer a
  draft — every OA tracked in `docs/open-artifacts.md` for the spec corpus is
  either Ratified or has its mechanism landed (OA-17). Continuing to carry a
  dash-suffix would misrepresent the status to readers and require a
  follow-up bump at release time, doubling the governance overhead.
- **Bump everything together (binary container, EPF, ELD, language).**
  Rejected: REQ-CORE-VERSION-001 makes the domains independent, and the
  other six are already at `0.1.0`. Bumping them again would be a no-op at
  best and a breaking change at worst for the binary container, whose
  reader still only accepts `0.1` (REQ-CORE-VERSION-004).
- **Leave the dash-suffix in place and document the discrepancy in the
  README.** Rejected: the version-domains table is meant to be the single
  source of truth for the four identifiers the README cites. A documented
  discrepancy would erode the table's authority and force every reader to
  cross-check two places.
