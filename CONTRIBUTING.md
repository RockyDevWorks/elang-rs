# Contributing to ELANG

Thanks for your interest in ELANG. This project follows a strict,
specification-first workflow because ELANG defines a deterministic, byte-exact
format. Small changes to parsing or canonicalization can break interoperability,
so contributions are reviewed against the normative spec and the conformance
corpus.

## Ground rules

1. **The steering contract is authoritative.** Read
   [`.kiro/steering/elang.md`](.kiro/steering/elang.md). `MUST`/`MUST NOT` are
   mandatory; `SHOULD` is preferred; `MAY` is optional. Never silently change a
   locked rule.
2. **Do not invent normative data.** The exact IPA allowlist, its SHA-256, and
   the integrity-manifest grammar are *open artifacts*. Producing them requires
   an explicit, reviewed artifact plus test vectors — not a guess.
3. **No change to a locked rule without an approved ADR.** See
   [`docs/decisions/`](docs/decisions/).
4. **Security first.** Code that touches untrusted input must be bounded,
   streaming where possible, and free of panics and unchecked indexing.

## Workflow

1. Open an issue describing the change and the requirement ID(s) it affects.
2. Branch from `main`; never push directly to `main`.
3. Implement in small, reviewable commits. Each task from
   [`.kiro/specs/elang-v0.1.0/tasks.md`](.kiro/specs/elang-v0.1.0/tasks.md)
   should map to a requirement and a test.
4. Update the traceability matrix
   ([`docs/traceability.md`](docs/traceability.md)) so every `MUST` keeps test
   coverage.
5. Open a pull request. Describe what changed, what you tested, and any blocked
   or deferred items.

## Definition of done (per task)

- [ ] Implementation committed with API docs.
- [ ] Unit/integration tests prove the required behavior **and** at least one
      rejection path where relevant.
- [ ] Resource limits and error handling checked for untrusted input.
- [ ] Requirement ID and conformance fixture/test linked.
- [ ] `cargo fmt`, `cargo clippy`, `cargo test` and CI all pass.
- [ ] No locked rule changed without an approved ADR.

## Local checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

## Commit and review conventions

- Keep commits focused; reference the task ID (e.g. `T0203`) and requirement ID.
- Prefer adding fixtures over ad-hoc test data; fixtures live under `fixtures/`
  with the naming conventions documented in the spec.
