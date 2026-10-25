# ELANG version-compatibility and migration policy (T1611)

> **Scope:** the rules that govern how a v0.1.0 implementation and a
> future `v0.2+` implementation co-exist, what a consumer of a v0.1.0
> package can rely on across releases, and the process by which a
> breaking change enters the format. No `v0.2+` release has been
> declared; this document is the contract that **would** apply when
> one is.

## Versioned identifiers

| Surface | Version | Where it appears |
|---|---|---|
| Binary container | `Header/format.version` (currently `0.1.0`) | First 16 bytes of every `.elang` |
| EPF grammar | `epf "<ver>"` | Top of every `package.epf` |
| ELD grammar | `eld "<ver>"` | Top of every `Linguistics/*.eld` |
| Language version | `language-version "<ver>"` | `package.epf` `metadata` block |
| Repertoire | Pinned by SHA-256 of `specs/schema/ipa-v1.json` (the pinned digest is `PINNED_JCS_SHA256` in `crates/elang-repertoire/src/repertoire.rs`) | Per-package ELD validation |
| Corpus | `corpus: "elang-canonical-zstd-v1-conformance"` in `fixtures/corpus_index.json` | `fixtures/corpus_index.json` |

The three format-owned version identifiers (`Header/format.version`, the EPF
`epf` field, and the EPF `eld` field) are pinned at the v0.1.0 baseline
and follow the rules below. `language-version` is independent metadata owned
by the language author and is excluded from format migrations.

## Semver rules

The ELANG format follows **strict semver** for the three format-owned
version identifiers.

| Bump | When | Reader behaviour |
|---|---|---|
| **Patch** (`0.1.x`) | Normative text corrections; rule clarifications; goldens re-pinned under the same OA set; bug fixes that do not change accepted/rejected inputs. | Every v0.1.0 reader keeps accepting and producing the same set of inputs. New goldens may pin additional inputs as additive coverage. |
| **Minor** (`0.x.0`) | A new OA artifact enters the normative corpus (e.g. a new error code, a new condition field); an optional feature becomes normative; the canonical serialization profile gains a rule. | Existing v0.1.0 readers continue to accept every v0.1.0 input. They may reject new inputs that use the new feature. A v0.2.0 reader must accept every v0.1.0 input. |
| **Major** (`x.0.0`) | A change in normative behaviour that would cause a v0.1.0 reader to reject an input it used to accept, or to produce different bytes from the same input. **This is the only category that breaks v0.1.0.** | Readers of a new major version (e.g. v1.0.0) may reject v0.1.0 inputs, requiring migration. A v0.2.0 reader must still accept every v0.1.0 input. |

## The v0.1.0 contract

A v0.1.0 implementation MUST:

1. Accept every published valid fixture in the
   `elang-canonical-zstd-v1-conformance` corpus
   (`fixtures/corpus_index.json`).
2. Reject every `invalid/` fixture with the recorded stage and error
   code.
3. Produce correct outputs and the specified canonical bytes for the five
   published benchmark operations. Wall-clock timings in
   `docs/benchmarks.md` are measurements under the documented hardware,
   build profile, and methodology, not normative pass/fail requirements.
4. Honour the budget ceilings in `docs/public/security.md`.
5. Emit one of the documented reader-stage / error-code pairs for every
   rejection path.

A v0.1.0 implementation MAY:

* Add additional accept-only behaviour (additional inputs that are
  rejected by v0.1.0 but accepted by the implementation, provided they
  are clearly version-tagged).
* Decline to read a v0.2+ input that uses a new feature, by rejecting
  with `HEADER_BAD_VERSION` (or the equivalent stage-2 error).

A v0.1.0 implementation MUST NOT:

* Accept an input the corpus publishes as invalid.
* Silently re-define an error code.
* Treat the published wall-clock measurements in `docs/benchmarks.md`
  as conformance thresholds or a performance ceiling.

## When a breaking change happens

A major bump is the **only** path that may reject previously valid inputs
or change canonical bytes for existing inputs. The procedure is:

1. **Open a Phase-X issue** titled
   `Breaking: <one-line summary of the change>`, naming the version
   it would land in (e.g. `v1.0.0`) and the normative artifact(s) it
   touches.
2. **Write an ADR** under `docs/decisions/` documenting the change,
   the alternatives considered, and the migration path. The ADR is
   approved under the normal `REQ-GOV-ADR-001` process.
3. **Publish a transitional corpus** under `fixtures/v1.0-transitional/`
   that asserts the **old** behaviour alongside the new one for one
   release cycle, so an implementation on the previous major version
   can verify its conformance without rebuilding the corpus.
4. **Bump** the three format-owned version identifiers atomically:
   `Header/format.version`, EPF `epf`, and EPF `eld`. Keep the language
   author's `language-version` unchanged by the format migration. The
   `Cargo.lock.sha256`, `dependencies.txt`, `repertoire.sha256`, and
   `conformance-hashes.txt` records regenerate.
5. **Run the reproducible-build workflow** against the new commit and
   publish the `reproducible-build-evidence.md` artefact for the new
   release.

Migration tooling (a `migrate` CLI subcommand) is **only** introduced
when the breaking change can be expressed as an automatic
transformation. Manual-only migrations must be documented in
`docs/release/v1.0.0/migration.md` and accompanied by a worked example.

## The `language-version` field

`package.epf`'s `language-version` is the **language author**'s
version of their own phoneme inventory, not the ELANG format version.
It is independent of the three format-owned version identifiers and is owned by
the language package, not by this repository.

A `v0.1.0` reader ignores `language-version` for compatibility
decisions. A reader that encounters a `language-version` it does not
recognise MUST still accept the package, because the field is a hint,
not a constraint.

## What ships in v0.1.0

* `Header/format.version` = `0.1.0`
* EPF `epf "<ver>"` = `0.1.0`
* ELD `eld "<ver>"` = `0.1.0`
* Normative manifest pinned checksum =
  `681022fbdd9a91d4e017c187884de7e6912409bee4267399464f8e4dc7d72e46`
* Corpus name = `elang-canonical-zstd-v1-conformance`
  (the `corpus` field of `fixtures/corpus_index.json`; the normative
  manifest itself carries no `corpus` field)

A future v0.2.0 release will update each of these atomically.

## See also

* [`docs/release/v0.1.0/README.md`](README.md) — the v0.1.0 release notes
* [`docs/release/v0.1.0/ARCHIVE.md`](ARCHIVE.md) — the archival checksum metadata
* [`docs/decisions/`](../../decisions/) — the ADR process
* [`docs/open-artifacts.md`](../../open-artifacts.md) — which normative artifacts are ratified vs. open
