//! The hand-authored promo write path (user-owned; survives regen).
//!
//! Three responsibilities, none of which post GL:
//!   1. `resolve` — the marquee READ. Given a line's dimensions + optional coupon, deterministically
//!      pick the winning pricing rule (priority DESC, then specificity, then newest) and return the
//!      effective unit price. Selling/POS consume this via `PriceResolverPort`; resolve NEVER mutates
//!      (previewing a price must not consume a coupon).
//!   2. `commit_coupon_redemption` — the bounded WRITE. Advance a coupon's `used_count` atomically,
//!      guarded so it can never exceed `max_use` (over-redemption is impossible under concurrency).
//!   3. loyalty ledger — `accrue` (idempotent per source) + `redeem` (balance-bounded, serialized).
//!
//! Money is IDR, 2dp, half-away-from-zero. Points are whole (floored on accrual) and are NOT money.

use backbone_orm::company_scope;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    CouponCodeRepository, CouponRedemptionRepository, LineRuleQuery, LoyaltyPointEntryRepository,
    LoyaltyProgramRepository, NewAccrualRow, NewRedemptionRow, PricingRuleRepository,
    PromoBundleComponentRepository, PromoBundleRepository,
};

use super::promo_events::{
    CouponRedeemed, LoyaltyPointsEarned, LoyaltyPointsRedeemed, PromoEvent, PromoEventSink,
};
use super::promo_ports::{
    AccrualRequest, AdjustmentSource, CartQuery, OrderAdjustment, PriceQuery, PriceResolverPort,
    PricingError, RedemptionRequest, ResolvedCart, ResolvedLine, ResolvedPrice, RewardLine,
};

/// Round to 2dp, half away from zero (IDR money).
fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// The promo write service: orchestrates the repositories that hold the SQL, and owns the units of
/// work (the coupon burn and the loyalty redemption each run in a transaction it opens).
pub struct PromoWriteService {
    pool: PgPool,
    rules: PricingRuleRepository,
    coupons: CouponCodeRepository,
    redemptions: CouponRedemptionRepository,
    bundles: PromoBundleRepository,
    bundle_components: PromoBundleComponentRepository,
    programs: LoyaltyProgramRepository,
    entries: LoyaltyPointEntryRepository,
}

/// A candidate rule pulled from the DB, with the fields resolution needs.
struct Candidate {
    id: Uuid,
    priority: i32,
    apply_on: String,
    customer_id: Option<Uuid>,
    customer_group_id: Option<Uuid>,
    coupon_required: bool,
    rate_or_discount: String,
    rate: Option<Decimal>,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    valid_from: chrono::DateTime<chrono::Utc>,
}

