# Glossary — ubiquitous language

> Reader: **All** · Mode: **Reference** · Last reviewed 2026-07-05.
> One term, one meaning. Every page in this handbook uses these definitions. If a term you need is
> missing, add it here rather than coining a synonym elsewhere.

## Domain entities

**PricingRule** — the unit the resolver evaluates. A rule targets a *selector* (`apply_on`: item /
item_group / brand / all) under optional customer, quantity, amount, and date conditions, and carries
*exactly one* effect (rate / discount_percentage / discount_amount). Source of truth:
`schema/models/pricing_rule.model.yaml`.

**CouponCode** — a redeemable code that unlocks a `coupon_required` PricingRule. Bounded by `max_use`;
carries its own validity window independent of the rule's. `schema/models/coupon_code.model.yaml`.

**CouponRedemption** — the per-source redemption ledger. One row per `(company, coupon, source_type,
source_id)` recording *which document* consumed each coupon use — the idempotency record.
`schema/models/coupon_redemption.model.yaml`.

**LoyaltyProgram** — the earn/burn rates for a loyalty scheme: `collection_factor` (points per 1
currency spent) and `conversion_factor` (currency value per point on redemption).
`schema/models/loyalty_program.model.yaml`.

**LoyaltyPointEntry** — one *signed* movement in the points ledger: `earned` (+), `redeemed` (−),
`expired` (−), tied to a source document. Same file as LoyaltyProgram.

## Core verbs (the write path)

**resolve** — the marquee **read**. Given a `PriceQuery`, deterministically pick the winning PricingRule
and return a `ResolvedPrice`. **Side-effect-free**: never mutates, never consumes a coupon.

**commit_coupon_redemption** — the bounded **write**. At sale-commit, advance a coupon's `used_count`
atomically (capped at `max_use`) and record the `CouponRedemption` row. Idempotent per source.

**accrue** — the loyalty **earn** leg. Write one `earned` LoyaltyPointEntry for a settled purchase.
Idempotent per source document.

**redeem** — the loyalty **burn** leg. Spend points as a discount worth `points · conversion_factor`.
Balance-bounded and serialized per member.

## Key concepts

**resolution / the resolver** — the algorithm inside `resolve`. Ranks applicable rules by a **total
order**: priority DESC → specificity DESC → newest (`valid_from` DESC) → id. Same inputs always pick the
same rule.

**applicable / applicability** — a rule *applies* iff it is active, within its validity window, its
selector matches the line, and every set condition (customer, customer_group, min/max qty, min amount)
holds. (BR-2.)

**specificity** — the tie-break when rules share a priority: item (30) > brand / item_group (20) > all
(10); +2 for a customer match, +1 for a customer_group match. A more targeted rule wins.

**passthrough** — the result when **no** rule applies: charge the list price
(`ResolvedPrice::passthrough`). `unit_price == list_price`, `discount_amount == 0`.

**effect** — the single thing a rule does: `rate` (absolute unit-price override),
`discount_percentage` (percent off list, clamped ≤ 100), or `discount_amount` (fixed per-unit off). The
effective price is **floored at zero**.

**coupon gate** — a rule with `coupon_required = true` stays dormant until a valid coupon for *that* rule
is presented at resolve. (BR-4.)

**bounded** — an operation that cannot exceed its cap under concurrency. Coupon redemption is bounded:
`used_count ≤ max_use` always (guarded increment).

**idempotent per source** — a write keyed by the source document `(…, source_type, source_id, …)` so a
replay of the *same* source is a no-op, not a repeat. Applies to coupon burn and loyalty accrual/redeem.

**balance-bounded** — a redemption may not exceed the member's **signed balance** `Σ points`
(earned + / redeemed − / expired −). Over-redemption → `insufficient_points`.

**source document** — the consuming/producing business document in another module (e.g. a
`sales_order`, `pos_invoice`, `sales_invoice`), identified by `source_type` + `source_id`. The
idempotency key.

## Boundary & integration terms

**port** — `PriceResolverPort`, the trait a selling/POS caller holds. The caller depends on the *trait*,
not the promo crate. A composing service wires `PromoWriteService` behind it (`PromoPriceResolver`).

**zero normal Cargo edge** — the structural guarantee that the shipped promo library has no normal
(non-dev) dependency on selling/POS, and vice versa. Verified by
`cargo tree -e normal -i backbone-selling` being empty.

**seam** — a proven integration point between promo and another module: the price-resolution seam, the
coupon-burn seam, the loyalty-accrual seam. Each is dependency-inverted and tested against the *real*
counterpart module (as a dev-dependency).

**logical FK** — a cross-module id stored without a database foreign-key constraint
(`@exclude_from_foreign_key_check`): company → organization, item/group/brand → catalog,
customer/group → party, source docs → their owning module. No DB constraint crosses a module boundary.

**posts no GL** — promo never writes a general-ledger / accounting entry. A resolved price is an *input*
to selling/POS; loyalty points are a *subledger*, not money of record.

**subledger** — the loyalty points ledger. A record of points movements that is **not** an accounting
ledger; points are not money.

## Framework terms (backbone `module` type)

**SSoT (single source of truth)** — `schema/models/*.model.yaml`. Generated code is downstream of it.

**regeneration / regen** — the `metaphor-schema` codegen pass that rewrites generated files from the
schema. Preserves only `// <<< CUSTOM … // END CUSTOM` blocks and hand-owned files.

**`// <<< CUSTOM` marker** — the regen-safe region inside a generated file. Edits between the markers
survive; edits outside are overwritten.

**hand-owned / user-owned file** — a source file with no generated twin, authored not generated:
`promo_write_service.rs`, `promo_ports.rs`, `promo_events.rs`. Survives regen wholesale.

**BackboneCrudHandler** — the framework wiring that gives every entity the 12 standard CRUD endpoints
without hand-written routes.

**GenericCrudService / GenericCrudRepository** — the framework generics that a `*_service.rs` type alias
and a `*_repository.rs` newtype specialise per entity.

## Money & units

**IDR / money** — currency is Indonesian Rupiah, 2 decimal places, rounded **half-away-from-zero**.
promo is region-neutral: **no tax** logic (that is billing/tax's job).

**points** — whole numbers, floored on accrual (`floor(purchase_amount · collection_factor)`). Points
are **not money** and post no GL.

## Test-oracle labels

**PGC-n** — a Promo Golden Case (`tests/promo_golden_cases.rs`): a resolution expectation.
**IP-n** — an Integrity Probe (`tests/integrity_probes.rs`): an invariant (bounding, idempotency, balance).
**PRSEAM-n** — a Price-Resolution Seam test (`tests/price_resolution_seam.rs`): a cross-module seam
against the real selling/POS write paths.
**BR-n** — a Business Rule in the [BRD](./BRD.md) (BR-1…BR-6).

See [business-flows/golden-cases.md](./business-flows/golden-cases.md) for the numeric values behind
each label.
