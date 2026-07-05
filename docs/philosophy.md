# Philosophy & motivation

> Reader: **Evaluator** · Mode: **Explanation** · Last reviewed 2026-07-05.
> Read this to decide *whether* promo belongs in your system and *why* it is shaped the way it is.

## The one problem

Every selling and point-of-sale system has to answer one question on every line it charges:

> *What should we actually charge for this?*

List price is rarely the answer. A customer has a negotiated rate, the item is on a storewide
promotion, a coupon was presented, a loyalty balance is being spent. Someone has to turn "list price
100,000, quantity 2, this customer, this coupon, right now" into "charge 80,000 a unit."

In most ERPs that *someone* is a tangle of `if`-branches buried inside the sales controller — the same
code that also posts the sale to the general ledger. **promo pulls that decision out into its own
module.** It owns pricing rules, coupons, and loyalty, and it answers exactly that one question.

## The north star: posts no GL, owns no money of record

This is the single constraint every other decision follows from:

> A resolved price is an **input** to selling/POS, which own the revenue post. Loyalty points are a
> subledger of their own — never an accounting entry.

Why this matters, concretely:

- **A merchant can run promo, swap it, or ignore it without touching the books.** Turn promo off and
  every line simply resolves to its list price. No hard failure, no orphaned journal entries.
- **There is no double source of truth for money.** Promo never records revenue or cash; it never
  competes with the accounting module for "what was really charged." It advises; selling decides.
- **The blast radius of a promo bug is a wrong price, not a corrupted ledger.** A pricing mistake is
  visible and recoverable at the line; it can never silently unbalance the books.

If you need promo to post journal entries, promo is the wrong module — that is billing/accounting's
job, and deliberately so.

## The three things it does

1. **Resolve a price** — deterministically pick the winning pricing rule for a line and return the
   effective unit price. This is a *read*: side-effect-free, safe to call as many times as a UI wants.
2. **Bound a coupon** — a coupon unlocks a gated rule and can be consumed at most `max_use` times,
   even under concurrency, and at most once per source document (idempotent).
3. **Run the loyalty ledger** — earn points on a settled purchase (idempotent per source), redeem them
   as a discount (balance-bounded, serialized per member).

Each is detailed as a business rule in the [BRD](./BRD.md) (BR-1…BR-6) and proven numerically in the
[golden cases](./business-flows/golden-cases.md).

## Design commitments (and what each buys you)

| Commitment | What it means | What it buys |
|-----------|---------------|--------------|
| **Determinism** | Resolution is a total order: priority → specificity → newest → id. Same inputs, same rule, always. | A price quote is reproducible; the UI can re-quote freely; support can explain any price. |
| **Side-effect-free reads** | `resolve` never mutates. Coupons are consumed by a *separate* commit at sale time. | You can preview a price without burning a coupon. Quoting is not redemption. |
| **Idempotency per source** | Every write keys on the source document. Replays no-op. | At-least-once delivery is safe: a dropped ack + retry never double-burns a coupon or double-earns points. |
| **Zero normal Cargo edge** | Callers depend on a *port* trait, not on promo the crate. | Selling/POS stay decoupled; promo is genuinely swappable, not a compile-time dependency. |
| **Region-neutral money** | IDR, 2dp, half-away-from-zero. No tax logic here. | Tax stays billing/tax's job; promo doesn't fork per jurisdiction. |

## Non-goals (stated honestly)

promo deliberately does **not** do these today. Each is parked with a reason, not forgotten — see the
[PRD non-goals](./PRD.md#non-goals--deferred-with-reason) and [ADR-001 parking lot](./adr/ADR-001-pricing-boundary-and-resolution-seam.md#parking-lot-each-with-a-gate):

- **Promotional schemes** (buy-X-get-Y, tiered slabs). ERPNext models these as *generators* of pricing
  rules; the free-line/tier mechanics are a materially larger surface. Parked until a merchant needs it.
- **Shipping rules** — a logistics cost band, closer to delivery than pricing.
- **Multi-tier loyalty resolution** — the schema carries the `program_type`, but tier *selection* is
  deferred.
- **Points-expiry sweep** — `expiry_date` is stamped on accrual, but nothing yet burns lapsed points.
  **Supported config today is `expiry_duration_days = NULL` (non-expiring points).** A short finite
  expiry is accepted by the schema but not yet enforced — do not rely on it until the sweep job ships.

Stating these up front is the point: promo is trustworthy precisely because its edges are drawn and
labelled, not blurred.

## Where promo sits

Tier 2, Financials pillar — but the *input* side of it, not a GL producer. It is asked by the
transactional modules (selling, POS); it depends on none of them. See
[Architecture › Context](./architecture.md#level-1--system-context) for the full picture.

---
Next: **[Background & prior art](./background.md)** — why the model is borrowed from ERPNext but the
coupling is not.
