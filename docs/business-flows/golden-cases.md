# Promo — Golden Cases (the numeric oracle)

Mirrors `tests/promo_golden_cases.rs`, `tests/integrity_probes.rs`, and the cross-module seam in
`tests/price_resolution_seam.rs`. Money is exact IDR (2dp, half-away-from-zero). Points are whole.

## Resolution (`tests/promo_golden_cases.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **PGC-1** | 10% rule on item; list 100,000 × 2 | unit `90,000`; discount `10,000/unit`; rule applied. |
| **PGC-2** | rate override 75,000; list 100,000 | unit `75,000`; discount `25,000`. |
| **PGC-3** | amount discount 150,000 on list 100,000 | unit `0` (floored); discount `100,000` (never negative). |
| **PGC-4** | two rules, both priority 5: storewide `all` 5% vs item 20% | the **item** rule wins (specificity); unit `80,000`. |
| **PGC-5** | rule needs qty ≥ 10 | qty 3 → passthrough `100,000` (no rule); qty 10 → `70,000`. |
| **PGC-6** | coupon-gated 25% rule + coupon `SAVE25` | no coupon → passthrough; coupon (case-insensitive) → `75,000`, coupon reported. |

## Integrity probes (`tests/integrity_probes.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **IP-1** | (a) `max_use=1`, two different sales; (b) `max_use=2`, the SAME sale twice | (a) 1st ok; 2nd `coupon_exhausted`; `used_count`=`1` (bounded). (b) both return the same rule; `used_count`=`1` — **the same source consumes one use, not two** (idempotent per source; council 2026-07-06). |
| **IP-2** | accrue the same source twice (0.01 pts/spend, 250,000) | 1st earns `2,500`; 2nd is a no-op (`already`); balance stays `2,500`. |
| **IP-3** | balance 2,500; redeem 3,000 then 1,000, then replay | over-redeem → `insufficient_points` (balance untouched); redeem 1,000 → discount `100,000`, balance `1,500`; replay same source → idempotent, balance `1,500`. |
| **IP-4** | resolve a coupon-gated line 3× | each returns the discount; `used_count` stays `0` — **resolve consumes no coupon**. |

## Price-resolution seam — promo ↔ selling ↔ POS (`tests/price_resolution_seam.rs` + `scripts/price_resolution_seam_roundtrip.sh`)

| Case | Input | Expected |
|------|-------|----------|
| **PRSEAM-1** | 20% rule on one item; resolve through `PriceResolverPort` → price a REAL selling Sales Order (list 100,000 × 2) | resolved unit `80,000`; the discounted order's subtotal `160,000` vs the no-rule item's `200,000` — the promo discount is exactly the difference selling booked. Zero normal Cargo edge. |
| **PRSEAM-2** | promo accrues off POS's REAL `PosInvoicePaid` (rounded 250,000; 0.01 pts/spend) | earns `2,500` points; redelivering the same paid event earns nothing more (idempotent per source). |
| **PRSEAM-3** (council 2026-07-06) | coupon-gated 30% rule, `max_use=1`: A resolves → prices a REAL selling order → commit at order-confirm; B presents the same code | A: unit `70,000`, `applied_coupon_id` returned; a re-resolve is still just an OFFER (cap intact) until commit; commit keyed by the order consumes the use. B: coupon **refused** at resolve, pays list `100,000` — **the cap binds end-to-end** (the wired burn, not just the probe). Proven-by-revert: skip the commit and B still gets the discount. |
| **§5 round-trip** | regen promo `--force`, re-run | the three user-owned seam files byte-identical; PGC/IP/PRSEAM all still green. |

## Conventions
- Promo posts **no GL** and owns no money of record — a resolved price is an *input* to selling/POS;
  loyalty points are a subledger, not an accounting entry.
- Resolution is deterministic (priority → specificity → newest → id) and side-effect-free.
- Coupons are bounded (`used_count ≤ max_use`); loyalty accrual is idempotent per source; redemption is
  balance-bounded + serialized per member.

## Cart-scoped resolution — `resolve_cart` (ADR-002; `tests/cart_resolution.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **CART-1** | order rule 10% off, spend ≥ 250k; lines 100k + 200k | 30k off, allocated ∝ gross → 10k / 20k; total 270k. |
| **CART-3** | 10,000 fixed off, three equal 100k lines | shares 3333.33 / 3333.33 / 3333.34 (tie out exactly). |
| **CART-4/5/6** | all_of / missing component / any_n bundles | fires on the matched set / doesn't fire / fires on `required_distinct` present. |
| **CART-15** (council 2026-07-06) | 100%-off **stackable** bundle on 1 of 2 lines + stackable 50% order rule | net A = 0, net B = 50k, **Σ net == total (50k)** — capacity-aware allocation, no lost cents. |
| **CART-16** (free-line) | buy A → 1 free B | A charged in full; `reward_lines = [{B, 1}]`; total unchanged. |

## Cart seams — selling / POS (`tests/cart_selling_seam.rs`, `tests/cart_pos_seam.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **CSSEAM-1** | bundle 10% + order 5% (stackable), priced by promo → REAL Sales Order | subtotal 300k → 30k → 13.5k → **256.5k on the order**. |
| **CSSEAM-3** / **CPSEAM-2** | buy-X-get-Y free item → real order / ticket | the free item is added as a **zero-priced line**; subtotal/net unchanged. |
| **CPSEAM-1** | same bundle+order rule → REAL POS ticket | ticket net = **256.5k**. |
