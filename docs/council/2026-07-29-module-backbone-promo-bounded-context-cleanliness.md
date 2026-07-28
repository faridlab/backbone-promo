---
date: 2026-07-29
repo_type: module
unit: backbone-promo
domain: promo / pricing & promotions
focus: bounded-context-cleanliness
roster:
  standing: [chair, skeptic, steelman, yagni-business]
  context: [ddd-bounded-context, contract-seat]
  invited: [domain-expert]   # promo encodes real-world domain rules
  subagent_seats: [skeptic, steelman, chair]   # run as isolated subagents
probe: "grep the reference service for reads of promo `type` outside the evaluate() switch"
---

# Council — module:backbone-promo — focus: bounded-context-cleanliness

## Best call

**Opt-B — document the structural mapping only; do NOT adopt a `promo_type` field, enum, or Campaign aggregate.** Concretely: (1) author `docs/adr/ADR-003-promo-type-not-adopted.md` recording the decision and the probe evidence; (2) add one informal glossary entry `Promotion` to `docs/glossary.md` carrying the type→structure mapping. Do NOT touch `pricing_rule.model.yaml`, `promo_bundle.model.yaml`, `ResolvedPrice`, the `resolve()` signature (ADR-001), or the cart pipeline (ADR-002). The generated `BackboneCrudHandler` per-entity listing stays as-is.

Mapping to record (all derivable from existing fields — no stored tag):
- `standard` → `PricingRule` (scope=line, no threshold, no window)
- `minimum` → `PricingRule` (scope=order, `min_order_amount > 0`)
- `event` → `PricingRule` (non-trivial `valid_from`/`valid_to` window)
- `quantity` → `PromoBundle` (match + free-item/discount reward) — correctly its own entity

- **Residual negative value:** ~1–2h now to write ADR-003 + glossary note; zero coupling added (no new field, no new normal Cargo edge, `ResolvedPrice` untouched). The deferred cost: when a backbone-side reporting/addressing consumer arrives, ~3–4h to add a derived view or opaque admin-layer classification — and crucially **no data backfill**, because the classification is fully computable from existing fields. Ongoing cost is one doc invariant: the mapping note must stay accurate if PricingRule/PromoBundle axes change (low — these axes are SSoT-stable). No active consumer friction exists today.
- **Reversibility:** easy. Opt-B is documentation-only — zero schema/code/runtime surface change. A future move to Opt-C (Campaign aggregate) or a stored `promo_type` column is fully additive; no row rewrite is ever needed.
- **What would flip this:** a concrete consumer *inside backbone-promo or a consuming backend-service* that must address/group/filter/report by a single "Promotion" classification. Specifically: (a) an admin "Promotions" index aggregating `PricingRule` + `PromoBundle` under one surface, or (b) a usage-by-promo-kind report on the promo or selling side. Cheap probe to re-confirm: `grep -ri "promotion" src/ presentation/` in backbone-promo + backbone-selling + backbone-pos for any list/report/group-by surface, or a product spec referencing "list promotions."

## Disagreement map

1. **Vocabulary gap is real vs. no demonstrated consumer.** *Crux:* does a missing "Promotion" noun justify building it without a reader? Steelman + Domain-Expert say the umbrella term is genuinely absent (glossary confirmed: PricingRule/PromoBundle/CouponCode only). YAGNI + Skeptic counter that no backbone consumer reads it — the probe found exactly one classification read and it lives in the reference service's `PromotionUsageReportService`, not here. **Chair side: YAGNI.** The glossary absorbs the noun at zero cost; building without a consumer pays maintenance for residue.

2. **Campaign aggregate vs. thin tag wrapper.** *Crux:* is "Promotion" an aggregate with identity+lifecycle, or just a label? Steelman/Opt-C propose Campaign as the carrier. DDD-seat (strict) + Contract-seat + Skeptic object that no consumer needs identity+lifecycle, and a Campaign that references PricingRule/PromoBundle by internal struct shape *extends* the existing re-export leak rather than sealing it. **Chair side: no aggregate.** Aggregate cost (invariants, lifecycle, repository, regen surface) for a derivable classification is unjustified.

3. **Non-branching classification tag (read layer) vs. no tag at all.** *Crux:* does the reference service's single usage-report read justify even a thin, non-branching `promo_type` in backbone's read layer? A Steelman-lite/Domain-Expert coalition wants a classification for reporting vocabulary. YAGNI + Contract-seat + Skeptic note the consumer is in another service, a tag without a reader is residue, and any tag creates drift pressure toward `ResolvedPrice`. **Chair side: no tag today** — add one only when the flip evidence above appears, and only as an opaque admin/read field.

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | **Opt-B: ADR-003 + glossary `Promotion` mapping; no schema/struct change.** | Highest — closes the vocabulary/addressability gap at zero coupling cost; preserves ADR-001 determinism and the zero-normal-Cargo-edge invariant. | Latent reporting friction (~3–4h later, no backfill); ~1–2h now; one doc invariant to maintain. | Easy (docs-only; fully additive later). | A backbone/selling/POS consumer that groups or reports by promo classification. |
| 2 | **Fill the missing TARGET dimension on `ApplyOn`** (`showcase`, `bundle`) in `pricing_rule.model.yaml` — **orthogonal capability, decide in its own ADR, not here.** | High — closes a real capability gap (Domain-Expert confirmed the reference service targets by showcase/bundle; backbone's ApplyOn is item/item_group/brand/all only). | Widens the enum + resolver selector surface; enum shrink later is one-way-ish. | Additive now, costly to reverse. | Confirm `showcase` is a catalog concept backbone-promo can reference *without* re-introducing a cross-module leak (it may belong as a logical FK, not an ApplyOn variant). |
| 3 | **If a consumer appears: add a NON-branching, opaque `promo_type` as a derived view/admin-layer field only — never in `ResolvedPrice` or the resolver signature.** | Conditional — unlocks grouping/reporting only with a reader. | A derivable field that can drift; prefer a view/computed column over a stored one. | Easy if view; medium if stored (then backfill). | The flip evidence in #1 materializing. |
| 4 | **Opt-A (PromoType enum + branching switch): REJECTED — do not do.** | None — strictly worse. | Creates a second execution path; one concept means two things; breaks ADR-001's total-order determinism; duplicates the structural model the probe proved is superior. | n/a | None — the probe forecloses it. |

## Parking lot

- **Campaign aggregate (Opt-C's carrier):** revisit only when a consumer needs identity+lifecycle spanning PricingRule + PromoBundle (e.g., a single "create promotion" admin workflow with its own window/coupon). Not justified today — no demonstrated lifecycle need.
- **`showcase` / `bundle` targeting on `ApplyOn`:** real capability gap, but a *capability* question, not a *classification* one — out of this focus lens. Needs its own ADR; the open question is whether "showcase" is a catalog concept that leaks across the boundary.
- **Coupon-gating a `PromoBundle`:** already deferred per `promo_bundle.model.yaml` note / ADR-002; unrelated to this question.
- **Unified admin "Promotion" index in `presentation/http`:** a read-model/presentation concern, not a domain-model one — defer until a real admin surface is specced.
