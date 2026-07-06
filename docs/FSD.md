# FSD — backbone-promo

> Functional Spec. Tier 2 · Financials pillar (pricing input; posts no GL). Date: 2026-07-05.

## Entities (schema/models/*.model.yaml — SSoT)
PricingRule (selector `apply_on` + conditions + one `rate_or_discount` effect; `priority`,
validity window, `coupon_required`) · CouponCode (`code`, `pricing_rule_id`, `max_use`/`used_count`,
window) · CouponRedemption (per-source redemption ledger; partial unique `(company, coupon, source)` —
idempotency) · LoyaltyProgram (`collection_factor`, `conversion_factor`, `expiry_duration_days`, window)
· LoyaltyPointEntry (signed `points`, `entry_type`, `source_type`/`source_id`, `expiry_date`). Cross-module
ids are logical FKs (`@exclude_from_foreign_key_check`): company→organization, item/item_group/brand→
catalog, customer/customer_group→party; source docs→their owning module.

## Services (application/service — hand-authored, user_owned)
- `PromoWriteService`
  - `resolve(PriceQuery) -> ResolvedPrice` — deterministic, side-effect-free price resolution (BR-1/2/3).
  - `resolve_cart(CartQuery) -> ResolvedCart` — cart-scoped: line → bundle → order → reconcile;
    order/bundle discounts allocated ∝ remaining capacity so `Σ net_line_total == total` (BR-7); a
    buy-X-get-Y bundle emits a zero-priced `RewardLine` (BR-8). ADR-002.
  - `commit_coupon_redemption(company, coupon, source) -> rule_id` — atomic bounded use (BR-4).
  - `accrue(AccrualRequest) -> AccrualOutcome` — idempotent-per-source earn (BR-5).
  - `redeem(RedemptionRequest) -> RedemptionOutcome` — balance-bounded, serialized, idempotent burn (BR-6).
- `promo_ports` — the outward contract: `PriceQuery` / `ResolvedPrice` + `PriceResolverPort` (the trait a
  selling/POS caller holds; **zero normal Cargo edge**, a composing service wires `PromoWriteService`
  behind it via `PromoPriceResolver`). Plus `AccrualRequest` / `RedemptionRequest`.
- `promo_events` — `PromoEvent` {`CouponRedeemed`, `LoyaltyPointsEarned`, `LoyaltyPointsRedeemed`} +
  `PromoEventSink`. Domain events, not GL posts.

## HTTP surface (presentation/http)
The 12 generated CRUD endpoints per entity author rules/coupons/programs (admin surface). Resolution and
loyalty are service/seam-driven (a caller passes a `PriceQuery`); they are not generic mutation routes.

## State / determinism
- Resolution is a pure function of (rules in the DB, query) — no state machine.
- Coupon `used_count`: `0 → max_use` monotone, capped (never exceeds).
- Loyalty balance = `Σ signed points`; earned `+`, redeemed/expired `−`; never negative for a valid burn.

## Integration seams
- **Price-resolution seam (proven, marquee):** selling/POS build a `PriceQuery` per line → promo
  `resolve` → `ResolvedPrice` → they charge it. `tests/price_resolution_seam.rs` PRSEAM-1 drives the REAL
  backbone-selling write path (a live Sales Order priced from the resolved unit price). ADR-001.
- **Coupon-burn seam (proven, council 2026-07-06):** the caller keeps `ResolvedPrice.applied_coupon_id`
  from resolve and, at **sale-commit**, calls `commit_coupon_redemption(coupon, source=(sales_order|
  pos_invoice, doc_id))`. PRSEAM-3 wires it against a REAL selling order: the cap binds end-to-end (once
  `max_use` is reached, resolve refuses the coupon), and the source-keyed burn is idempotent on retry.
- **Cart-pricing seam (proven, ADR-002):** backbone-selling (`create_sales_order_priced`) and
  backbone-pos (`ring_sale_priced`) price a whole basket via their own `CartPricingPort` over
  `resolve_cart`; order/bundle discounts + free lines land on a REAL Sales Order / ticket
  (`tests/cart_selling_seam.rs` CSSEAM-1/2/3, `tests/cart_pos_seam.rs` CPSEAM-1/2). Zero normal Cargo edge.
- **Loyalty seam (proven):** promo `accrue` consumes backbone-pos's REAL `PosInvoicePaid` (PRSEAM-2),
  idempotent per source. **Outbound:** the loyalty/coupon events feed analytics / claw-back.

## Test oracle
`promo_golden_cases` (6: percentage / rate / amount-floored effects, specificity tie-break, condition
gating, coupon gate), `integrity_probes` (4: **coupon bounded**, **accrual idempotent**, **redemption
balance-bounded + idempotent**, **resolve consumes no coupon**), `price_resolution_seam` (3: PRSEAM-1
resolved price drives a real selling order; PRSEAM-2 loyalty accrues from POS's paid event; **PRSEAM-3
the coupon cap binds across a real selling commit** — council 2026-07-06) + §5 round-trip
(`scripts/price_resolution_seam_roundtrip.sh`). Plus the cart surface (ADR-002): `cart_resolution`
(13: order-minimum, penny reconciliation, all_of/any_n bundles, stacking, scope isolation, **CART-15
conservation**, **CART-16 free-line**), `cart_selling_seam` (3), `cart_pos_seam` (2). **~31 tests.**
