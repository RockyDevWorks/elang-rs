# ADR 0012: Reader diagnostic stage metadata

- **Status:** Approved
- **Date:** 2026-10-24
- **Supersedes:** none
- **Superseded by:** none
- **Related requirements:** REQ-READER-001, REQ-READER-005
- **Related tasks:** T1207, T1214

## Context

The registry previously assigned `HEADER_BAD_MAGIC` only to stage 2 and
most frame failures only to stages 7 or 11. Reader integration requires
errors to identify the stage and resource where they surface. The requested
reader correction explicitly assigns bad header magic to stage 1.

## Decision

`Package::open` tags `HEADER_BAD_MAGIC` as stage 1. Stage 2 remains registered
for direct header decoding, matching steering §4's header-validation stage.
This is an explicit reader diagnostic compatibility change.

Frame errors preserve their existing codes and `FRAME` category when decoding
at stages 8–12. Register these stages alongside the existing inspection stages;
`RESOURCE_MISSING` retains its registered `RESOURCE_PATH` category. Registry
Markdown and JSON must agree, and their generated manifest and pinned checksum
must be refreshed following ADR 0011.

## Consequences

Consumers filtering reader errors by stage must accept bad header magic at
stage 1 and frame errors at the calling decode stage. No code is renamed and
archive acceptance rules are unchanged. Reader failure tests assert the code,
category, stage and resource path.

## Alternatives considered

Keeping the old stage metadata would contradict the requested reader behavior
and leave legitimate decode failures outside the registry's documented stages.
