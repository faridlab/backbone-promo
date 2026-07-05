# ADR-001 — Promo's boundary and the price-resolution seam

Status: accepted · 2026-07-05 · Tier 2 (Financials pillar; pricing input, not a GL producer)

## Context

Selling and POS were built taking `unit_price`/`discount` as **given inputs**. Something has to decide
those numbers from pricing rules, coupons, and loyalty. ERPNext bakes that decision into
`get_item_details` inside the selling/buying controllers — the same monolith coupling the GL-posting
contract exists to avoid. We want pricing to be a **separate, swappable module** the transactional
modules ask, not code they contain.

## Decision

1. **Promo posts no GL and owns no money of record.** A resolved price is an *input* to selling/POS
   (which own the revenue post). Loyalty points are a subledger of their own, never an accounting entry.
   This is what lets a merchant run/swap/ignore promo without touching the books.

2. **Resolution is a read the caller pulls, through a port.** Selling/POS build a `PriceQuery` per line
   and call `PriceResolverPort::resolve`. The caller depends only on the trait; a composing service
   wires `PromoWriteService` behind it (`PromoPriceResolver`). **Zero normal Cargo edge** — exactly the
   POS↔billing/payment pattern. Promo does not depend on, nor call into, selling/POS.

3. **Resolution is deterministic and side-effect-free.** A total order (priority → specificity → newest
   → id) means the same inputs always pick the same rule; previewing a price **never** mutates state
   (coupons are consumed by a separate commit at sale time). This makes a price quote safe to compute as
   many times as the UI likes.

4. **Coupons are bounded AND idempotent per source** (maturity council, 2026-07-06).
   `commit_coupon_redemption` is an atomic guarded increment that cannot exceed `max_use`, and a
   `CouponRedemption` ledger keyed `(company, coupon, source_type, source_id)` (partial unique +
   `ON CONFLICT DO NOTHING` gating the counter bump, one transaction) makes a retry of the *same* sale a
   no-op instead of a second burn — the same idempotency pattern the loyalty accrual leg uses. Without
   it, a dropped ack + retry burned two uses of one coupon (proven by revert: IP-1's same-source case).

5. **Loyalty is a signed ledger, idempotent on the earn leg.** Accrual keys on
   `(company, source_type, source_id, earned)` so replaying a paid event never double-earns; redemption
   serializes per member and is balance-bounded.

## Consequences

- Selling/POS stay the owners of what they charge; promo is the advisor. A missing/disabled promo simply
  resolves to list price — no hard failure.
- The seam is proven end-to-end (`tests/price_resolution_seam.rs`) and survives a full regen (§5).

## Parking lot (each with a gate)
- **Promotional schemes** (buy-X-get-Y / tiers) — free-line + tier mechanics; gate: merchant demand.
- **Points expiry sweep** — `expiry_date` is stamped on accrual, but the redemption balance (`Σ points`)
  does not yet exclude lapsed points, and nothing writes `expired` entries. **Supported config today is
  `expiry_duration_days = NULL` (non-expiring points)**; a *finite* expiry is accepted by the schema but
  not yet enforced, so a merchant who sets a short window (e.g. "points expire end of month") would let
  customers spend lapsed points until the sweep ships. Gate: a scheduled burn job that writes `expired`
  entries via FIFO lot consumption (a read-time `WHERE` clause is insufficient — earned lots can be
  partially redeemed) and, since it is a second balance-decreasing path, takes the redemption
  advisory-lock key. Flagged by the completeness council (2026-07-06) as legitimately parked for the
  NULL/long-window case, owed before any short finite expiry goes live.
- **Shipping rules**, **multi-tier loyalty resolution** — parked (see PRD non-goals).
- **Specificity vs a customer-scoped storewide rule** — a customer-scoped `all` rule loses to a generic
  item rule (specificity 12 < 30). Deterministic and by design (priority is the primary control); a
  business-judgment knob, not an invariant break.
