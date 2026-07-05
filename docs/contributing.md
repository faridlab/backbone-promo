# Contributing to backbone-promo

> Reader: **Contributor** · Mode: **How-to** · Last reviewed 2026-07-05.
> How to open a correct PR against promo on the first try. Assumes you've read the
> [maintainer guide](./maintainer-guide.md) for the regen mechanics.

## Dev setup

1. **Toolchain:** Rust 2021 (stable), plus the `metaphor` CLI (`metaphor --version` → `0.2.0`+).
2. **Database:** a local PostgreSQL. promo owns the `promo` schema; run its migrations before tests.
3. **Confirm you're in the right project:** `metaphor info` should report you're inside
   `backbone-promo` (a `module`).
4. **Build & test loop:**
   ```bash
   metaphor dev test      # canonical
   cargo test             # fine for a tight inner loop inside this project dir
   metaphor lint check    # clippy + fmt — must pass
   ```
   Do **not** run `cargo build`/`cargo test` from the workspace root — each project has its own
   `Cargo.toml`.

## The schema-first workflow (non-negotiable)

promo is a `module`: **`schema/models/*.model.yaml` is the source of truth.** A PR that hand-edits a
generated file outside `// <<< CUSTOM` markers will be rejected — the change vanishes on the next regen.

- Entity/field/index/enum change → **edit the schema**, `metaphor migration generate <name>`,
  regenerate, review the diff.
- Business behavior (pricing/coupon/loyalty rules) → the **hand-owned** write path
  (`promo_write_service.rs`, `promo_ports.rs`, `promo_events.rs`) or a `*_custom.rs` sibling.

See the [maintainer guide walkthrough](./maintainer-guide.md#walkthrough--add-a-feature-without-breaking-conventions).

## Commit conventions

- **Conventional commits.** `feat(promo): …`, `fix(coupon): …`, `docs(promo): …`, `refactor: …`,
  `test: …`, `chore: …`. One-line, imperative, states **why** where it isn't obvious.
- **No signatures.** Do **not** add `Co-Authored-By`, `Generated with`, or any trailer. This is a
  workspace rule (root `CLAUDE.md`) and is enforced in review.
- **No filler messages.** Not "update", "fix stuff", "wip" — say what changed and why.
- Group changes by functionality; small related files together, large files on their own.

## Branching & PRs

1. Branch off `main` (`main` is the default and PR base).
2. Keep the PR scoped — one feature/fix. Cross-project changes require reading `metaphor.yaml` first.
3. In CI, prefer `--affected --base=main` so only what changed is rebuilt/retested.

## The test oracle — prove it or it isn't shipped

promo's guarantees are executable. Every behavior change updates the oracle. The suites (see
[golden cases](./business-flows/golden-cases.md) for the numeric expectations):

| Suite | File | Proves |
|-------|------|--------|
| **Golden cases** | `tests/promo_golden_cases.rs` (PGC-1…6) | Resolution effects, specificity tie-break, condition gating, coupon gate. |
| **Integrity probes** | `tests/integrity_probes.rs` (IP-1…4) | Coupon bounded + idempotent, accrual idempotent, redemption balance-bounded + idempotent, resolve consumes nothing. |
| **Price-resolution seam** | `tests/price_resolution_seam.rs` (PRSEAM-1…3) | The resolved price drives a **real** selling order; loyalty accrues from POS's **real** paid event; the coupon cap binds end-to-end. |
| **Entity API tests** | `tests/integration/tests/*_api_test.rs` | The generated CRUD surface per entity. |

Rules:

- **A new business rule needs a new probe** in `integrity_probes.rs` **and** a numeric row in
  `docs/business-flows/golden-cases.md`. The two must agree.
- **The seam must survive regen.** The `§5` round-trip regenerates promo `--force` and asserts the three
  hand-owned seam files come back byte-identical with all tests green. Don't break it.
- **Prove-by-revert.** For an invariant fix, show the test fails without the fix (the golden cases note
  this for IP-1 and PRSEAM-3).

Unit/integration test *mechanics* are the `test-writer` agent's domain; business-level flows and their
Gherkin are the `business-flow-bdd` skill's. This handbook page owns the *policy* (what must be proven).

## PR checklist

Before you request review:

- [ ] Schema edited first (if entities/fields/indexes/enums changed); no generated file hand-edited
      outside `// <<< CUSTOM`.
- [ ] `metaphor schema schema validate` passes.
- [ ] `metaphor dev test` green (or `cargo test` in-project) — including the seam + round-trip.
- [ ] `metaphor lint check` clean (clippy + fmt).
- [ ] New behavior has a probe **and** a golden-case row; the two agree.
- [ ] The six invariants still hold (see below).
- [ ] promo still posts **no GL** and has **zero normal Cargo edge** to selling/POS
      (`cargo tree -e normal -i backbone-selling` empty).
- [ ] Conventional-commit messages, **no signatures**, no filler.
- [ ] Docs updated if behavior/contract changed; a new decision is a **new ADR**, not an edit to an
      accepted one.

## The invariants a reviewer will check

1. Resolve is **side-effect-free** (previewing consumes no coupon).
2. Resolution is a **total order** (priority → specificity → newest → id).
3. `used_count ≤ max_use` **always**, under concurrency.
4. **One source document = at most one** coupon burn / one accrual (idempotency keys).
5. Loyalty balance `= Σ signed points`; a redemption **never overspends** (advisory-locked).
6. promo **posts no GL** and owns no money of record.

## Decisions & ADRs

A design decision that changes the boundary, the seam, or an invariant is recorded as an **ADR** in
[`docs/adr/`](./adr/) — context, decision, status, consequences. ADRs are **immutable once accepted**;
supersede with a new ADR rather than editing. See
[ADR-001](./adr/ADR-001-pricing-boundary-and-resolution-seam.md) for the format and the parking-lot
convention (each deferred item carries a gate).

---
See also the **[Glossary](./glossary.md)** for the exact meaning of every term used above.
