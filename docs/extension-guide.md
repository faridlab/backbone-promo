# Extension guide — backbone-promo

What a consuming service may depend on, and how to extend promo without forking it.

## Public / stable surface

- **The resolution port.** `PriceResolverPort` + `PriceQuery` / `ResolvedPrice`
  (`application/service/promo_ports.rs`). This is the contract selling/POS bind to. Hold the trait
  object; a composing service wires `PromoPriceResolver { service: Arc<PromoWriteService> }` behind it.
  Depending on the port keeps your crate free of a promo Cargo edge.
- **The write verbs.** `PromoWriteService::{resolve, commit_coupon_redemption, accrue, redeem}`. These
  are hand-authored and survive regen. `resolve` is safe to call for a preview (side-effect-free);
  `commit_coupon_redemption` consumes a use and belongs at sale-commit, not at quote time.
- **Domain events.** `PromoEvent` {`CouponRedeemed`, `LoyaltyPointsEarned`, `LoyaltyPointsRedeemed`}
  via `PromoEventSink` (`application/service/promo_events.rs`). Subscribe for analytics, notifications,
  or a returns claw-back. These are NOT GL posts.
- **The 12 generated CRUD endpoints** per entity (author rules/coupons/programs). Admin surface.

## Supported-but-coupled
- A sibling `*_service_custom.rs` for extra read models over the promo tables (e.g. a "best price per
  item" projection). Owns its own file; survives regen.

## Internal / not a contract
- `// <<< CUSTOM` blocks inside generated files — preserve your own edits only, not a cross-module hook.
- The `promo.*` table shapes — read them through the service/events, don't couple to columns.

## How to…
- **Price a line** (selling/POS): build a `PriceQuery` (list price, qty, item/group/brand, customer/
  group, optional coupon, `at`) → `resolve` → charge `ResolvedPrice.unit_price`. No rule → you get list.
- **Take a coupon**: `resolve` with `coupon_code` set to preview; at sale-commit call
  `commit_coupon_redemption` to consume the use (bounded by `max_use`).
- **Run loyalty**: on a settled sale, `accrue` (idempotent per source doc); to spend points, `redeem`
  (balance-bounded) and apply `discount_value` as a line/tender discount in the caller.

## Boundaries
- Promo posts no GL and owns no money of record — never route a revenue/cash post through it.
- Cross-module ids are logical FKs; promo never imports selling/POS/catalog/party.
