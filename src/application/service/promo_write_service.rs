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

use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::promo_events::{
    CouponRedeemed, LoyaltyPointsEarned, LoyaltyPointsRedeemed, PromoEvent, PromoEventSink,
};
use super::promo_ports::{
    AccrualRequest, PriceQuery, PriceResolverPort, PricingError, RedemptionRequest, ResolvedPrice,
};

/// Round to 2dp, half away from zero (IDR money).
fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// The promo write service: a thin holder over the pool.
pub struct PromoWriteService {
    pool: PgPool,
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
        Self { pool }
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
        let rows = sqlx::query(
            r#"
            SELECT id, priority, apply_on::text AS apply_on, customer_id, customer_group_id,
                   coupon_required, rate_or_discount::text AS rate_or_discount,
                   rate, discount_percentage, discount_amount, valid_from
            FROM promo.pricing_rules
            WHERE company_id = $1
              AND is_active = true
              AND (metadata->>'deleted_at') IS NULL
              AND valid_from <= $2
              AND (valid_to IS NULL OR valid_to >= $2)
              AND (
                    apply_on = 'all'
                 OR (apply_on = 'item'       AND item_id = $3)
                 OR (apply_on = 'item_group' AND item_group_id = $4)
                 OR (apply_on = 'brand'      AND brand_id = $5)
              )
              AND (customer_id IS NULL OR customer_id = $6)
              AND (customer_group_id IS NULL OR customer_group_id = $7)
              AND min_qty <= $8
              AND (max_qty IS NULL OR max_qty >= $8)
              AND min_amount <= $9
            "#,
        )
        .bind(q.company_id)
        .bind(q.at)
        .bind(q.item_id)
        .bind(q.item_group_id)
        .bind(q.brand_id)
        .bind(q.customer_id)
        .bind(q.customer_group_id)
        .bind(q.quantity)
        .bind(gross)
        .fetch_all(&self.pool)
        .await?;

