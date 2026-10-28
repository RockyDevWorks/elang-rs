# Fuzz triage discipline

The fuzzing pipeline produces findings on its own schedule; producing a
useful response to a finding is the responsibility of the on-call
engineer. This document captures the three rules that every fuzz
finding record must satisfy before the lifecycle can move past
"reproduced".

## The three rules

1. **No finding record without a committed reproduction.** Every
   `fuzz/findings/<NNNN>-<slug>.md` entry must link to a `git`-pinned
   corpus entry in `fuzz/regressions/` that crashes on a clean checkout of
   `main`. If the corpus cannot be reproduced, the finding is
   downgraded to a hypothesis and the record is closed without a fix.

2. **No regression test that calls the wrong code path.** When a
   finding is fixed, the regression test must exercise the exact
   production function the fuzz target reached. Adding a regression
   test against a wrapper, mock, or adjacent path is rejected at code
   review — it would not catch a regression in the original target.

3. **No `fixed` finding without a passing regression test.** A
   `fixed` finding carries the commit that closed it and a link to a
   `proptest`/`cargo test` assertion that fails before the fix and
   passes after. Both must be present; either missing is grounds for
   reopening the finding.

## Why this is a separate document

These rules are part of the public safety story for the fuzzing
pipeline (referenced from `docs/security-testing.md`). Keeping them
in their own file keeps `security-testing.md` focused on the
threat model and lets the rules evolve independently.
