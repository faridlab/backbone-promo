# Maintainer guide

> Reader: **Maintainer** · Mode: **How-to** · Last reviewed 2026-07-05.
> You know Rust and the backbone conventions. This page shows how to change promo without breaking
> regeneration. It assumes competence and gets to the point.

## The one rule that governs everything

**`schema/models/*.model.yaml` is the single source of truth.** The codegen pipeline
(`metaphor-schema`) regenerates entities, DTOs, migrations, repositories, service aliases, handlers, and
routes *from it*. **Regeneration preserves only code inside `// <<< CUSTOM … // END CUSTOM` blocks and
hand-owned sibling files.** Everything else is overwritten.

So the mental model is: *edit the schema, regenerate, then customize inside the safe zones.* Never
hand-edit a generated file outside its CUSTOM markers — your change silently disappears on the next
regen.

## The golden path

Run these from **inside** `backbone-promo/` (they act on this project). The forms below are the working
CLI commands (confirmed against `metaphor 0.2.0`; do not invent flags — check `metaphor <cmd> --help`).

```bash
metaphor schema schema validate     # check the schema YAML parses + is consistent
metaphor make entity <Name>         # scaffold a new entity from schema
metaphor migration generate <name>  # emit a new migration from a schema change
metaphor dev test                   # run the module's tests
metaphor lint check                 # clippy + fmt gate
```

> **Never** run `cargo build`/`cargo test` from the *workspace root* — each project has its own
> `Cargo.toml`. Inside this project directory, `cargo test` is fine for a tight loop, but
> `metaphor dev test` is the canonical entry point.

## What is generated vs. hand-authored

| You want to change… | Edit… | Then… |
|---------------------|-------|-------|
| A field, index, enum, validity window | `schema/models/<entity>.model.yaml` | `metaphor migration generate <change>` → regenerate → review |
| A new entity | add `schema/models/<entity>.model.yaml`, register in `index.model.yaml` `imports:` | `metaphor make entity <Name>` → wire the service in `src/lib.rs` builder |
| Pricing/coupon/loyalty **behavior** | `src/application/service/promo_write_service.rs` ★ | it's hand-owned — just edit it |
| The outward contract | `src/application/service/promo_ports.rs` ★ | hand-owned |
| Domain events | `src/application/service/promo_events.rs` ★ | hand-owned |
| Extra read model | new `*_service_custom.rs` sibling | never regenerated |
| Builder wiring / custom routes | `// <<< CUSTOM` blocks in `src/lib.rs`, `src/routes/mod.rs` | preserved across regen |

★ These three files are hand-authored and **not** produced by the generator. They are the domain core.

## The three regen-safe zones

1. **`// <<< CUSTOM … // END CUSTOM` markers** inside generated files. Example — the module builder in
   [`src/lib.rs`](../src/lib.rs):

   ```rust
   // <<< CUSTOM - custom builder methods
   // END CUSTOM
   ```

   Anything between the markers survives; anything outside is regenerated. Use these for builder
   methods, extra struct fields on `PromoModule`, and custom route registration.

2. **Sibling `*_custom.rs` files** (e.g. `pricing_rule_service_custom.rs`). Never touched by regen. Put
   read models / projections here.

3. **Hand-owned files with no generated twin** — `promo_write_service.rs`, `promo_ports.rs`,
   `promo_events.rs`. These *are* the write path. They live under `application/service/` alongside the
   generated aliases but are authored, not generated.

The round-trip is proven: `§5` of the [golden cases](./business-flows/golden-cases.md) regenerates promo
with `--force` and asserts the three hand-owned seam files come back **byte-identical** and all tests
stay green.

## Walkthrough — add a feature without breaking conventions

**Goal: add a `usage_limit_per_customer` cap to `CouponCode`** (illustrative).

1. **Edit the schema.** In `schema/models/coupon_code.model.yaml`, add the field under `fields:`:

   ```yaml
   usage_limit_per_customer:
     type: int?
     attributes: ["@positive"]
     description: "Max redemptions by a single customer (null = no per-customer cap)"
   ```

2. **Validate.** `metaphor schema schema validate` — fix any DSL error before generating.

3. **Generate the migration.** `metaphor migration generate coupon_per_customer_cap` — review the
   emitted `migrations/NNN_*.up.sql` / `.down.sql`. Migrations are **additive and reviewed**; never
   hand-edit a generated migration's shape without re-deriving it from the schema.

4. **Regenerate the entity + DTOs.** The generator refreshes `domain/entity/coupon_code.rs`, the DTOs,
   the repository, and the handler. Your `// <<< CUSTOM` blocks are preserved.

5. **Enforce the rule in the write path.** The *cap* is behavior — add it to
   `commit_coupon_redemption` in `promo_write_service.rs` (hand-owned): count this customer's prior
   redemptions in the same transaction, reject past the limit. Generated CRUD won't enforce a business
   rule; the write service is where invariants live.

6. **Prove it.** Add a probe to `tests/integrity_probes.rs` mirroring the IP-style oracle, and a numeric
   row to [`docs/business-flows/golden-cases.md`](./business-flows/golden-cases.md). A behavior with no
   test is not shipped.

7. **Run the gate.** `metaphor dev test && metaphor lint check`.

## Where new code goes, by intent

- **A new CRUD entity** → schema YAML + `metaphor make entity`; register its service in the `build()`
  method of `PromoModuleBuilder` (`src/lib.rs`) and merge its routes in `all_crud_routes()` and
  `src/routes/mod.rs`.
- **A new invariant on an existing entity** → the hand-owned write service, not the CRUD path.
- **A new outward verb** (something selling/POS will call) → add it to `promo_ports.rs` (the DTO +
  trait) and implement in `promo_write_service.rs`. Keep the caller depending on the *port*.
- **A new domain event** → add the struct + a `PromoEvent` variant in `promo_events.rs`; publish it from
  the write path.

## Migrations & seeds

- Migrations live in `migrations/` as timestamped `*.up.sql` / `*.down.sql` pairs. promo owns the
  `promo` Postgres schema (`CREATE SCHEMA promo`); tables are `promo.<collection>`.
- Seed order is declared in `migrations/seeds/seed_order.yml` (FK-respecting order:
  coupon_code → loyalty_program → loyalty_point_entry → pricing_rule → coupon_redemption).
- Seeders run via the generated binary: `cargo run --bin promo-seeder` (add `--force` to reseed).
- **Read [database-migration-specialist](../CLAUDE.md#deeper-knowledge-load-on-demand)** (skill) before
  a non-additive migration; use only backbone-generated migrations.

## Invariants you must not break

These are the load-bearing guarantees — every change must keep them true (they are the [BRD](./BRD.md)
BR-1…BR-6, tested in `integrity_probes.rs`):

- **Resolve is side-effect-free.** Previewing a price never consumes a coupon.
- **Resolution is a total order** (priority → specificity → newest → id) — deterministic.
- **`used_count ≤ max_use` always**, under concurrency (guarded increment).
- **One source document = at most one coupon burn / one accrual** (idempotency keys).
- **Loyalty balance = `Σ signed points`; a redemption never overspends** (advisory-locked, balance-checked).
- **promo posts no GL.** Never route a revenue/cash post through this module.

## Related skills (load on demand)

- `backbone-schema-maintainer` — the schema YAML DSL and generators.
- `custom-logic-specialist` — writing logic that survives regen.
- `database-migration-specialist` — safe PostgreSQL migrations.
- `modules-orchestrator` — composing promo into a service.

---
Next: **[Developer guide](./developer-guide.md)** — consuming promo from a service.
