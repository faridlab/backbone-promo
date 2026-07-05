# backbone-promo — Handbook

> The documentation set for the **promo** bounded context: pricing rules, coupons, and loyalty.
> Type: `module` (a DDD 4-layer library crate). Schema YAML is the single source of truth.
> Last reviewed: 2026-07-05 · Module version `0.1.0` · schema version `2`.

**What promo is, in one sentence:** selling and POS take `unit_price`/`discount` as *given inputs* —
promo is the module that decides those numbers, resolving an effective price per line and running the
loyalty points ledger. It **posts no GL and owns no money of record.**

## Read this by who you are

Every page below names its reader and its [Diátaxis](https://diataxis.fr) mode at the top. Start where
you fit; the index links forward.

| You are… | You want | Start here |
|----------|----------|-----------|
| **Evaluator** | Whether/why to adopt this module | [Philosophy](./philosophy.md) → [Background](./background.md) → [Technology](./technology.md) |
| **App developer** | Install, price a line, run loyalty | [Developer guide](./developer-guide.md) → [Extension guide](./extension-guide.md) |
| **Maintainer** | How the machine works, how to extend it | [Architecture](./architecture.md) → [Maintainer guide](./maintainer-guide.md) |
| **Contributor** | How to land a change | [Contributing](./contributing.md) |

## The handbook

1. **[Philosophy & motivation](./philosophy.md)** — *Evaluator.* The one problem promo exists to solve,
   the "posts no GL" north star, and the honest non-goals.
2. **[Background & prior art](./background.md)** — *Evaluator.* What ERPNext does with pricing and why
   promo borrows the model but rejects the monolith coupling.
3. **[Technology & the "why"](./technology.md)** — *Evaluator + Maintainer.* The stack (Rust, SQLx,
   Axum, PostgreSQL, the port pattern) with a rationale and a rejected alternative for each choice.
4. **[Architecture](./architecture.md)** — *Maintainer.* C4 top-down: context, containers, the DDD
   4-layer component shape, and the two seams traced end-to-end (resolve, accrue).
5. **[Maintainer guide](./maintainer-guide.md)** — *Maintainer.* Schema-YAML SSoT, the regeneration
   pipeline, the `// <<< CUSTOM` markers, and where hand-authored write logic lives.
6. **[Developer guide](./developer-guide.md)** — *App developer.* Install → quickstart → recipes
   (price a line, take a coupon, run loyalty) → configuration → troubleshooting.
7. **[Contributing](./contributing.md)** — *Contributor.* Dev setup, commit conventions, the test
   oracle, and the PR checklist.
8. **[Glossary](./glossary.md)** — *All.* One term, one meaning — the ubiquitous language the whole
   handbook uses.
9. **[ADRs](./adr/)** — *Maintainer.* Accepted decisions, immutable once accepted.
   - [ADR-001 — Promo's boundary and the price-resolution seam](./adr/ADR-001-pricing-boundary-and-resolution-seam.md)

## Product & requirements (the "what")

These predate the handbook and remain the authoritative requirement records — the handbook explains and
extends them, it does not replace them:

- [PRD](./PRD.md) — product requirements: scope, non-goals, success criteria.
- [BRD](./BRD.md) — business rules BR-1…BR-6 (the resolution + idempotency invariants).
- [FSD](./FSD.md) — functional spec: entities, services, HTTP surface, seams, test oracle.
- [Business flows — golden cases](./business-flows/golden-cases.md) — the numeric oracle (PGC / IP / PRSEAM).
- [Extension guide](./extension-guide.md) — the public/stable surface a consumer may depend on.

## Sources of truth (when a doc and the code disagree, the code wins)

- **`schema/models/*.model.yaml`** — every entity. Generated code is downstream of this.
- **`src/application/service/promo_write_service.rs`** — the hand-authored write path (`resolve`,
  `commit_coupon_redemption`, `accrue`, `redeem`). Survives regeneration.
- **`src/application/service/promo_ports.rs`** — the outward contract (`PriceQuery` / `ResolvedPrice` /
  `PriceResolverPort`).
- **`tests/`** — the executable oracle: `promo_golden_cases`, `integrity_probes`,
  `price_resolution_seam`.
