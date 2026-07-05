# BRD — backbone-promo

> Business Requirements & Rules. Tier 2 · Financials pillar (pricing input). Date: 2026-07-05.
> Pairs with `docs/business-flows/golden-cases.md`.

## Documents
PricingRule (the resolver's unit) · CouponCode (a capped code that unlocks a gated rule) ·
CouponRedemption (per-source redemption ledger — idempotency) · LoyaltyProgram (earn/burn rates) ·
LoyaltyPointEntry (the signed points ledger).

## Business rules

**BR-1 (deterministic resolution — the marquee read).** `resolve(query)` returns the effective unit
price for a line. It picks among **applicable** rules by **priority DESC**, then **specificity DESC**
(item > brand/group > storewide; a customer/group match adds specificity), then **newest** (valid_from
DESC), then id — a total order, so the same inputs always resolve the same rule. No applicable rule →
**passthrough** (charge list price). Resolve is **side-effect-free**: previewing a price never consumes
a coupon (BR-4).

**BR-2 (applicability).** A rule applies iff it is active + in its validity window AND its selector
matches the line (apply_on item/item_group/brand, or `all`) AND every set condition holds: customer /
customer_group (null = don't constrain), `min_qty ≤ qty ≤ max_qty`, `min_amount ≤ qty·list`.

**BR-3 (the single effect).** Exactly one of: `rate` (absolute unit price), `discount_percentage`
(percent off list, clamped ≤ 100), `discount_amount` (fixed per-unit). The effective unit price is
**floored at zero** — a discount never produces a negative price. Money is IDR, 2dp, half-away-from-zero.

**BR-4 (coupon gate + bounded, idempotent redemption).** A `coupon_required` rule stays dormant until a
valid coupon (active, in-window, `used_count < max_use`) for *that* rule is presented; `resolve` then
applies it and reports the coupon. **Consuming** a use is a separate write, `commit_coupon_redemption`,
called when the sale commits. It is **bounded** — a guarded increment makes `used_count` impossible to
advance past `max_use`, even under concurrent redemptions (→ `coupon_exhausted`) — AND **idempotent per
source**: a `CouponRedemption` ledger row keyed `(company, coupon, source_type, source_id)` records which
document consumed each use, so a retry of the *same* sale (a dropped ack, at-least-once delivery) finds
the existing row and returns the same result **without a second burn** (council 2026-07-06 maturity fix).
The ledger claim + counter bump commit in one transaction. Previewing a price never consumes.

**BR-5 (loyalty accrual — idempotent per source).** `accrue` earns
`floor(purchase_amount · collection_factor)` points against an active program, writing one
`earned` LoyaltyPointEntry keyed by `(company, source_type, source_id, earned)`. A replayed paid event
(e.g. POS's `PosInvoicePaid`) earns **at most once** — the partial unique key no-ops the duplicate.
`expiry_date` is stamped when the program sets `expiry_duration_days`.

**BR-6 (loyalty redemption — balance-bounded, serialized, idempotent).** `redeem` spends points as a
discount worth `points · conversion_factor`. It serializes per `(company, customer, program)` with an
advisory lock, rejects a request exceeding the member's **signed balance** (`Σ points`) as
`insufficient_points`, and writes a negative `redeemed` entry. A replayed redemption source returns the
same result without spending twice.

## Events
`CouponRedeemed` (a use was consumed) · `LoyaltyPointsEarned` (accrual) · `LoyaltyPointsRedeemed` (burn).
None is a GL post — promo emits domain events, not accounting envelopes.

## Deferred (with reason)
Promotional schemes (buy-X-get-Y / tiers), shipping rules, multi-tier loyalty resolution, the points
expiry sweep job. See PRD non-goals. (Per-source coupon idempotency shipped in the maturity pass — BR-4.)
