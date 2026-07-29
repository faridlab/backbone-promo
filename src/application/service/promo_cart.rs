//! Cart-scoped price resolution (hand-authored, user-owned).
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]: a whole
//! basket run through a fixed pipeline that preserves single-line determinism. Reuses [`super::promo_resolve`]'s
//! `resolve` for the line pass, then runs a bundle pass and an order pass, reconciled so
//! `Σ net_line_total == total` EXACTLY. Side-effect-free, exactly like `resolve` (coupons are burned
//! only by `commit_coupon_redemption`).
//!
//! Per the module's 4-layer rule this file holds no SQL — the bundle/component/order-rule searches
//! live on `PromoBundleRepository` / `PromoBundleComponentRepository` / `PricingRuleRepository`.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::promo_ports::{
    AdjustmentSource, CartLine, CartQuery, OrderAdjustment, PriceQuery, PricingError, ResolvedCart,
    ResolvedLine, RewardLine,
};
use super::promo_write_service::{money, PromoWriteService};

/// A `scope=order` rule pulled for the cart's order pass.
struct OrderRuleCand {
    id: Uuid,
    priority: i32,
    customer_id: Option<Uuid>,
    customer_group_id: Option<Uuid>,
    coupon_required: bool,
    rate_or_discount: String,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    discount_upto: Option<Decimal>,
    stackable: bool,
    valid_from: chrono::DateTime<chrono::Utc>,
}

impl OrderRuleCand {
    /// A narrower audience wins a priority tie: +2 for a customer match, +1 for a group match.
    fn specificity(&self) -> i32 {
        (if self.customer_id.is_some() { 2 } else { 0 })
            + (if self.customer_group_id.is_some() { 1 } else { 0 })
    }

    /// The discount this order rule takes off `base` (the remaining order value). `rate` is not
    /// meaningful at order scope and yields no discount.
    fn discount_on(&self, base: Decimal) -> Decimal {
        let hundred = Decimal::from(100);
        let raw = match self.rate_or_discount.as_str() {
            "discount_percentage" => {
                let pct = self.discount_percentage.unwrap_or(Decimal::ZERO).min(hundred);
                money(base * pct / hundred)
            }
            "discount_amount" => money(self.discount_amount.unwrap_or(Decimal::ZERO)),
            _ => Decimal::ZERO,
        };
        // discount_upto caps the discount this rule may grant; null/zero = no cap.
        match self.discount_upto {
            Some(cap) if cap > Decimal::ZERO => raw.min(cap),
            _ => raw,
        }
    }
}

/// One component of a bundle: a selector + how much of it a single satisfied set needs.
struct BundleComponentCand {
    apply_on: String,
    item_id: Option<Uuid>,
    item_group_id: Option<Uuid>,
    brand_id: Option<Uuid>,
    min_qty: Decimal,
}

impl BundleComponentCand {
    /// Does this resolved line satisfy this component's selector?
    fn matches(&self, l: &ResolvedLine, cart_line: &CartLine) -> bool {
        match self.apply_on.as_str() {
            "item" => self.item_id == Some(l.item_id),
            "item_group" => {
                self.item_group_id.is_some() && self.item_group_id == cart_line.query.item_group_id
            }
            "brand" => self.brand_id.is_some() && self.brand_id == cart_line.query.brand_id,
            _ => false,
        }
    }
}

/// A bundle pulled for the cart's bundle pass, with its components.
struct BundleCand {
    id: Uuid,
    match_type: String,
    required_distinct: Option<i32>,
    reward: String,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    reward_item_id: Option<Uuid>,
    reward_qty: Option<Decimal>,
    stackable: bool,
    components: Vec<BundleComponentCand>,
    gifts: Vec<(Uuid, Decimal)>,
}

impl BundleCand {
    /// Number of satisfied "sets" of this component in the cart, and the line ids that matched it.
    fn component_fill(
        &self,
        comp: &BundleComponentCand,
        cart: &CartQuery,
        lines: &[ResolvedLine],
    ) -> (Decimal, Vec<Uuid>) {
        let mut qty = Decimal::ZERO;
        let mut ids = Vec::new();
        for (rl, cl) in lines.iter().zip(cart.lines.iter()) {
            if comp.matches(rl, cl) {
                qty += rl.quantity;
                ids.push(rl.line_id);
            }
        }
        let sets = if comp.min_qty > Decimal::ZERO {
            (qty / comp.min_qty).floor()
        } else {
            Decimal::ZERO
        };
        (sets, ids)
    }