impl Candidate {
    /// Specificity: a more targeted selector / narrower audience wins a priority tie.
    /// item(30) > brand/item_group(20) > all(10); +2 for a customer match, +1 for a group match.
    fn specificity(&self) -> i32 {
        let base = match self.apply_on.as_str() {
            "item" => 30,
            "brand" | "item_group" => 20,
            _ => 10,
        };
        base + if self.customer_id.is_some() { 2 } else { 0 }
            + if self.customer_group_id.is_some() { 1 } else { 0 }
    }
}

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
        match self.rate_or_discount.as_str() {
            "discount_percentage" => {
                let pct = self.discount_percentage.unwrap_or(Decimal::ZERO).min(hundred);
                money(base * pct / hundred)
            }
            "discount_amount" => money(self.discount_amount.unwrap_or(Decimal::ZERO)),
            _ => Decimal::ZERO,
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
    fn matches(&self, l: &ResolvedLine, cart_line: &super::promo_ports::CartLine) -> bool {
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

/// Outcome of a loyalty accrual.
#[derive(Debug, Clone, PartialEq)]
pub struct AccrualOutcome {
    pub entry_id: Option<Uuid>,
    pub points: Decimal,
    /// True when the source had already accrued (idempotent no-op).
    pub already: bool,
}

/// Outcome of a loyalty redemption.
#[derive(Debug, Clone, PartialEq)]
pub struct RedemptionOutcome {
    pub entry_id: Uuid,
    pub points: Decimal,
    pub discount_value: Decimal,
    /// True when the source had already redeemed (idempotent replay).
    pub already: bool,
}

impl PromoWriteService {
    pub fn new(pool: PgPool) -> Self {
        let rules = PricingRuleRepository::new(pool.clone());
        let coupons = CouponCodeRepository::new(pool.clone());
        let redemptions = CouponRedemptionRepository::new(pool.clone());
        let bundles = PromoBundleRepository::new(pool.clone());
        let bundle_components = PromoBundleComponentRepository::new(pool.clone());
        let programs = LoyaltyProgramRepository::new(pool.clone());
        let entries = LoyaltyPointEntryRepository::new(pool.clone());
        Self { pool, rules, coupons, redemptions, bundles, bundle_components, programs, entries }
    }

    // ---- 1. resolve (read-only) --------------------------------------------------------------

    /// Resolve the effective price for a line. Deterministic and side-effect-free.
    pub async fn resolve(&self, q: &PriceQuery) -> Result<ResolvedPrice, PricingError> {
        if q.quantity <= Decimal::ZERO {
            return Err(PricingError::Invalid("quantity must be positive".into()));
        }
        if q.list_price < Decimal::ZERO {
            return Err(PricingError::Invalid("list_price must be non-negative".into()));
        }
        let gross = money(q.quantity * q.list_price);

        // If a coupon was presented, resolve it to the rule it unlocks (must be valid + not exhausted).
        let unlocked: Option<(Uuid, Uuid)> = match &q.coupon_code {
            Some(code) => self.lookup_valid_coupon(q.company_id, code, q.at).await?,
            None => None,
        };
        let unlocked_rule = unlocked.map(|(_, rule_id)| rule_id);
        let unlocked_coupon = unlocked.map(|(coupon_id, _)| coupon_id);

        // Structural candidates: active, in-window, selector + audience + qty/amount all match.
        // RLS scope (ADR-0008): the query carries its company — scope the read so it is fenced on
        // `app.company_id` even off the request path. The explicit `company_id = $1` filter stays as
        // defense-in-depth.
        let rows = company_scope::with_company_scope(
            Some(q.company_id),
            self.rules.find_line_candidates(&self.pool, &LineRuleQuery {
                company_id: q.company_id,
                at: q.at,
                item_id: q.item_id,
                item_group_id: q.item_group_id,
                brand_id: q.brand_id,
                customer_id: q.customer_id,
                customer_group_id: q.customer_group_id,
                quantity: q.quantity,
                gross,
            }),
        )
        .await?;

        let mut candidates: Vec<Candidate> = rows
            .into_iter()
            .map(|r| Candidate {
                id: r.id,
                priority: r.priority,
                apply_on: r.apply_on,
                customer_id: r.customer_id,
                customer_group_id: r.customer_group_id,
                coupon_required: r.coupon_required,
                rate_or_discount: r.rate_or_discount,
                rate: r.rate,
                discount_percentage: r.discount_percentage,
                discount_amount: r.discount_amount,
                valid_from: r.valid_from,
            })
            // A coupon-gated rule applies only if the presented coupon unlocks *this* rule.
            .filter(|c| !c.coupon_required || unlocked_rule == Some(c.id))
            .collect();

        if candidates.is_empty() {
            return Ok(ResolvedPrice::passthrough(q.list_price));
        }

        // Deterministic winner: priority DESC, specificity DESC, newest DESC, id ASC.
        candidates.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(b.specificity().cmp(&a.specificity()))
                .then(b.valid_from.cmp(&a.valid_from))
                .then(a.id.cmp(&b.id))
        });
        let win = &candidates[0];

        let unit_price = self.apply_effect(win, q.list_price);
        let discount = (q.list_price - unit_price).max(Decimal::ZERO);
        Ok(ResolvedPrice {
            unit_price,
            discount_amount: money(discount),
            applied_rule_id: Some(win.id),
            // Report the coupon only when it was load-bearing (the winning rule required it).
            applied_coupon_id: if win.coupon_required { unlocked_coupon } else { None },
        })
    }

    /// Compute the effective unit price for the winning rule's effect (never negative).
    fn apply_effect(&self, c: &Candidate, list_price: Decimal) -> Decimal {
        let hundred = Decimal::from(100);
        let unit = match c.rate_or_discount.as_str() {
            "rate" => c.rate.unwrap_or(list_price),
            "discount_percentage" => {
                let pct = c.discount_percentage.unwrap_or(Decimal::ZERO).min(hundred);
                list_price - (list_price * pct / hundred)
            }
            "discount_amount" => list_price - c.discount_amount.unwrap_or(Decimal::ZERO),
            _ => list_price,
        };
        money(unit.max(Decimal::ZERO))
    }

    /// Look up a coupon that is active, in its validity window, and not exhausted.
    /// Returns `(coupon_id, pricing_rule_id)`. `None` if no such usable coupon exists.
    async fn lookup_valid_coupon(
        &self,
        company_id: Uuid,
        code: &str,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<(Uuid, Uuid)>, PricingError> {
        // RLS scope (ADR-0008): company on the parameter — scope the lookup.
        Ok(company_scope::with_company_scope(
            Some(company_id),
            self.coupons.find_usable(&self.pool, company_id, &code.to_uppercase(), at),
        )
        .await?)
    }

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
            for rule in self.load_order_rules(cart, subtotal, unlocked_rule).await? {
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
        // A bundle with no components can never be satisfied — drop it.
        bundles.retain(|b| !b.components.is_empty());
        Ok(bundles)
    }

    // ---- 2. coupon redemption (bounded write) -------------------------------------------------

    /// Consume one use of a coupon when a sale commits. Atomic, bounded, AND idempotent per source:
    ///   * **bounded** — the guarded increment makes `used_count` impossible to advance past
    ///     `max_use`, even under concurrent redemptions (→ `CouponExhausted`).
    ///   * **idempotent** — a `coupon_redemptions` ledger row keyed by `(company, coupon, source)`
    ///     records WHICH document consumed the use. A retry of the same sale (a dropped ack, an
    ///     at-least-once event) finds the existing row and returns the same result WITHOUT a second
    ///     burn — the same partial-unique pattern the loyalty accrual leg uses.
    ///
    /// The ledger insert and the counter bump commit in one transaction: on a fresh source we insert
    /// the ledger row then advance the counter (rolling both back if the coupon is exhausted); on a
    /// replayed source we short-circuit before touching the counter. Returns the pricing_rule_id.
    pub async fn commit_coupon_redemption(
        &self,
        company_id: Uuid,
        coupon_id: Uuid,
        source_type: &str,
        source_id: Uuid,
        sink: &dyn PromoEventSink,
    ) -> Result<Uuid, PricingError> {
        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company is an explicit argument — bind it onto our own transaction so the
        // ledger claim and the guarded counter bump both pass the `app.company_id` fence.
        company_scope::bind_company_on(&mut tx, company_id).await?;

        // Idempotency gate: claim this (coupon, source) exactly once. ON CONFLICT → already redeemed.
        let claimed = self
            .redemptions
            .claim(&mut tx, company_id, coupon_id, source_type, source_id)
            .await?;

        let rule_id: Uuid = match claimed {
            // Fresh source: advance the counter, bounded. Exhausted → roll back the ledger claim.
            Some(rule_id) => {
                let bumped = self.coupons.bump_used_count(&mut tx, coupon_id, company_id).await?;
                if bumped.is_none() {
                    // No use remained (or the coupon is inactive) — undo the ledger claim.
                    return Err(PricingError::CouponExhausted);
                }
                tx.commit().await?;
                sink.publish(&PromoEvent::CouponRedeemed(CouponRedeemed {
                    coupon_id,
                    pricing_rule_id: rule_id,
                    company_id,
                    source_type: source_type.to_string(),
                    source_id,
                }));
                rule_id
            }
            // Replayed source: this sale already consumed a use — return it, no second burn.
            None => {
                let existing = self
                    .redemptions
                    .find_existing(&mut tx, company_id, coupon_id, source_type, source_id)
                    .await?;
                tx.commit().await?;
                existing
            }
        };
        Ok(rule_id)
    }

    // ---- 3. loyalty ledger --------------------------------------------------------------------

    /// Accrue points for a settled purchase. Idempotent per source: the partial unique key
    /// `(company, source_type, source_id, earned)` means one document earns at most once, however
    /// many times the paid event is replayed. `points = floor(purchase_amount · collection_factor)`.
    pub async fn accrue(
        &self,
        req: &AccrualRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<AccrualOutcome, PricingError> {
        if req.purchase_amount < Decimal::ZERO {
            return Err(PricingError::Invalid("purchase_amount must be non-negative".into()));
        }
        let program = self.load_active_program(req.company_id, req.loyalty_program_id, req.at).await?;
        let (collection_factor, expiry_days): (Decimal, Option<i32>) = program;

        let points = (req.purchase_amount * collection_factor).floor();
        if points <= Decimal::ZERO {
            return Ok(AccrualOutcome { entry_id: None, points: Decimal::ZERO, already: false });
        }
        let expiry = expiry_days.map(|d| req.at + chrono::Duration::days(d as i64));

        // RLS scope (ADR-0008): company on the accrual request — scope the insert so it passes the
        // WITH CHECK fence (accrual is event-driven and has no ambient scope of its own).
        let row = company_scope::with_company_scope(
            Some(req.company_id),
            self.entries.claim_accrual(&self.pool, &NewAccrualRow {
                company_id: req.company_id,
                loyalty_program_id: req.loyalty_program_id,
                customer_id: req.customer_id,
                points,
                purchase_amount: money(req.purchase_amount),
                source_type: &req.source_type,
                source_id: req.source_id,
                at: req.at,
                expiry,
            }),
        )
        .await?;

        match row {
            Some(entry_id) => {
                sink.publish(&PromoEvent::LoyaltyPointsEarned(LoyaltyPointsEarned {
                    entry_id,
                    loyalty_program_id: req.loyalty_program_id,
                    company_id: req.company_id,
                    customer_id: req.customer_id,
                    points,
                    purchase_amount: money(req.purchase_amount),
                    source_type: req.source_type.clone(),
                    source_id: req.source_id,
                }));
                Ok(AccrualOutcome { entry_id: Some(entry_id), points, already: false })
            }
            None => Ok(AccrualOutcome { entry_id: None, points, already: true }),
        }
    }

    /// Redeem points against a purchase. Serialized per member so concurrent redemptions can't
    /// oversell the balance; bounded by the available balance; idempotent per source.
    /// `discount_value = points · conversion_factor`.
    pub async fn redeem(
        &self,
        req: &RedemptionRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<RedemptionOutcome, PricingError> {
        if req.points <= Decimal::ZERO {
            return Err(PricingError::Invalid("points to redeem must be positive".into()));
        }
        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company on the redemption request — bind it so the advisory lock, the
        // balance read, and the redeemed-entry insert all run inside this tenant's fence.
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Serialize all balance-changing ops for this (company, customer, program).
        self.entries
            .lock_member_balance(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await?;

        // Idempotent replay: a prior redemption for this exact source returns the same result.
        if let Some(r) = self
            .entries
            .find_redemption_by_source(&mut tx, req.company_id, &req.source_type, req.source_id)
            .await?
        {
            let prior_points = r.points;
            let conversion_factor = self.program_conversion(&mut tx, req).await?;
            tx.commit().await?;
            return Ok(RedemptionOutcome {
                entry_id: r.id,
                points: -prior_points,
                discount_value: money(-prior_points * conversion_factor),
                already: true,
            });
        }

        let conversion_factor = self.program_conversion(&mut tx, req).await?;

        // Balance = Σ signed points (earned +, redeemed/expired −).
        let available = self
            .entries
            .available_balance(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await?;

        if req.points > available {
            return Err(PricingError::InsufficientPoints { available, requested: req.points });
        }

        let discount_value = money(req.points * conversion_factor);
        let entry_id = self.entries.insert_redemption(&mut tx, &NewRedemptionRow {
            company_id: req.company_id,
            loyalty_program_id: req.loyalty_program_id,
            customer_id: req.customer_id,
            points: -req.points,
            source_type: &req.source_type,
            source_id: req.source_id,
            at: req.at,
        })
        .await?;

        tx.commit().await?;
        sink.publish(&PromoEvent::LoyaltyPointsRedeemed(LoyaltyPointsRedeemed {
            entry_id,
            loyalty_program_id: req.loyalty_program_id,
            company_id: req.company_id,
            customer_id: req.customer_id,
            points: req.points,
            discount_value,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
        }));
        Ok(RedemptionOutcome { entry_id, points: req.points, discount_value, already: false })
    }

    /// Load an active, in-window program → (collection_factor, expiry_duration_days).
    async fn load_active_program(
        &self,
        company_id: Uuid,
        program_id: Uuid,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(Decimal, Option<i32>), PricingError> {
        // RLS scope (ADR-0008): company on the parameter — scope the read.
        company_scope::with_company_scope(
            Some(company_id),
            self.programs.find_active_collection(&self.pool, company_id, program_id, at),
        )
        .await?
        .ok_or(PricingError::ProgramInvalid)
    }

    /// The program's conversion_factor (currency per point), read inside a redemption tx.
    async fn program_conversion(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: &RedemptionRequest,
    ) -> Result<Decimal, PricingError> {
        self.programs
            .find_active_conversion(tx, req.company_id, req.loyalty_program_id, req.at)
            .await?
            .ok_or(PricingError::ProgramInvalid)
    }
}

/// A composable adapter so a caller holding a `PriceResolverPort` trait object drives `resolve`.
pub struct PromoPriceResolver {
    pub service: std::sync::Arc<PromoWriteService>,
}

#[async_trait::async_trait]
impl PriceResolverPort for PromoPriceResolver {
    async fn resolve(&self, query: &PriceQuery) -> Result<ResolvedPrice, PricingError> {
        self.service.resolve(query).await
    }

    async fn resolve_cart(&self, query: &CartQuery) -> Result<ResolvedCart, PricingError> {
        self.service.resolve_cart(query).await
    }
}
