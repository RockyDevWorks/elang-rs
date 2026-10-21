# Validation pipeline

The ELANG reader is a **fail-closed state machine** with exactly **12 logical
stages**. Each stage has typed inputs and outputs and produces stage-tagged,
machine-readable errors. The three CLI modes expose increasing depth.

## The 12 stages

| # | Stage | What it checks |
|--:|-------|----------------|
| 1 | Archive preflight | File size, bounds, minimal structure. |
| 2 | Header | Magic, versions, flags, reserved bytes. |
| 3 | Footer | Magic, offset consistency (checked arithmetic). |
| 4 | Index hash | SHA-256 of the exact index bytes equals the footer hash. |
| 5 | Index parse | Entries and paths (NFC, ordering, no duplicates). |
| 6 | Frame layout | Contiguity: no gaps, overlaps, or unindexed bytes. |
| 7 | Frame headers | Per-frame size, window, checksum, dictionary rules. |
| 8 | Version & mimetype | Decode and validate `Header/format.version` and `mimetype`. |
| 9 | Integrity manifest | Parse and validate the manifest structure. |
| 10 | EPF | Parse and validate `package.epf`. |
| 11 | Integrity verify | Decompress **all** resources (bounded) and verify SHA-256. |
| 12 | ELD semantics | Full phoneme-inventory validation. |

## Mode boundaries

```text
stage 1 ─ ... ─ stage 7     ← inspect ends here
        ─ ... ─ stage 11    ← verify ends here
        ─ ... ─ stage 12    ← validate ends here
```

| Mode | Stages | Meaning |
|------|--------|---------|
| `inspect`  | 1–7  | Structure is well formed. **Not an integrity check.** |
| `verify`   | 1–11 | Structure plus full SHA-256 integrity of every resource. |
| `validate` | 1–12 | Everything above, plus ELD phoneme semantics. |

Stage 11 is the **only** stage that decompresses every resource. `inspect`
deliberately does not verify integrity, and the public API returns distinct
result types so a caller cannot mistake inspection for verification.

!!! warning
    A successful `inspect` says nothing about whether a package's contents match
    its integrity manifest. Use `verify` or `validate` for that.

## Inside stage 12: ELD semantics

Stage 12 runs its own pipeline over the phoneme inventory:

```text
budgeted KDL AST
  ─▶ structural schema (groups / roots / allophones)
  ─▶ IPA identifier parse + repertoire allowlist check
  ─▶ romanization rules (regex, case-insensitive uniqueness, inheritance)
  ─▶ when AST build (arity / depth / predicate budgets)
  ─▶ reference resolution (forward / self / allophone → root)
  ─▶ global uniqueness (IPA, including tie-bar aliases)
  ─▶ validated immutable model  ─▶ (optionally) canonical ELD bytes
```

The `when` **evaluator** is a separate pure function over a supplied context and
returns an eligibility set — it is not part of acceptance/rejection validation.

## Error reporting

Every failure reports the **stage**, the **resource**, and a **stable error
code**. Where possible, errors carry the byte offset and the KDL source span.
Diagnostics are capped (at 100) so that a flood of errors cannot exhaust memory —
and truncating diagnostics never turns a failure into a success.
