//! Single-line price resolution (hand-authored, user-owned).
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]: the
//! marquee READ. Given a line's dimensions + optional coupon, deterministically pick the winning
//! pricing rule (priority DESC, then specificity, then newest) and return the effective unit price.
//! Selling/POS consume this via `PriceResolverPort`; resolve NEVER mutates (previewing a price must
//! not consume a coupon).
//!
//! Per the module's 4-layer rule this file holds no SQL — the candidate search and the coupon lookup
//! live on `PricingRuleRepository` / `CouponCodeRepository`, scoped by the company on the query.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::LineRuleQuery;

use super::promo_ports::{PriceQuery, PricingError, ResolvedPrice};
use super::promo_write_service::{money, PromoWriteService};

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
    discount_upto: Option<Decimal>,
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

impl PromoWriteService {
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
                discount_upto: r.discount_upto,
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

        let base_unit = self.apply_effect(win, q.list_price);
        let mut discount = (q.list_price - base_unit).max(Decimal::ZERO);
        // discount_upto caps the line's TOTAL discount (per-unit × qty); when it binds, back-compute
        // the per-unit discount so ResolvedPrice stays per-unit while honoring an Rp ceiling on the line.
        if let Some(cap) = win.discount_upto {
            let line_total_disc = discount * q.quantity;
            if line_total_disc > cap {
                discount = money(cap / q.quantity);
            }
        }
        let unit_price = (q.list_price - discount).max(Decimal::ZERO);
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
    pub(super) async fn lookup_valid_coupon(
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
}
