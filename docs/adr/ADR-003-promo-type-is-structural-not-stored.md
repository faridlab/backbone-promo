# ADR-003 — Promo "type" is structural, not a stored classification

Status: accepted · 2026-07-29 · Tier 2 (Financials pillar; pricing input, not a GL producer)

Related: [ADR-001](./ADR-001-pricing-boundary-and-resolution-seam.md) (price-resolution seam + determinism),
[ADR-002](./ADR-002-cart-scoped-resolution.md) (cart-scoped resolution + bundles).
Council: [`docs/council/2026-07-29-module-backbone-promo-bounded-context-cleanliness.md`](../council/2026-07-29-module-backbone-promo-bounded-context-cleanliness.md).

## Context

A reference PHP/Laravel commerce service models every promotion as one row in a single
`promotions` table with a `type` enum — `standard | minimum | quantity | event` — that a `switch` in its
`PromotionService::evaluate()` dispatches on (`minimum` → threshold check; `quantity` → free-items math;
else → flat discount). The question put to the council: should backbone-promo adopt that `type` concept —
as a literal enum + switch (Opt-A), a structural mapping with no stored tag (Opt-B), or a new `Campaign`
aggregate carrying a non-branching classification (Opt-C)?

The decisive evidence came from probing **where that service's `type` is actually read**. Outside the
validation path it has exactly three readers:

1. `PromotionService::evaluate()` — `type === 'minimum'` (the switch)
2. `PromotionService::evaluate()` — `type === 'quantity'` (the switch)
3. `PromotionUsageReportService` — `'type' => $promo->type` (one usage report)

No Blade view, admin filter, or `GROUP BY type` consumes it. So `type` is an **execution-mode
discriminator** bolted onto a single-table model — it compresses three orthogonal axes (*has-a-window*,
*has-a-threshold*, *is-a-bundle*) into one tag — not a domain classification. Three of the four "types"
(`standard` / `event` / `minimum`) are the *same* `PricingRule` with different optional fields set; only
`quantity` is structurally distinct, and backbone already gave it its own entity (`PromoBundle`, ADR-002).

## Decision

**Do not adopt a `promo_type` field, enum, or `Campaign` aggregate.** Record how each `type` value maps
onto backbone's structural model, as documentation only. backbone already expresses every "type"
structurally:

| commerce-service `type` | backbone structural home (existing) |
|---|---|
| `standard` (flat discount on a target) | `PricingRule`, `scope = line` |
| `event` (time-windowed discount) | `PricingRule` with a `valid_from` / `valid_to` window |
| `minimum` (discount once a cart/spend threshold clears) | `PricingRule`, `scope = order`, `min_order_amount > 0` |
| `quantity` ("buy X get Y free") | `PromoBundle` with a free-item reward (`reward_item_id` + `reward_qty`) |

Invariants this preserves:

1. **ADR-001's deterministic total order stays the single execution path.** A stored `type` read by
   resolution would be a *second* path — `type = minimum` would have to be kept consistent with
   `scope = order + min_order_amount`, a duplicate source of truth that either bypasses the resolver
   (breaking determinism) or redundantly recomputes what the structural model already derives. Rejected
   (Opt-A).
2. **The classification never crosses the resolution contract.** `promo_type` shall not appear in
   `ResolvedPrice` / `ResolvedCart` or the `PriceResolverPort` signature — selling/POS depend on the
   *price* promise, not promo's internal vocabulary.
3. **No new normal Cargo edge, no new entity lifecycle.** A `Campaign` aggregate (Opt-C) is unjustified
   while no consumer needs identity+lifecycle for a classification that is fully computable from existing
   fields; a Campaign referencing rules by internal struct shape would also *extend* the existing re-export
   surface rather than seal it.

The ubiquitous-language umbrella a merchant uses ("a promotion") is captured informally in the
[glossary](../glossary.md) `Promotion` entry, which carries this same mapping.

## Consequences

- **Zero schema, migration, or code change.** `pricing_rule.model.yaml`, `promo_bundle.model.yaml`,
  `ResolvedPrice`, `resolve` / `resolve_cart`, and the cart pipeline are untouched. Fully reversible — a
  stored tag or aggregate can be added later with **no data backfill**, because the classification is
  derivable on demand.
- **The deferred cost is small and backfill-free.** When a backbone-side consumer (an admin "promotions"
  index, or a usage-by-kind report) appears, the classification is a **derived view / computed column**
  over existing fields — never a stored duplicate.
- **One doc invariant to maintain:** the mapping table must stay accurate if `PricingRule` / `PromoBundle`
  axes change. Low risk — those axes are SSoT-stable (ADR-001 / ADR-002).

## Not in scope (each with a gate)

- **`showcase` / `bundle` as `ApplyOn` target dimensions** — that service targets by `showcase` and
  `bundle`; backbone's `ApplyOn` is `item | item_group | brand | all`. This is a real **capability gap**,
  but it is a *targeting* question, not a *type/classification* one, so it is tracked separately. Gate:
  confirming `showcase` is a catalog concept backbone-promo can reference without re-introducing a
  cross-module leak (it may belong as a logical FK, not an `ApplyOn` variant). Already noted in ADR-002's
  parking lot.
- **A `Campaign` / `Promo` aggregate** — revisit only when a consumer needs a single addressable promotion
  with its own identity + lifecycle spanning `PricingRule` + `PromoBundle` (e.g. one "create promotion"
  admin workflow wrapping a rule, a window, and a coupon). No such consumer today.
- **A stored, non-branching `promo_type` column** — only if a consumer must *persist* a classification that
  is not derivable from existing fields; prefer a view until then.