    /// Number of satisfied "sets" of the WHOLE bundle and the contributing line ids. Shared by the
    /// discount reward and the free-item reward.
    fn satisfied_sets(&self, cart: &CartQuery, lines: &[ResolvedLine]) -> (Decimal, Vec<Uuid>) {
        let mut satisfied = 0i32; // components with ≥1 set
        let mut min_sets: Option<Decimal> = None; // min sets across satisfied components (all_of)
        let mut contributing: Vec<Uuid> = Vec::new();
        for comp in &self.components {
            let (sets, ids) = self.component_fill(comp, cart, lines);
            if sets >= Decimal::ONE {
                satisfied += 1;
                min_sets = Some(min_sets.map_or(sets, |m| m.min(sets)));
                for id in ids {
                    if !contributing.contains(&id) {
                        contributing.push(id);
                    }
                }
            }
        }
        let sets = match self.match_type.as_str() {
            // any_n: any `required_distinct` (default: all) distinct components present → one set.
            "any_n" => {
                let need = self.required_distinct.unwrap_or(self.components.len() as i32).max(1);
                if satisfied >= need { Decimal::ONE } else { Decimal::ZERO }
            }
            // all_of: every component must be present; sets = min fill across them.
            _ => {
                if satisfied == self.components.len() as i32 {
                    min_sets.unwrap_or(Decimal::ZERO)
                } else {
                    Decimal::ZERO
                }
            }
        };
        (sets, contributing)
    }

    /// The free item this bundle grants, if it's a buy-X-get-Y bundle and it is satisfied:
    /// `(reward_item, reward_qty × sets)`.
    fn free_reward(&self, cart: &CartQuery, lines: &[ResolvedLine]) -> Option<(Uuid, Decimal)> {
        let item = self.reward_item_id?;
        let (sets, _) = self.satisfied_sets(cart, lines);
        if sets < Decimal::ONE {
            return None;
        }
        let per_set = self.reward_qty.unwrap_or(Decimal::ZERO);
        let qty = per_set * sets;
        if qty <= Decimal::ZERO {
            return None;
        }
        Some((item, qty))
    }

    /// The free item(s) this bundle grants via its gift list, when it is satisfied: one
    /// `(gift_item, gift_qty × sets)` per gift. Empty when the bundle has no gifts or isn't satisfied.
    /// Takes precedence over the legacy single `reward_item_id` when gifts are configured.
    fn free_rewards(&self, cart: &CartQuery, lines: &[ResolvedLine]) -> Vec<(Uuid, Decimal)> {
        let (sets, _) = self.satisfied_sets(cart, lines);
        if sets < Decimal::ONE || self.gifts.is_empty() {
            return Vec::new();
        }
        self.gifts
            .iter()
            .filter_map(|(item, per_set)| {
                let qty = *per_set * sets;
                if qty > Decimal::ZERO { Some((*item, qty)) } else { None }
            })
            .collect()
    }

    /// Compute the reward discount and the lines that contributed to satisfying the bundle.
    /// Returns `(discount, contributing_line_ids)`; discount is 0 when the bundle isn't satisfied.
    fn reward(&self, cart: &CartQuery, lines: &[ResolvedLine]) -> (Decimal, Vec<Uuid>) {
        let (sets, contributing) = self.satisfied_sets(cart, lines);
        if sets < Decimal::ONE {
            return (Decimal::ZERO, Vec::new());
        }

        let matched_value: Decimal = money(
            lines
                .iter()
                .filter(|l| contributing.contains(&l.line_id))
                .map(|l| l.unit_price * l.quantity)
                .sum(),
        );
        let hundred = Decimal::from(100);
        let disc = match self.reward.as_str() {
            "discount_percentage" => {
                let pct = self.discount_percentage.unwrap_or(Decimal::ZERO).min(hundred);
                money(matched_value * pct / hundred)
            }
            // Fixed amount off, once per satisfied set.
            "discount_amount" => money(self.discount_amount.unwrap_or(Decimal::ZERO) * sets),
            _ => Decimal::ZERO,
        };
        (disc.min(matched_value), contributing)
    }
}

