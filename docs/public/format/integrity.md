# Integrity

Every package carries `META-INF/integrity.kdl`, a KDL 2.0 manifest that records a
SHA-256 digest of the **uncompressed** bytes of every other indexed resource,
excluding itself.

## Two independent hashes

ELANG uses SHA-256 in two distinct places, verified separately:

1. **Index hash (footer).** The 64-byte footer stores the SHA-256 of the exact
   serialized index bytes. This is checked at stage 4, before any resource is
   decompressed.
2. **Resource integrity (manifest).** `META-INF/integrity.kdl` stores a SHA-256
   over each resource's uncompressed bytes. These are verified at stage 11, after
   bounded decompression.

The index hash protects the container's own structure; the manifest protects the
resource contents.

## What the manifest covers

- The manifest covers **every indexed resource except `integrity.kdl` itself**.
- A manifest entry for a resource that is not in the index is rejected.
- A missing manifest entry for an indexed resource is rejected.

Verification streams SHA-256 over the exact uncompressed bytes, compares the
result (using a constant-time comparison where appropriate), and reports
per-resource failures. The manifest is parsed under a 16 MiB cap, with unknown or
duplicate fields rejected.

## Integrity is not authorship

!!! danger "What a verified digest does and does not prove"
    A matching digest proves the package's contents are **intact and
    unaltered**. It does **not** identify who produced the package.

    - URN-like language identifiers do **not** prove ownership.
    - ELANG v0.1.0 defines **no publisher signatures**.
    - A verified hash is not a trust decision about the publisher.

This is a deliberate, documented non-goal. Treating a hash — or a URN — as proof
of authorship is a mistake the format and tooling are designed to avoid.

## Status

The complete `META-INF/integrity.kdl` grammar and its canonical serialization,
including the self-exclusion rule, have been ratified. See
[Specification status](../spec-status.md) for what remains open.
