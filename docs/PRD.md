# PRD — backbone-promo

> Product Requirements. Tier 2 · Financials pillar (the pricing input, not a GL producer). Date: 2026-07-05.

## Why this module exists

Selling and POS currently take `unit_price` and `discount` as **given inputs** — someone, somewhere,
decides the number. `backbone-promo` is that someone. It owns pricing rules, coupons, and loyalty, and
answers one question the transactional modules ask on every line: *what should we actually charge?*

It **posts no GL and owns no money of record.** A resolved price is an input to selling/POS (which own
the revenue post); loyalty points are a subledger of their own, never an accounting entry. This keeps
promo composable — a merchant can run it, swap it, or ignore it without touching the books.

## Scope (Indonesia-first, SMB)

- **Pricing rules** — conditional price/discount by item / group / brand / storewide, under optional
  customer, quantity, amount, and date conditions. One effect per rule: rate override, percent off, or
  fixed amount off. Deterministic resolution (highest priority, most specific wins).
- **Coupons** — a code that unlocks a coupon-gated rule, with a hard redemption cap (`max_use`).
- **Loyalty** — earn points on a settled purchase (idempotent per source), redeem them as a discount
  (balance-bounded). Region-neutral IDR; PPN stays billing/tax's job.

## Non-goals / deferred (with reason)

- **Promotional schemes** (buy-X-get-Y, tiered slabs) — ERPNext models these as *generators* of pricing
  rules; the free-line / tier mechanics are a materially larger surface. Parked until a merchant needs it.
- **Shipping rules** — a logistics cost band, closer to delivery than pricing. Parked.
- **Tier resolution** for multi-tier loyalty — the schema carries the type; tier selection is deferred.
- **Points expiry sweep** — `expiry_date` is stamped on accrual; the batch that burns lapsed points is
  a downstream job (parked with a gate).

## Success criteria

- Selling/POS resolve a line's price through promo and charge exactly what promo returns (proven:
  `tests/price_resolution_seam.rs`).
- A coupon can never be redeemed past its cap; a purchase earns loyalty points at most once.
- The whole seam is a dev-dependency edge only — the shipped library has **zero normal Cargo edges**.
