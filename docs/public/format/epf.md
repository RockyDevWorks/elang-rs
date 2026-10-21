# EPF — package descriptor

`package.epf` is the package's descriptor, written in [KDL 2.0](https://kdl.dev).
It declares the format version domains, the language's identity and metadata, and
the manifest entry pointing to the phoneme inventory.

## Example

```kdl
epf "0.1.0"
eld "0.1.0"
language-version "1.0.0"
metadata {
    identifier "urn:elang:language:rocky:aelith:1.0.0"
    name "Aelith"
    description "A small constructed language."
}
manifest {
    phonemes "Linguistics/phonemes.eld"
}
```

## Top-level nodes

An EPF document has **exactly five** top-level nodes, each appearing **exactly
once**. Source order is arbitrary. Unknown nodes, duplicates, stray properties,
extra arguments, and disallowed child blocks are all rejected.

| Node | Form |
|------|------|
| `epf` | one string argument, no block — must be `"0.1.0"` |
| `eld` | one string argument, no block — must be `"0.1.0"` |
| `language-version` | one string argument, no block — strict `MAJOR.MINOR.PATCH` |
| `metadata` | no arguments, mandatory child block |
| `manifest` | no arguments, mandatory child block |

## `metadata`

| Child | Requirement |
|-------|-------------|
| `identifier` | Required once. A URN (below). Nonempty, ≤ 512 UTF-8 bytes. |
| `name` | Required once. Nonempty, ≤ 256 UTF-8 bytes. |
| `description` | Optional (at most once). ≤ 4096 UTF-8 bytes; may be empty. |

### The language URN

`metadata.identifier` is a URN with this exact shape:

```text
urn:elang:language:<namespace>:<language-name>:<version>
```

- `<namespace>` and `<language-name>` are ASCII **lowercase slugs**: letters,
  digits, and hyphens, beginning and ending with an alphanumeric character.
- `<version>` is strict `MAJOR.MINOR.PATCH` with no leading zeros.

The `language-version` top-level node **must equal the URN's version suffix
exactly**.

!!! warning "A URN is not proof of ownership"
    The URN identifies a language release. It does not establish who published
    it. ELANG v0.1.0 defines no publisher signatures.

## `manifest`

The manifest declares which resources the package exposes. In v0.1.0 it has
**exactly one** child:

| Child | Requirement |
|-------|-------------|
| `phonemes` | One nonempty string argument: a safe, package-relative `.eld` path that is present in the container index. |

There is **no `modules` node** in EPF v0.1.0, and any other manifest entry is
rejected.

## Canonical serialization

EPF has a ratified canonical serializer profile that fixes the top-level node
order (`epf`, `eld`, `language-version`, `metadata`, `manifest`), string encoding,
whitespace, comments, and newlines, so a given descriptor always serializes to
the same bytes.

## Validation summary

An EPF document is accepted only if:

- it has exactly the five required nodes, each once;
- `epf` and `eld` are both `"0.1.0"`;
- the `metadata` URN is well formed and `language-version` matches its suffix;
- `metadata.name` is present and within length;
- `manifest` has exactly one `phonemes` entry whose `.eld` path is present in the
  index;
- there are no unknown fields, duplicate fields, extra arguments, or nested
  blocks anywhere.
