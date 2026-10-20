---
name: Bug report
about: Report incorrect parsing, validation, canonicalization or a crash
title: "[bug] "
labels: ["bug", "needs-triage"]
---

## Summary

A clear, concise description of the bug.

## Affected area

- [ ] Container (header/index/footer/frames)
- [ ] KDL parsing / budgets
- [ ] EPF schema
- [ ] ELD structure / IPA / `when`
- [ ] Integrity manifest
- [ ] Canonical serialization
- [ ] CLI
- [ ] Other:

**Requirement ID(s) involved (if known):**

## Reproduction

Steps or a minimal input. **Attach** any `.elang` / `.eld` / `.epf` reproducer
as a file rather than pasting binary inline.

```text
<command or code>
```

## Expected vs actual

- Expected:
- Actual (include the exact error code / stage if available):

## Environment

- ELANG version / commit:
- OS and architecture:
- Rust / toolchain:

## Security note

If this is a potential security vulnerability (crash, OOB, decompression bomb,
path traversal), please follow [SECURITY.md](../../SECURITY.md) and report
privately instead of filing a public issue.
