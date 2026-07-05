# Developer guide

> Reader: **App developer** · Mode: **Tutorial → How-to** · Last reviewed 2026-07-05.
> You are building a `backend-service` that needs pricing, coupons, or loyalty. This gets you from
> zero to a priced line. Examples use the real types in
> [`src/application/service/promo_ports.rs`](../src/application/service/promo_ports.rs).

## Install

promo is a library crate consumed by a host service. Add it to your service's `Cargo.toml`:

```toml
[dependencies]
backbone-promo = { path = "../backbone-promo" }   # or a git ref, matching your workspace
```

Requirements: **Rust 2021**, a reachable **PostgreSQL** where promo's migrations have run (it owns the
`promo` schema), and Tokio. Run promo's migrations before first use (see the
[maintainer guide](./maintainer-guide.md#migrations--seeds)).

## Quickstart — build the module, mount CRUD

The smallest thing that runs: wire the module over a pool and expose the admin CRUD surface.

```rust
use backbone_promo::PromoModule;
use sqlx::PgPool;

async fn build(pool: PgPool) -> anyhow::Result<axum::Router> {
    let promo = PromoModule::builder()
        .with_database(pool)
        .build()?;

    // Admin/seeding only — UNGUARDED full CRUD (12 endpoints × 5 entities).
    // For production, compose a guarded router instead (see below).
    Ok(promo.all_crud_routes())
}
```

That gives you, per entity (`pricing_rules`, `coupon_codes`, `coupon_redemptions`,
`loyalty_programs`, `loyalty_point_entries`), the 12 standard endpoints:

| Route | Method | Purpose |
|-------|--------|---------|
| `/api/v1/{collection}` | GET / POST | list (paginate/filter/sort) / create |
| `/api/v1/{collection}/:id` | GET / PUT / PATCH / DELETE | get / update / patch / soft-delete |
| `/api/v1/{collection}/:id/restore` | POST | restore a soft-deleted row |
| `/api/v1/{collection}/bulk` · `/upsert` | POST | bulk create / upsert |
| `/api/v1/{collection}/trash` · `/empty` | GET / DELETE | list deleted / empty trash |
| `find_by_id`, `list_deleted` | — | the remaining generated reads |

> ⚠️ `all_crud_routes()` (and the deprecated `routes()`) are **unvalidated** — a well-formed request can
> create an invalid row or soft-delete a referenced master. Use the unguarded surface only for
> admin/seeding. For production, mount **reads** from the CRUD surface and route **writes** through your
> own validation, or a guarded composition. This is flagged on the methods in
> [`src/lib.rs`](../src/lib.rs).

## Key concept — the port, not the crate

The hot-path work (pricing a line) does **not** go through HTTP and does **not** require depending on
`backbone-promo` at all. Your selling/POS code holds a **`PriceResolverPort`** trait object; a composing
layer wires promo's write service behind it. Depending on the port (not the crate) keeps your module
free of a promo Cargo edge. See [ADR-001](./adr/ADR-001-pricing-boundary-and-resolution-seam.md).

## Recipe — price a line

Build a `PriceQuery` for the line, call `resolve`, charge `ResolvedPrice.unit_price`. The port and its
DTOs are re-exported from `backbone_promo::application::service` (not the crate root). To get a
`PriceResolverPort`, wrap the write service: `PromoPriceResolver { service: Arc::new(PromoWriteService::new(pool)) }`.

```rust
use backbone_promo::application::service::{PriceQuery, PriceResolverPort, ResolvedPrice};
use rust_decimal_macros::dec;

async fn price_line(resolver: &dyn PriceResolverPort) -> anyhow::Result<ResolvedPrice> {
    let query = PriceQuery {
        company_id,
        list_price: dec!(100000),      // what you'd otherwise charge
        quantity:   dec!(2),
        item_id,
        item_group_id: None,
        brand_id: None,
        customer_id: Some(customer_id),
        customer_group_id: None,
        coupon_code: None,             // no coupon on this line
        at: chrono::Utc::now(),        // evaluate validity windows against this instant
    };

    let priced = resolver.resolve(&query).await?;
    // priced.unit_price is what to charge; == list_price when no rule applied.
    Ok(priced)
}
```

- **No applicable rule → passthrough:** `unit_price == list_price`, `discount_amount == 0`. Never a hard
  failure.
- **`resolve` is side-effect-free** — call it as often as the UI re-quotes; it consumes nothing.
- The effective price is **floored at zero**; a discount never yields a negative price.

## Recipe — take a coupon

Two steps, deliberately separate: *preview* at quote time, *consume* at sale-commit.

```rust
// 1. Quote: set coupon_code to preview the unlocked rule (still side-effect-free).
let query = PriceQuery { coupon_code: Some("SAVE25".into()), ..query };
let priced = resolver.resolve(&query).await?;
// priced.applied_coupon_id is Some(..) if the coupon unlocked the winning rule.

// 2. Commit: when the sale is confirmed, consume ONE use, keyed by the source document.
//    `sink` is your PromoEventSink (a LoggingSink works for single-process/tests).
let rule_id = promo_write_service
    .commit_coupon_redemption(company_id, coupon_id, "sales_order", order_id, &sink)
    .await?;   // Err(PricingError::CouponExhausted) once max_use is reached.
```

- Coupon codes are **case-insensitive** at resolve.
- `commit_coupon_redemption` is **bounded** (`used_count` can never exceed `max_use`, even under
  concurrency) and **idempotent per source**: retrying the *same* `(source_type, source_id)` is a no-op,
  not a second burn.
- **Only commit at sale-commit.** Committing at quote time burns uses on abandoned carts.

## Recipe — run loyalty

The DTOs come from `backbone_promo::application::service`; each verb takes a trailing
`&dyn PromoEventSink`.

```rust
use backbone_promo::application::service::{AccrualRequest, RedemptionRequest};

// Earn: on a settled purchase (idempotent per source document).
let earned = promo_write_service.accrue(&AccrualRequest {
    company_id, loyalty_program_id, customer_id,
    purchase_amount: dec!(250000),
    source_type: "pos_invoice".into(), source_id: invoice_id,
    at: chrono::Utc::now(),
}, &sink).await?;
// earned.points == floor(purchase_amount · collection_factor); earned.already == true on replay.

// Spend: redeem points as a discount (balance-bounded, serialized per member).
let burned = promo_write_service.redeem(&RedemptionRequest {
    company_id, loyalty_program_id, customer_id,
    points: dec!(1000),
    source_type: "sales_invoice".into(), source_id: invoice_id,
    at: chrono::Utc::now(),
}, &sink).await?;
// Apply burned.discount_value as a line/tender discount in YOUR module.
// Err(PricingError::InsufficientPoints{..}) if points exceed the signed balance.
```

- Accrual is **idempotent per source** — replaying `PosInvoicePaid` earns at most once.
- Redemption is **balance-bounded** (`Σ signed points`) and **serialized** per `(company, customer,
  program)` via an advisory lock; a replayed redemption source returns the same result without spending
  twice.
- Points are **not money** and post **no GL** — `discount_value` is an input you apply; promo records no
  cash.

## Subscribe to events

```rust
use backbone_promo::application::service::{PromoEvent, PromoEventSink};

struct MySink;
impl PromoEventSink for MySink {
    fn publish(&self, event: &PromoEvent) {
        // CouponRedeemed | LoyaltyPointsEarned | LoyaltyPointsRedeemed
        // feed analytics, notifications, or a returns claw-back. NOT GL posts.
    }
}
```

## Configuration

Module-local config lives in `config/application.yml`; hook/lifecycle config (auth TTLs, audit
retention, notification channels) in `schema/hooks/index.hook.yaml`. Feature flags (`events`, `auth`,
`grpc`, `openapi`, `validation`) are declared in [`Cargo.toml`](../Cargo.toml) and **off by default** —
enable per host as needed. gRPC/proto/GraphQL generators are disabled at the schema level today.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `resolve` returns list price unexpectedly | No rule is *applicable* — inactive, outside its window, selector/condition mismatch, or a `coupon_required` rule with no coupon presented | Check `is_active`, validity window vs `at`, `apply_on` selector, and qty/amount conditions (BR-2). |
| Coupon ignored at resolve | Coupon invalid (inactive / out of window / `used_count ≥ max_use`) or not tied to the winning rule | Verify the coupon and that its `pricing_rule_id` is the rule you expect. |
| `CouponExhausted` on commit | `max_use` reached | Expected — the cap bound. Not a bug. |
| Points didn't accrue on retry | Idempotent no-op (`already == true`) | Expected — the source already earned. |
| `InsufficientPoints` | Requested points exceed `Σ signed balance` | Show the member's balance; reduce the redemption. |
| Two rules tie on priority | Specificity tie-break: item(30) > brand/group(20) > all(10), +2 customer, +1 group, then newest, then id | Deterministic by design (BR-1). Raise `priority` to force a winner. |
| A finite points expiry seems unenforced | The expiry sweep is **parked**; only `expiry_duration_days = NULL` is fully supported today | Don't rely on short finite expiry until the sweep ships (ADR-001 parking lot). |

## What you may depend on

The stable surface — the port, the write verbs, the events, the 12 CRUD endpoints — is enumerated in the
**[extension guide](./extension-guide.md)**. Internal table shapes and `// <<< CUSTOM` blocks are **not**
a contract; read the tables through the service/events, never couple to columns.

---
Next: **[Contributing](./contributing.md)** — to land a change in promo itself.
