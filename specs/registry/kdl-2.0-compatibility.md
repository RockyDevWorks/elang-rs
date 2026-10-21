# KDL 2.0 compatibility review of ELANG examples and canonical serialization

> **Task:** T0120. **Normative source:** steering `.kiro/steering/elang.md`
> §5, §6, §8, §9. This is a review note, not a new rule.

## Scope

Confirm that every ELANG example and every canonical serialization profile
produces documents that parse under **KDL 2.0** (not 1.0), and flag any KDL 2.0
feature ELANG deliberately constrains more tightly than the base grammar.

## KDL version

ELANG targets KDL **2.0** exclusively (steering §5, §6; design §1). The parser
adapter (Phase 5, T0501) must verify it implements KDL 2.0 grammar, not 1.0.
Key KDL 2.0 differences ELANG relies on vs 1.0:

- String handling and escape set follow KDL 2.0.
- Node/argument/property model (`name (arg)* (key=value)* { children }`) is the
  structure every ELANG grammar is expressed over.

## ELANG constraints tighter than base KDL 2.0

These are intentional ELANG restrictions; a document can be valid KDL 2.0 yet
rejected by ELANG:

| Area | KDL 2.0 allows | ELANG requires |
|---|---|---|
| Node/property names | broad identifier set | only the enumerated ELANG node names per grammar |
| Type annotations `(type)` | permitted | not used by ELANG; a validator treats annotations as unknown syntax to reject at the schema layer |
| Arguments/properties | arbitrary counts/types | exact per-node arity and types (see EPF/ELD/integrity grammars) |
| Numbers/bools/null values | first-class | ELANG values are strings (or the fixed `sha256` hex property); non-string values rejected where a string is required |
| Whitespace/comments | flexible, incl. slashdash `/-` | accepted on **input**; canonical **output** emits none |
| Child blocks | optional per node | required/forbidden per ELANG grammar (e.g. mandatory nonempty `metadata` block; bare vs block allophone) |

## Canonical output is valid KDL 2.0

The three canonical profiles — ELD (locked §9, with residual edge cases
ratified in [ADR 0009](../../docs/decisions/0009-residual-eld-canonicalization-edge-cases.md)),
EPF (ADR 0005) and integrity (ADR 0004) — all emit:

- standard double-quoted strings with `"`/`\` escaped,
- 4-space indentation, one node per line, LF endings, single trailing LF,
- no comments, no blank lines, no type annotations, no slashdash.

This is a strict subset of KDL 2.0 surface syntax, so canonical output
round-trips through any conformant KDL 2.0 parser. The reverse does not hold:
arbitrary KDL 2.0 is not necessarily valid ELANG (see the table above).

## Action for the parser adapter (Phase 5)

- Assert KDL 2.0 support explicitly; reject KDL 1.0-only constructs.
- Surface type annotations and non-string scalars to the schema layer as
  rejectable rather than silently coercing them.
- Preserve source spans so schema errors can carry `kdlSpan` (error registry).
