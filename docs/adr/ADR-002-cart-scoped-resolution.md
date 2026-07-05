# ADR-002 — Cart-scoped resolution: bundles and order-total thresholds

Status: accepted (promo half implemented) · 2026-07-05 · Tier 2 (Financials pillar; pricing input, not a GL producer)

Supersedes the "Promotional schemes" parking-lot item in ADR-001.

## Implementation status (2026-07-06)

The **promo-internal half** of this decision is implemented and green:
- `PricingRule` gained `scope` (`line` | `order`), `min_order_amount`, `stackable`; new `PromoBundle`
  + `PromoBundleComponent` entities (`match_type` = `all_of` | `any_n`) — all schema-first + regenerated.
- `PriceResolverPort::resolve_cart` (new) sits beside the unchanged `resolve`; the pipeline
  (line → bundle → order → reconcile) and the proportional penny-reconciled `allocate` live in the
  hand-authored `promo_write_service.rs`. The per-line `resolve` query now filters `scope = 'line'`
  so order rules can never leak into single-line pricing.
- Covered by `tests/cart_resolution.rs` (11 cases: order-total minimum + gate, penny reconciliation,
  all_of / any_n bundles, per-line-rule-in-cart, stacking exclusivity vs combine, group scoping,
  scope isolation). The §5 regen round-trip still holds (seam files byte-identical).

**Deferred (separate, cross-repo work):** selling / POS actually *calling* `resolve_cart` with the
whole basket. Until they do, the capability ships but no live order consumes it. Coupon-gating a
bundle is also deferred (v1 bundles fire on cart contents; see the not-in-scope list).

## Context

Today's resolver (`PromoWriteService::resolve`) prices **one line in isolation**: it takes a
`PriceQuery` (item, qty, line gross, customer, coupon, instant), collects the applicable
`PricingRule`s, and applies exactly one winning effect to that line's `unit_price`. That per-line
boundary is deliberate — it is what makes resolution deterministic, side-effect-free, and cheap to
preview (ADR-001 §2–3). It also draws a hard line around two whole classes of promotion a merchant
will ask for:

1. **Bundling — "buy A + B, get a discount."** The condition spans *distinct* lines: the discount
   fires only because A **and** B are both in the same basket. A per-line resolver can never see this
   — it prices A without knowing B exists.

2. **Cart-total minimum — "spend 500k, get 10% off / a fixed amount off."** `PricingRule.min_amount`
   exists but is checked against the **line** gross (`qty · list_price`), not the basket subtotal.
   "Spend 500k *across the cart*" is not expressible; a single 500k line is not the same requirement.

Both are currently listed as gaps. The important observation is that **they are the same gap**: each
needs the resolver to evaluate *the whole cart at once* and to produce an **order-level discount** that
must then be **allocated back across the lines**. Solving them separately would grow two allocation
mechanisms; solving them together grows one.

## Decision (proposed)

Introduce **cart-scoped resolution** as a second seam method beside the per-line one, and express both
promotions on top of it.

1. **New seam method, additive.** `PriceResolverPort` gains `resolve_cart(CartQuery) -> ResolvedCart`
   alongside the unchanged `resolve(PriceQuery) -> ResolvedPrice`. A `CartQuery` is a `Vec<CartLine>`
   (each carrying today's `PriceQuery` dimensions plus a `line_id`) with cart-wide customer/coupon/at.
   Single-line callers keep calling `resolve` and are byte-for-byte unaffected.

2. **Cart-total minimums extend `PricingRule` — no new entity.** Add `scope: line | order` (default
   `line`, today's behavior) and `min_order_amount`. A `scope=order` rule is evaluated once against the
   cart subtotal; its existing effect (`rate_or_discount`) lands as an **order-level discount**, not a
   `unit_price` change. Cheap because an order threshold is still one condition + one effect.

3. **Bundles get their own entity pair — `PromoBundle` + `PromoBundleComponent`.** A bundle is
   structurally *a set of required components* (`all_of`, or `any_n` via `required_distinct`), each a
   reusable `ApplyOn` selector with a `min_qty`. A flat rule row cannot express a set, so it earns a
   model. The bundle carries one reward effect (mirroring `PricingRule`'s effect fields).

4. **A fixed resolution pipeline preserves ADR-001's determinism.** `resolve_cart` runs:
   **(1) line pass** — today's `resolve` per line, unchanged, yielding the subtotal;
   **(2) bundle pass** — satisfiable "sets" per bundle (`min` over components of
   `floor(matched_qty / min_qty)`), reward × sets, contributing lines marked consumed;
   **(3) order pass** — `scope=order` rules gated on subtotal;
   **(4) reconcile** — cap total discount at subtotal (order never goes negative).
   Precedence stays the ADR-001 total order (priority → specificity → newest → id), extended over
   bundles. A `stackable` flag (default `false`) decides whether a firing order-rule/bundle is
   exclusive within its pass or combines.

5. **Allocation is proportional with explicit penny reconciliation.** An order/bundle discount is one
   number spread across lines **∝ line net value**, rounded half-away-from-zero (IDR, 2dp, per
   ADR-001). The rounding remainder is assigned to the **largest line** so `Σ allocations` equals the
   adjustment **exactly**. `ResolvedCart` returns both per-line results and the itemized
   `OrderAdjustment`s (source + amount + per-line allocation) so selling/POS can post a coherent,
   fully-reconciled per-line figure.

6. **Coupons and side-effects are unchanged.** A bundle may be coupon-gated (`coupon_required`); the
   coupon still points at its target and is still burned only by the separate
   `commit_coupon_redemption` — `resolve_cart` stays a pure read, exactly like `resolve`.

## Consequences

- **One new capability, not two.** Both gaps close on the same cart-scoped path and the same allocation
  primitive. The per-line resolver and all its callers/tests are untouched.
- **The genuinely new, load-bearing piece is allocation + penny reconciliation.** Spreading one
  order-level number back over lines so it ties out to the cent (and survives partial refunds
  downstream) is where the correctness risk lives — it deserves its own golden-case + integrity-probe
  suite before ship, mirroring how coupon redemption was hardened in ADR-001 §4.
- **A combination policy is now a first-class decision, not an accident.** With line + bundle + order
  discounts co-existing, "do they stack?" must be answered explicitly (`stackable`) rather than falling
  out of a single-winner sort. This is a merchant-facing business knob.
- **Cost profile changes.** `resolve` is one indexed query per line; `resolve_cart` is that plus a
  bundle-satisfiability scan over the basket. Still a read, still previewable, but no longer O(1) per
  line — worth a bounded candidate set and a note in the perf budget.

## Not in scope (deliberately parked, each with a gate)

- **Buy-X-get-Y / free-line rewards** (a bundle whose reward is a *free unit* rather than a discount on
  the matched set) — needs a synthetic reward line, not just an allocation. Gate: bundle-discount path
  shipped and proven first.
- **Progressive / marginal quantity brackets** (line 1 full price, lines 2–4 at x, 5+ at y *within one
  line*) — today's `min_qty`/`max_qty` bands apply one winning bracket to the whole line; true marginal
  pricing is a separate effect shape. Gate: merchant demand.
- **Cross-currency baskets** — allocation assumes one `currency` across the cart; mixed-currency
  reconciliation is out. Gate: multi-currency selling.
- **Showcase targeting and dynamic customer attributes** (location, purchase history) — orthogonal to
  cart scope; those are missing *dimensions*, tracked separately, and still resolve to a
  pre-computed `customer_group_id` upstream.