        let mut candidates: Vec<Candidate> = rows
            .into_iter()
            .map(|r| Candidate {
                id: r.get("id"),
                priority: r.get("priority"),
                apply_on: r.get("apply_on"),
                customer_id: r.get("customer_id"),
                customer_group_id: r.get("customer_group_id"),
                coupon_required: r.get("coupon_required"),
                rate_or_discount: r.get("rate_or_discount"),
                rate: r.get("rate"),
                discount_percentage: r.get("discount_percentage"),
                discount_amount: r.get("discount_amount"),
                valid_from: r.get("valid_from"),
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
        let row = sqlx::query(
            r#"
            SELECT id, pricing_rule_id
            FROM promo.coupon_codes
            WHERE company_id = $1
              AND code = $2
              AND is_active = true
              AND (metadata->>'deleted_at') IS NULL
              AND valid_from <= $3
              AND (valid_upto IS NULL OR valid_upto >= $3)
              AND (max_use IS NULL OR used_count < max_use)
            "#,
        )
        .bind(company_id)
        .bind(code.to_uppercase())
        .bind(at)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.get("id"), r.get("pricing_rule_id"))))
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

        // Idempotency gate: claim this (coupon, source) exactly once. ON CONFLICT → already redeemed.
        let claimed = sqlx::query(
            r#"
            INSERT INTO promo.coupon_redemptions (company_id, coupon_id, pricing_rule_id,
                source_type, source_id)
            SELECT $1, $2, c.pricing_rule_id, $3, $4
            FROM promo.coupon_codes c
            WHERE c.id = $2 AND c.company_id = $1
            ON CONFLICT (company_id, coupon_id, source_type, source_id)
                WHERE (metadata->>'deleted_at') IS NULL
            DO NOTHING
            RETURNING pricing_rule_id
            "#,
        )
        .bind(company_id)
        .bind(coupon_id)
        .bind(source_type)
        .bind(source_id)
        .fetch_optional(&mut *tx)
        .await?;

        let rule_id: Uuid = match claimed {
            // Fresh source: advance the counter, bounded. Exhausted → roll back the ledger claim.
            Some(r) => {
                let bumped = sqlx::query(
                    r#"
                    UPDATE promo.coupon_codes
                    SET used_count = used_count + 1
                    WHERE id = $1
                      AND company_id = $2
                      AND is_active = true
                      AND (metadata->>'deleted_at') IS NULL
                      AND (max_use IS NULL OR used_count < max_use)
                    RETURNING pricing_rule_id
                    "#,
                )
                .bind(coupon_id)
                .bind(company_id)
                .fetch_optional(&mut *tx)
                .await?;
                if bumped.is_none() {
                    // No use remained (or the coupon is inactive) — undo the ledger claim.
                    return Err(PricingError::CouponExhausted);
                }
                tx.commit().await?;
                let rule_id: Uuid = r.get("pricing_rule_id");
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
                let existing: Uuid = sqlx::query_scalar(
                    r#"SELECT pricing_rule_id FROM promo.coupon_redemptions
                       WHERE company_id = $1 AND coupon_id = $2 AND source_type = $3 AND source_id = $4
                         AND (metadata->>'deleted_at') IS NULL"#,
                )
                .bind(company_id)
                .bind(coupon_id)
                .bind(source_type)
                .bind(source_id)
                .fetch_one(&mut *tx)
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

        let row = sqlx::query(
            r#"
            INSERT INTO promo.loyalty_point_entries
                (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
                 source_type, source_id, posting_date, expiry_date)
            VALUES ($1, $2, $3, 'earned', $4, $5, $6, $7, $8, $9)
            ON CONFLICT (company_id, source_type, source_id, entry_type)
                WHERE (metadata->>'deleted_at') IS NULL
            DO NOTHING
            RETURNING id
            "#,
        )
        .bind(req.company_id)
        .bind(req.loyalty_program_id)
        .bind(req.customer_id)
        .bind(points)
        .bind(money(req.purchase_amount))
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(req.at)
        .bind(expiry)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let entry_id: Uuid = r.get("id");
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

        // Serialize all balance-changing ops for this (company, customer, program).
        let lock_key = format!("{}:{}:{}", req.company_id, req.customer_id, req.loyalty_program_id);
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&lock_key)
            .execute(&mut *tx)
            .await?;

        // Idempotent replay: a prior redemption for this exact source returns the same result.
        if let Some(r) = sqlx::query(
            r#"SELECT id, points FROM promo.loyalty_point_entries
               WHERE company_id = $1 AND source_type = $2 AND source_id = $3 AND entry_type = 'redeemed'
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(req.company_id)
        .bind(&req.source_type)
        .bind(req.source_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            let prior_points: Decimal = r.get("points");
            let conversion_factor = self.program_conversion(&mut tx, req).await?;
            tx.commit().await?;
            return Ok(RedemptionOutcome {
                entry_id: r.get("id"),
                points: -prior_points,
                discount_value: money(-prior_points * conversion_factor),
                already: true,
            });
        }

        let conversion_factor = self.program_conversion(&mut tx, req).await?;

        // Balance = Σ signed points (earned +, redeemed/expired −).
        let available: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(points), 0) FROM promo.loyalty_point_entries
               WHERE company_id = $1 AND customer_id = $2 AND loyalty_program_id = $3
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(req.company_id)
        .bind(req.customer_id)
        .bind(req.loyalty_program_id)
        .fetch_one(&mut *tx)
        .await?;

        if req.points > available {
            return Err(PricingError::InsufficientPoints { available, requested: req.points });
        }

        let discount_value = money(req.points * conversion_factor);
        let entry_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO promo.loyalty_point_entries
                (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
                 source_type, source_id, posting_date)
            VALUES ($1, $2, $3, 'redeemed', $4, 0, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(req.company_id)
        .bind(req.loyalty_program_id)
        .bind(req.customer_id)
        .bind(-req.points)
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(req.at)
        .fetch_one(&mut *tx)
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
        let row = sqlx::query(
            r#"SELECT collection_factor, expiry_duration_days FROM promo.loyalty_programs
               WHERE id = $1 AND company_id = $2 AND is_active = true
                 AND (metadata->>'deleted_at') IS NULL
                 AND from_date <= $3 AND (to_date IS NULL OR to_date >= $3)"#,
        )
        .bind(program_id)
        .bind(company_id)
        .bind(at)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PricingError::ProgramInvalid)?;
        Ok((row.get("collection_factor"), row.get("expiry_duration_days")))
    }

    /// The program's conversion_factor (currency per point), read inside a redemption tx.
    async fn program_conversion(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: &RedemptionRequest,
    ) -> Result<Decimal, PricingError> {
        sqlx::query_scalar(
            r#"SELECT conversion_factor FROM promo.loyalty_programs
               WHERE id = $1 AND company_id = $2 AND is_active = true
                 AND (metadata->>'deleted_at') IS NULL
                 AND from_date <= $3 AND (to_date IS NULL OR to_date >= $3)"#,
        )
        .bind(req.loyalty_program_id)
        .bind(req.company_id)
        .bind(req.at)
        .fetch_optional(&mut **tx)
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
}
