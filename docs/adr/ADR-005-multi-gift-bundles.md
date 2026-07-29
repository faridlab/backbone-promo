# ADR-005 — Multi-gift bundles (Shopee-style "buy A, get gift(s)")

Status: accepted · 2026-07-29 · Tier 2 (Financials pillar; pricing input, not a GL producer)

Related: [ADR-002](./ADR-002-cart-scoped-resolution.md) (cart-scoped resolution + the single-gift
free-line), [ADR-003](./ADR-003-promo-type-is-structural-not-stored.md) (a "gift" is the `quantity`
mechanic, now generalized to multiple gifts), [ADR-0008] (company RLS).

## Context

ADR-002 shipped the **single-gift** case: a `PromoBundle` with `reward_item_id` + `reward_qty` grants
`reward_qty × satisfied_sets` of ONE free item as a zero-priced `RewardLine`. The Shopee-style "gift
promo" needs **multiple distinct gifts per promotion** — buy A → get gift B **and** gift C. The single
`reward_item_id` cannot express that. This adds a `PromoBundleGift` child entity (mirroring
`PromoBundleComponent`) so a bundle carries 1..N gifts, each granted as its own `RewardLine`.

## Decision

- **New child entity `PromoBundleGift`** (collection `promo_bundle_gifts`): `bundle_id` (FK
  `PromoBundle`), `gift_item_id` (the free product), `gift_qty` (units granted per satisfied set), audit
  metadata. Mirrors `PromoBundleComponent` exactly.
- **Resolver** (`promo_cart.rs`): `BundleCand` carries `gifts: Vec<(item_id, gift_qty)>`, loaded in ONE
  round trip via `PromoBundleGiftRepository::find_for_bundles` (the same `bundle_id = ANY($2)` pattern as
  components). New `BundleCand::free_rewards` returns `(gift_item, gift_qty × sets)` per satisfied gift;
  the bundle pass emits one `RewardLine` per gift.
- **Precedence + backward compat:** in the bundle pass, gifts are evaluated **first**; if a bundle has
  gift rows they win and the legacy `reward_item_id` is ignored. A bundle with **no** gifts falls back to
  the v1 single-gift `reward_item_id` path (ADR-002) unchanged. So existing bundles + CSSEAM-3 / CPSEAM-2
  keep working — `reward_item_id` is the v1 shortcut, the gifts table is the general 1..N mechanism.
- **Consumers unchanged:** backbone-selling (`selling_order.rs`) and backbone-pos (`pos_sale.rs`) already
  iterate `for rl in &reward_lines`, appending each as a zero-priced line — so multiple gifts become
  multiple free lines with **no consumer change**.
- **RLS:** `promo_bundle_gifts` is company-fenced (ADR-0008), like every promo table.

**Invariants preserved:**

- **ADR-002's free-line semantics:** gifts are EXTRA GOODS (zero-priced lines the consumer adds), NOT an
  order discount. They never touch `remaining` / `locked` / `order_discount_total`; the basket total is
  unchanged. (`cart19` asserts `total == subtotal`.)
- **ADR-001 determinism + ADR-002 conservation:** gifts don't enter allocation, so `assert_shares_tie_out`
  (`Σ net == total`) still holds.
- `gift_qty × satisfied_sets` reuses the **same** satisfaction logic as the discount reward (shared
  `satisfied_sets`).

## Consequences

- A bundle can grant 1..N distinct gifts. The single-gift case is one gift row (or the legacy
  `reward_item_id`).
- **Additive + backward compatible:** `reward_item_id` / `reward_qty` kept; no data migration.
- `PromoBundleGift` is a full CRUD entity (the standard 12 endpoints) — sellers configure gifts through
  the API.
- **One invariant:** gifts take precedence over `reward_item_id` when both are set (documented; a bundle
  should use one or the other).

## Not in scope

- **Gift stock reservation** — gifts are zero-priced lines; inventory is the consumer/catalog concern.
- **Coupon-gating gifts** — deferred like bundle coupon-gating (ADR-002).
- **Dropping `reward_item_id`** — kept for backward compat; could deprecate once gifts are the norm.
