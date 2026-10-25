# Concepts

This page defines the vocabulary used throughout the ELANG documentation.

## Package

A single `.elang` file. It is a custom binary container (not a ZIP) that holds a
set of **logical resources**, an index describing them, and a footer that
content-addresses the index.

## Logical resource

A named byte stream inside a package, addressed by a slash-separated relative
path such as `Linguistics/phonemes.eld`. Each resource is stored as exactly one
independent Zstandard frame. A compliant v0.1.0 package contains:

| Resource | Role |
|----------|------|
| `Header/format.version` | Container format version string (`0.1.0`). |
| `Header/mimetype` | Exact media type `application/vnd.elang`. |
| `package.epf` | KDL 2.0 descriptor: version domains, metadata, manifest. |
| `META-INF/integrity.kdl` | SHA-256 manifest over the other resources. |
| `*.eld` | The phoneme inventory (one per package). |

## EPF — the package descriptor

`package.epf` is a KDL 2.0 document with exactly five top-level nodes: `epf`,
`eld`, `language-version`, `metadata`, and `manifest`. It carries the language's
identity (a URN), human-readable metadata, and the manifest entry that points to
the phoneme inventory. See the [EPF reference](format/epf.md).

## ELD — the Linguistic Definition extension

`.eld` is the **internal KDL 2.0 extension** ELANG uses for linguistic
inventories. Each linguistic resource a language definition needs — phonemes
in v0.1.0, and other linguistic types (the lexicon, morphology, phonotactics,
syntax, prosody, etc.) in future versions — is carried as a distinct top-level
node within an `.eld` document.

v0.1.0 defines **only the `phonemes` module**. Within that module, an ELD
document's top-level `phonemes` node contains **groups** (`consonants`,
`vowels`, and up to 14 custom groups), each holding **root phonemes**. A root
has an IPA identifier, a romanization, optional conditional **allophones**,
and optional `when` conditions. See the [ELD reference](format/eld.md).

## Phoneme, allophone, romanization

- A **root phoneme** is an underlying sound, identified by an IPA string (for
  example `t`).
- An **allophone** is a conditional surface pronunciation of a root (for example
  `ɾ` or `tʰ`). An allophone may carry its own romanization and `when`
  condition; if it omits a romanization, it inherits the root's.
- A **romanization** (`roman`) is a 1–8 letter ASCII spelling. Explicit
  romanizations are globally unique, compared case-insensitively.

## IPA identifier

A validated IPA string used to identify a root or allophone. ELANG allows only
symbols from a pinned, reviewed allowlist (the **IPA Repertoire v1**, drawn from
the IPA 2026 chart), enforces Unicode NFC, and applies strict rules on logical
units, modifiers, and tie bars. See [Unicode & IPA](format/unicode-ipa.md).

## `when` condition

An optional, compact boolean expression attached to a root or allophone that
describes the phonetic context in which it is eligible. It combines four atomic
predicates (`word-position`, `syllable-position`, `previous-phoneme`,
`next-phoneme`) using implicit AND (`when`/`group`) and binary `or`. The
evaluator returns an **eligibility set**, never a probability or priority. See
the [`when` language reference](format/when.md).

## Integrity manifest

`META-INF/integrity.kdl` records a SHA-256 digest of the **uncompressed** bytes
of every other indexed resource (excluding itself). It proves the package's
contents are intact.

!!! warning "Integrity is not authorship"
    A matching digest proves a package has not been altered. It says nothing
    about *who* produced it. URN-like language identifiers do not prove
    ownership, and ELANG v0.1.0 defines no publisher signatures.

## Canonical form

A deterministic serialization. The canonical ELD writer fixes group/root/
allophone ordering, indentation, line endings, string quoting, and condition
ordering so that semantically equal inventories produce byte-identical output.
The canonical Zstd profile does the same for compressed frames. Canonical output
is **idempotent**: canonicalizing an already-canonical document changes nothing.

## Reader modes and validation stages

The reader is a fail-closed state machine with 12 stages. `inspect` runs stages
1–7 (structure), `verify` runs 1–11 (structure plus integrity), and `validate`
runs 1–12 (plus full ELD semantics). See [Validation pipeline](format/validation.md).

## Version domains

ELANG keeps several **independent** version numbers — for the specification, the
binary container, the EPF schema, the ELD schema, each language release, and the
canonical compression profile. A change in one does not imply a change in
another. See [Versioning](versioning.md).
