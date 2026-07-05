# Background & prior art

> Reader: **Evaluator** · Mode: **Explanation** · Last reviewed 2026-07-05.
> Read this to understand what came before promo, what it borrows, and what it deliberately rejects.

## The prior art: ERPNext pricing

promo's domain model is drawn directly from [ERPNext](https://erpnext.com), the open-source ERP whose
pricing engine is the most battle-tested SMB reference available. The vocabulary is intentionally
familiar to anyone who has run ERPNext:

| ERPNext concept | promo equivalent | What we kept |
|-----------------|------------------|--------------|
| **Pricing Rule** | `PricingRule` | Selector (`apply_on`: item / group / brand / all) + conditions (customer, qty, amount, date) + one effect (rate / percent / amount), resolved by priority. |
| **Coupon Code** | `CouponCode` | A code gating a pricing rule, with a redemption cap (`max_use`). |
| **Loyalty Program** | `LoyaltyProgram` | Earn rate (`collection_factor`) and burn rate (`conversion_factor`). |
| **Loyalty Point Entry** | `LoyaltyPointEntry` | A signed ledger of points movements tied to source documents. |

If you know how ERPNext prices a sales order, you already know how promo resolves a line. That is by
design — the model is proven, and familiarity lowers the adoption cost.

## What we rejected, and why

Borrowing the *model* is not the same as borrowing the *architecture*. ERPNext bakes the pricing
decision into `get_item_details` inside the selling and buying controllers — the very same code path
that also posts the sale to the general ledger. That coupling is the thing promo exists to avoid.

| ERPNext does… | promo does instead… | Because |
|---------------|--------------------|---------|
| Pricing logic **inside** the selling controller (`get_item_details`) | Pricing in a **separate module** the controller *asks* | A merchant can swap or disable pricing without touching selling or the GL. |
| The same code resolves price **and** posts revenue | promo **posts no GL**; selling owns the revenue post | A pricing bug is a wrong price, never a corrupted ledger. |
| Coupon `used` count bumped inline, best-effort | Bounded, **idempotent-per-source** redemption ledger | At-least-once retries can't double-burn a coupon. |
| Loyalty points entangled with the invoice | A standalone **signed subledger**, idempotent on earn | Points are not money; replaying a paid event never double-earns. |
| Pricing coupled to tax and currency handling | **Region-neutral** (IDR, no tax) | Tax is billing/tax's job; promo doesn't fork per jurisdiction. |

We credit ERPNext for the domain and reject only the monolith coupling. This is not a strawman —
ERPNext's inline model is a reasonable choice for a single deployable monolith. promo targets a
different shape: independent modules composed at the seam, where decoupling is worth the indirection.

## Why not the obvious alternatives

- **A shared pricing library, linked into selling.** Rejected: a library is a compile-time dependency,
  so selling and promo version together and can't be swapped independently. promo is consumed through a
  *port trait* with **zero normal Cargo edge** instead — see [ADR-001](./adr/ADR-001-pricing-boundary-and-resolution-seam.md).
- **A pricing microservice over HTTP/gRPC.** Rejected for now: the seam is a synchronous per-line read
  on the hot path; a network hop per line is latency the SMB target doesn't need. The port abstraction
  leaves that door open (a remote implementation could sit behind the same trait) without paying for it
  today.
- **Rules encoded as data in the selling module.** Rejected: that just moves the tangle. The pricing
  *decision* — specificity tie-breaks, coupon gating, loyalty math — is real domain logic that deserves
  its own bounded context and its own tests.

## The lineage in this repo

promo was extracted from the `bersihir` application (the `Cargo.toml` description still records the
origin). The v1 module carried advertisements, promo campaigns, and banners; **v2 (schema version 2)
replaced that entirely** with the pricing/coupon/loyalty domain documented here. If you find a doc or
comment mentioning advertisements or banners, it is stale v1 residue — the code and schema are the
current truth.

---
Next: **[Technology & the "why"](./technology.md)** — the stack choices behind the module.
