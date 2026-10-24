# ELANG

**ELANG** is a portable, deterministic definition and packaging format for
constructed languages, together with its Rust reference tooling. A single
`.elang` file carries a language's definition in a compact binary container that
is content-addressed, reproducible byte-for-byte, and safe to open even when it
comes from an untrusted source.

This site documents the **ELANG format** and the **Rust reference
implementation** (`elang-rs`): the binary container, the descriptor and phoneme
grammars, the integrity and Unicode/IPA rules, the reader/validator pipeline,
and the command-line tooling.

!!! note "Scope of this release"
    ELANG v0.1.0 defines **phonemes only**. It does not define lexicons,
    phonotactics, morphology, syntax, semantics, prosody, audio, GUI, dependency
    management, repository ownership, or publisher signatures. A verified package
    digest proves **integrity, not authorship**.

## What is in a package

Every compliant phoneme-only package contains exactly these logical resources:

```text
Header/format.version
mimetype
package.epf
META-INF/integrity.kdl
Linguistics/phonemes.eld   # path is configurable via the EPF manifest
```

- **`Header/format.version`** — the container format version string.
- **`mimetype`** — the exact media type `application/vnd.elang`.
- **`package.epf`** — a [KDL 2.0](https://kdl.dev) descriptor naming the language
  and its resources.
- **`META-INF/integrity.kdl`** — a SHA-256 manifest over the uncompressed bytes
  of every other resource.
- **`.eld`** — the ELANG Linguistic Definition extension: an internal
  [KDL 2.0](https://kdl.dev) document that carries linguistic inventories
  as distinct top-level modules. v0.1.0 defines only the **`phonemes`**
  module — consonants, vowels, custom groups, romanizations, allophones,
  and conditional realization rules. Future versions will define additional
  modules (lexicon, morphology, phonotactics, syntax, prosody, etc.).

## Why ELANG

- **Deterministic.** A canonical writer profile produces byte-identical output
  across runs and platforms, so packages are reproducible and content-addressable.
- **Safe by construction.** Every byte in a package is treated as untrusted. The
  reader uses bounded, streaming, checked-arithmetic code with hard limits on
  decompression, nesting, and allocation.
- **Explicit, not clever.** Ambiguous syntax is rejected rather than repaired.
  Nothing is silently normalized. Validation either passes every stage or fails
  with a precise, stable error.
- **Pinned and auditable.** Unicode 18.0.0, the IPA 2026 chart, and libzstd 1.5.7
  are pinned so behavior does not drift with the host environment.

## Start here

<div class="grid cards" markdown>

- :material-rocket-launch: **[Getting started](getting-started.md)**
  Install the tooling, build a package, and run the validator.

- :material-book-open-variant: **[Concepts](concepts.md)**
  The format's building blocks and the vocabulary used throughout.

- :material-file-tree: **[Format reference](format/container.md)**
  The binary container, descriptor, phoneme grammar, and canonical rules.

- :material-console: **[CLI reference](cli.md)**
  `inspect`, `verify`, `validate`, `pack`, `unpack`, and more.

- :material-shield-check: **[Security model](security.md)**
  The threat model and the defenses that back it.

- :material-tag-outline: **[Versioning](versioning.md)**
  The independent version domains and what each one governs.

</div>

## Project status

ELANG is pre-release, tracking **v0.1.0**. The locked format rules on this site
are stable; a small number of normative data artifacts (for example, the final
frozen IPA inventory and release golden hashes) are tracked as open items and are
release blockers. See [Specification status](spec-status.md) for the current
picture.
