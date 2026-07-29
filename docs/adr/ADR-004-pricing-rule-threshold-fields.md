# ADR-004 — PricingRule threshold-shape fields: `min_order_qty` + `discount_upto`

Status: accepted · 2026-07-29 · Tier 2 (Financials pillar; pricing input, not a GL producer)

Related: [ADR-001](./ADR-001-pricing-boundary-and-resolution-seam.md) (resolution seam + determinism),
[ADR-002](./ADR-002-cart-scoped-resolution.md) (cart-scoped resolution),
[ADR-003](./ADR-003-promo-type-is-structural-not-stored.md) (promo-type council).
Closes two of the capability gaps ADR-003's verification surfaced.

## Context

ADR-003's council verified all four promo *mechanics* (standard / event / minimum / quantity) already
resolve structurally — but that verification surfaced three orthogonal *capability* gaps the model
could not express:

1. A cart-wide **item-count** floor for `scope=order` rules — only the `min_order_amount` Rp floor
   existed. "Spend 500k" was expressible; "buy ≥ 10 items cart-wide" was not.
2. An Rp **ceiling** on a rule's discount — `discount_percentage` clamped to ≤ 100% of the matched
   value but had no separate max-Rp cap ("10% off, max 50k" was not expressible).
3. Targeting by `showcase` / `bundle` — a missing `ApplyOn` dimension.

This ADR closes gaps **1** and **2**. Gap 3 stays parked (see *Not in scope*). Both fields are
additive and nullable (`null` = feature off), so existing behavior is byte-for-byte unchanged.

## Decision

Add two nullable fields to `PricingRule` (source of truth: `schema/models/pricing_rule.model.yaml`):

- **`min_order_qty`** (`decimal?`, 18,4) — minimum total item quantity across the cart for a
  `scope=order` rule to fire (ignored when `scope=line`). Gated in SQL exactly like
  `min_order_amount`: `find_order_candidates` takes the cart's `total_qty` (`Σ` line quantities) and
  adds `AND (min_order_qty IS NULL OR min_order_qty <= :total_qty)`.
- **`discount_upto`** (`decimal?`, 18,2) — optional Rp ceiling on the discount a rule grants; the
  resolved discount is clamped to at most this (`null` = no cap). Applies to `discount_percentage`
  and `discount_amount`; not meaningful for `rate` (a price override, not a discount).

**The non-obvious part — how `discount_upto` clamps at each scope:**

1. **Order scope** (`scope=order`): a direct clamp. `discount_on()` computes the order discount, then
   returns `min(raw, cap)`. One number, one clamp.
2. **Line scope** (`scope=line`): the per-line `resolve` returns a *per-unit* price
   (`ResolvedPrice.unit_price` + per-unit `discount_amount`). `discount_upto` is a per-**line** Rp
   ceiling, so `resolve` caps the line's **total** discount (per-unit discount × qty) at the cap,
   then back-computes the per-unit discount `= capped_total / qty` (money-rounded). This keeps
   `ResolvedPrice` per-unit while honoring a ceiling on the whole line. The cap can only *reduce* the
   discount, never increase it.

**Invariants preserved:**

- **ADR-001's deterministic total order is unchanged.** `discount_upto` clamps the *winning* rule's
  magnitude only; `min_order_qty` is a structural applicability gate (like `min_order_amount`), not a
  sort key. The clamp sits *after* the winner is picked, so it can never change which rule wins.
- **ADR-002's conservation holds.** The cap is applied before allocation, so `Σ net_line_total ==
  total` still holds (proven by `cart17` / `cart18` via `assert_shares_tie_out`).
- **The classification never crosses the resolution contract.** Both fields are `PricingRule` fields;
  neither appears in `ResolvedPrice` / `ResolvedCart` or the `PriceResolverPort` signature.
- **Stacking policy unchanged.** Within-pass stays exclusive by default (`stackable = false`);
  `discount_upto` bounds each promo's exposure without altering the combination rules.

## Consequences

- **Backward compatible.** Both nullable, `null = off`; no data backfill; existing rules behave
  identically.
- **`discount_upto` lives on `PricingRule` only** — `PromoBundle` rewards are uncapped for now.
- **One invariant to maintain:** `discount_upto` must never influence winner selection or the total
  order — only the winning rule's discount magnitude. Enforced by where the clamp sits (post-pick).
- **Tested** by `cart17` (`min_order_qty` gate: below floor → no fire; above → fires) and `cart18`
  (`discount_upto` cap: 10% of 1,000,000 capped at 50,000), both with conservation asserts.

## Not in scope (each with a gate)

- **`showcase` / `bundle` as `ApplyOn` target dimensions (gap 3)** — a *targeting* question, not a
  *threshold* question; still parked per ADR-003. Gate: confirming `showcase` is a catalog concept
  backbone-promo can reference without re-introducing a cross-module leak.
- **A `discount_upto` on `PromoBundle` rewards** — only `PricingRule` carries the cap today. Gate:
  merchant demand for capping bundle rewards.
- **A stored, non-branching `promo_type`** — rejected by ADR-003; these fields are capability
  additions, not a type taxonomy.