/// Apply a pass's allocated shares onto the lines' running `order_discount_share`, so the next pass's
/// `allocate` sees the reduced remaining capacity.
fn apply_shares(lines: &mut [ResolvedLine], allocated: &[(Uuid, Decimal)]) {
    for (line_id, share) in allocated {
        if let Some(l) = lines.iter_mut().find(|l| l.line_id == *line_id) {
            l.order_discount_share += *share;
        }
    }
}

/// Allocate `total` across `line_ids` proportional to each line's **remaining capacity**
/// (gross − shares already taken), never assigning a line more than it can absorb, with the rounding
/// remainder folded onto the line with the most remaining capacity so Σ shares ties out EXACTLY.
///
/// Returns `(actually_allocated, shares)`. `actually_allocated` may be **less** than `total` when the
/// subset lacks capacity — a discount can never push a line below zero, so `Σ shares` can only cover
/// what the lines are worth. Weighting by *remaining* capacity (not raw gross) is what keeps
/// conservation intact when a bundle and a stackable order rule hit the same line: the second
/// adjustment sees the first's draw and can't over-allocate, so `Σ net_line_total == total` holds
/// without a lossy clamp (council 2026-07-06).
fn allocate(
    total: Decimal,
    line_ids: &[Uuid],
    lines: &[ResolvedLine],
) -> (Decimal, Vec<(Uuid, Decimal)>) {
    if total <= Decimal::ZERO || line_ids.is_empty() {
        return (Decimal::ZERO, Vec::new());
    }
    // Remaining capacity of a line = its gross minus what prior adjustments already took.
    let cap = |id: &Uuid| -> Decimal {
        lines
            .iter()
            .find(|l| l.line_id == *id)
            .map(|l| (l.unit_price * l.quantity - l.order_discount_share).max(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO)
    };
    let cap_sum: Decimal = line_ids.iter().map(cap).sum();
    if cap_sum <= Decimal::ZERO {
        return (Decimal::ZERO, Vec::new());
    }
    // Never allocate more than the subset can hold.
    let disc = total.min(cap_sum);

    let mut shares: Vec<(Uuid, Decimal)> = Vec::with_capacity(line_ids.len());
    let mut running = Decimal::ZERO;
    for id in line_ids {
        // Proportional share ≤ cap(id) since disc ≤ cap_sum; `.min` is a belt-and-braces guard.
        let share = money(disc * cap(id) / cap_sum).min(cap(id));
        running += share;
        shares.push((*id, share));
    }
    // Fold the rounding remainder onto the line with the most SLACK so shares tie out and stay ≤ cap.
    let remainder = disc - running;
    if remainder != Decimal::ZERO {
        let slack = |i: usize| cap(&shares[i].0) - shares[i].1;
        if let Some(idx) = (0..shares.len()).max_by(|&a, &b| slack(a).cmp(&slack(b))) {
            shares[idx].1 = money(shares[idx].1 + remainder);
        }
    }
    (disc, shares)
}

impl PromoWriteService {
    // ---- 1b. resolve_cart (cart-scoped read, ADR-002) -----------------------------------------

    /// Resolve a whole basket. Runs a fixed pipeline that preserves single-line determinism:
    ///   1. **line pass** — today's `resolve` per line, unchanged, yielding the subtotal.
    ///   2. **bundle pass** — each satisfiable `PromoBundle` (priority DESC) rewards its matched lines.
    ///   3. **order pass** — each `scope=order` PricingRule (priority DESC) gated on the subtotal.
    ///   4. **reconcile** — every order-level discount is allocated back across its contributing lines
    ///      (∝ line net value, penny-reconciled so shares sum EXACTLY), total capped at the subtotal.
    /// Side-effect-free, exactly like `resolve` (coupons are burned only by `commit_coupon_redemption`).
    pub async fn resolve_cart(&self, cart: &CartQuery) -> Result<ResolvedCart, PricingError> {
        // A coupon-gated order rule / (future) bundle unlocks only when the cart's coupon maps to it.
        let unlocked_rule: Option<Uuid> = match &cart.coupon_code {
            Some(code) => self
                .lookup_valid_coupon(cart.company_id, code, cart.at)
                .await?
                .map(|(_, rule_id)| rule_id),
            None => None,
        };

        // ---- 1. LINE PASS: price each line exactly as the single-line seam would. ----
        let mut lines: Vec<ResolvedLine> = Vec::with_capacity(cart.lines.len());
        for cl in &cart.lines {
            // Cart-wide customer/coupon/instant win over anything on the line's own query.
            let q = PriceQuery {
                company_id: cart.company_id,
                customer_id: cart.customer_id,
                customer_group_id: cart.customer_group_id,
                coupon_code: cart.coupon_code.clone(),
                at: cart.at,
                ..cl.query.clone()
            };
            let rp = self.resolve(&q).await?;
            let gross = money(rp.unit_price * q.quantity);
            lines.push(ResolvedLine {
                line_id: cl.line_id,
                item_id: q.item_id,
                quantity: q.quantity,
                unit_price: rp.unit_price,
                line_discount_amount: rp.discount_amount,
                applied_rule_id: rp.applied_rule_id,
                order_discount_share: Decimal::ZERO,
                net_line_total: gross,
            });
        }
        let subtotal: Decimal = money(lines.iter().map(|l| l.unit_price * l.quantity).sum());

        // `remaining` bounds the running order-level discount so Σ can never exceed the subtotal.
        let mut remaining = subtotal;
        // `locked` = an exclusive (non-stackable) adjustment has fired; nothing may stack on it.
        let mut locked = false;
        let mut adjustments: Vec<OrderAdjustment> = Vec::new();

        // ---- 2. BUNDLE PASS ----
        let mut reward_lines: Vec<RewardLine> = Vec::new();
        for bundle in self.load_active_bundles(cart, subtotal).await? {
            // Buy-X-get-Y: a satisfied free-item bundle grants extra goods, not a discount. It doesn't
            // touch `remaining`/`locked` (a free line isn't an order-level discount on the basket).
            // Gifts (1..N free items) take precedence over the legacy single reward_item_id.
            let gifts = bundle.free_rewards(cart, &lines);
            if !gifts.is_empty() {
                for (item_id, quantity) in gifts {
                    reward_lines.push(RewardLine { bundle_id: bundle.id, item_id, quantity });
                }
                continue;
            }
            if bundle.reward_item_id.is_some() {
                if let Some((item_id, quantity)) = bundle.free_reward(cart, &lines) {
                    reward_lines.push(RewardLine { bundle_id: bundle.id, item_id, quantity });
                }
                continue;
            }
            if locked || remaining <= Decimal::ZERO {
                break;
            }
            let (raw, contributing) = bundle.reward(cart, &lines);
            let want = money(raw.min(remaining));
            if want <= Decimal::ZERO {
                continue;
            }
            // A non-stackable promotion is exclusive: it may fire only if nothing else has yet.
            if !bundle.stackable && !adjustments.is_empty() {
                continue;
            }
            // `disc` is what the contributing lines could actually absorb (≤ want).
            let (disc, allocated) = allocate(want, &contributing, &lines);
            if disc <= Decimal::ZERO {
                continue;
            }
            apply_shares(&mut lines, &allocated);
            adjustments.push(OrderAdjustment {
                source: AdjustmentSource::Bundle(bundle.id),
                discount_amount: disc,
                allocated,
            });
            remaining -= disc;
            if !bundle.stackable {
                locked = true;
            }
        }

        // ---- 3. ORDER PASS: scope=order rules gated on the subtotal. ----
        if !locked {
            let all_line_ids: Vec<Uuid> = lines.iter().map(|l| l.line_id).collect();
            let total_qty: Decimal = cart.lines.iter().map(|l| l.query.quantity).sum();
            for rule in self.load_order_rules(cart, subtotal, total_qty, unlocked_rule).await? {
                if locked || remaining <= Decimal::ZERO {
                    break;
                }
                let want = money(rule.discount_on(remaining).min(remaining));
                if want <= Decimal::ZERO {
                    continue;
                }
                if !rule.stackable && !adjustments.is_empty() {
                    continue;
                }
                let (disc, allocated) = allocate(want, &all_line_ids, &lines);
                if disc <= Decimal::ZERO {
                    continue;
                }
                apply_shares(&mut lines, &allocated);
                adjustments.push(OrderAdjustment {
                    source: AdjustmentSource::OrderRule(rule.id),
                    discount_amount: disc,
                    allocated,
                });
                remaining -= disc;
                if !rule.stackable {
                    locked = true;
                }
            }
        }

        // ---- 4. RECONCILE: shares were applied incrementally (capacity-aware), so no line's share
        // can exceed its gross — net is exact, no lossy clamp, and Σ net_line_total == total. ----
        for l in &mut lines {
            let gross = money(l.unit_price * l.quantity);
            l.order_discount_share = money(l.order_discount_share);
            l.net_line_total = (gross - l.order_discount_share).max(Decimal::ZERO);
        }

        let order_discount_total: Decimal =
            money(adjustments.iter().map(|a| a.discount_amount).sum());
        Ok(ResolvedCart {
            lines,
            order_adjustments: adjustments,
            reward_lines,
            subtotal,
            order_discount_total,
            total: money(subtotal - order_discount_total),
        })
    }

    /// Load active, in-window `scope=order` rules whose subtotal floor + audience + coupon gate all
    /// pass, ordered priority DESC → specificity (customer > group) DESC → newest → id.
    async fn load_order_rules(
        &self,
        cart: &CartQuery,
        subtotal: Decimal,
        total_qty: Decimal,
        unlocked_rule: Option<Uuid>,
    ) -> Result<Vec<OrderRuleCand>, PricingError> {
        // RLS scope (ADR-0008): company on the cart — scope the read.
        let rows = company_scope::with_company_scope(
            Some(cart.company_id),
            self.rules.find_order_candidates(
                &self.pool,
                cart.company_id,
                cart.at,
                cart.customer_id,
                cart.customer_group_id,
                subtotal,
                total_qty,
            ),
        )
        .await?;

        let mut cands: Vec<OrderRuleCand> = rows
            .into_iter()
            .map(|r| OrderRuleCand {
                id: r.id,
                priority: r.priority,
                customer_id: r.customer_id,
                customer_group_id: r.customer_group_id,
                coupon_required: r.coupon_required,
                rate_or_discount: r.rate_or_discount,
                discount_percentage: r.discount_percentage,
                discount_amount: r.discount_amount,
                discount_upto: r.discount_upto,
                stackable: r.stackable,
                valid_from: r.valid_from,
            })
            .filter(|c| !c.coupon_required || unlocked_rule == Some(c.id))
            .collect();
        cands.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(b.specificity().cmp(&a.specificity()))
                .then(b.valid_from.cmp(&a.valid_from))
                .then(a.id.cmp(&b.id))
        });
        Ok(cands)
    }

    /// Load active, in-window bundles (with their components) whose subtotal floor passes, ordered
    /// priority DESC → newest → id.
    async fn load_active_bundles(
        &self,
        cart: &CartQuery,
        subtotal: Decimal,
    ) -> Result<Vec<BundleCand>, PricingError> {
        // RLS scope (ADR-0008): company on the cart — scope both the bundle and component reads.
        let brows = company_scope::with_company_scope(
            Some(cart.company_id),
            self.bundles.find_active(&self.pool, cart.company_id, cart.at, subtotal),
        )
        .await?;
        if brows.is_empty() {
            return Ok(Vec::new());
        }

        let bundle_ids: Vec<Uuid> = brows.iter().map(|r| r.id).collect();
        let crows = company_scope::with_company_scope(
            Some(cart.company_id),
            self.bundle_components.find_for_bundles(&self.pool, cart.company_id, &bundle_ids),
        )
        .await?;

        let grows = company_scope::with_company_scope(
            Some(cart.company_id),
            self.bundle_gifts.find_for_bundles(&self.pool, cart.company_id, &bundle_ids),
        )
        .await?;

        let mut bundles: Vec<BundleCand> = brows
            .into_iter()
            .map(|r| BundleCand {
                id: r.id,
                match_type: r.match_type,
                required_distinct: r.required_distinct,
                reward: r.reward,
                discount_percentage: r.discount_percentage,
                discount_amount: r.discount_amount,
                reward_item_id: r.reward_item_id,
                reward_qty: r.reward_qty,
                stackable: r.stackable,
                components: Vec::new(),
                gifts: Vec::new(),
            })
            .collect();
        for cr in crows {
            if let Some(b) = bundles.iter_mut().find(|b| b.id == cr.bundle_id) {
                b.components.push(BundleComponentCand {
                    apply_on: cr.apply_on,
                    item_id: cr.item_id,
                    item_group_id: cr.item_group_id,
                    brand_id: cr.brand_id,
                    min_qty: cr.min_qty,
                });
            }
        }
        for gr in grows {
            if let Some(b) = bundles.iter_mut().find(|b| b.id == gr.bundle_id) {
                b.gifts.push((gr.gift_item_id, gr.gift_qty));
            }
        }
        // A bundle with no components can never be satisfied — drop it.
        bundles.retain(|b| !b.components.is_empty());
        Ok(bundles)
    }
}
