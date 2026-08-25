//! Loyalty ledger — accrue + redeem (hand-authored, user-owned).
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]. Points
//! accrue idempotently per source (one document earns at most once, however many times the paid
//! event is replayed); redemptions serialize per member and are bounded by the expiry-aware
//! available balance. `discount_value = points · conversion_factor`. Points are whole (floored on
//! accrual) and are NOT money.
//!
//! Redeem-time expiry (no cron, no sweep): the balance read is `(available, lapsed)` at the
//! redemption instant — `available` counts only entries whose `expiry_date` is NULL or still in the
//! future, `lapsed` the rest. A refusal distinguishes the two causes: lapsed points that would have
//! covered the ask → [`PricingError::PointsExpired`] ("your points expired"), otherwise →
//! [`PricingError::InsufficientPoints`]. The `expired` ledger entry type stays reserved for a
//! future sweep; nothing here writes it.
//!
//! Per the module's 4-layer rule this file holds no SQL — the accrual claim, the member anchor
//! lock, the expiry-aware balance read, and the redemption insert live on
//! `LoyaltyPointEntryRepository`, the member-anchor mint on `LoyaltyMemberAnchorRepository`, and
//! the program/collection/conversion reads on `LoyaltyProgramRepository`. Every tx-taking repo
//! method rides the bind this service makes.

use backbone_orm::company_scope;
use rust_decimal::Decimal;

use crate::infrastructure::persistence::{NewAccrualRow, NewRedemptionRow};

use super::promo_events::{LoyaltyPointsEarned, LoyaltyPointsRedeemed, PromoEvent, PromoEventSink};
use super::promo_ports::{AccrualRequest, LockResource, PricingError, RedemptionRequest};
use super::promo_write_service::{lock_busy_or_db, money, AccrualOutcome, PromoWriteService, RedemptionOutcome};

impl PromoWriteService {
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
        // WITH CHECK fence (accrue is event-driven and has no ambient scope of its own).
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
    /// oversell the balance; bounded by the expiry-aware available balance; idempotent per source.
    /// `discount_value = points · conversion_factor`.
    ///
    /// Lock order (fixed): the program row FIRST (`FOR UPDATE NOWAIT`, pins the conversion factor
    /// against admin edits), then the member anchor (`FOR UPDATE NOWAIT`, serializes every
    /// balance-changing op for this member). A lost lock surfaces as [`PricingError::LockBusy`]
    /// (host contract: 409, `Retry-After: 1`, retry once); everything else flows through as `Db`.
    pub async fn redeem(
        &self,
        req: &RedemptionRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<RedemptionOutcome, PricingError> {
        if req.points <= Decimal::ZERO {
            return Err(PricingError::Invalid("points to redeem must be positive".into()));
        }
        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company on the redemption request — bind it so the anchor lock, the
        // balance read, and the redeemed-entry insert all run inside this tenant's fence.
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Lock 1 — program row (NOWAIT): the factor that prices this burn is the one the serialized
        // balance check sees, pinned for the length of the transaction.
        let conversion_factor = self
            .program_conversion(&mut tx, req)
            .await?;

        // Lock 2 — member anchor (NOWAIT): serialize all balance-changing ops for this
        // (company, customer, program). The anchor row is minted on first touch, so even a member
        // with no ledger history yet gets a lock target.
        self.anchors
            .ensure_and_lock(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::MemberBalance))?;

        // Idempotent replay: a prior redemption for this exact source returns the same result.
        if let Some(r) = self
            .entries
            .find_redemption_by_source(&mut tx, req.company_id, &req.source_type, req.source_id)
            .await?
        {
            let prior_points = r.points;
            tx.commit().await?;
            return Ok(RedemptionOutcome {
                entry_id: r.id,
                points: -prior_points,
                discount_value: money(-prior_points * conversion_factor),
                already: true,
            });
        }

        // Expiry-aware balance at the redemption instant: available = not-yet-lapsed Σ, lapsed = Σ
        // of entries whose expiry has passed. Read under the member lock, so it cannot be raced.
        let (available, lapsed) = self
            .entries
            .balances_at(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id, req.at)
            .await?;

        if req.points > available {
            if available + lapsed >= req.points {
                // The member HAD these points — they lapsed. Say so instead of "insufficient".
                return Err(PricingError::PointsExpired { lapsed, available });
            }
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

    /// Load an active, in-window program → (collection_factor, expiry_duration_days). Shared by the
    /// loyalty chunks (the per-order grant verb rides the same read).
    pub(super) async fn load_active_program(
        &self,
        company_id: uuid::Uuid,
        program_id: uuid::Uuid,
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

    /// The program's conversion_factor (currency per point), locked `FOR UPDATE NOWAIT` inside the
    /// redemption tx. Lock contention (SQLSTATE 55P03) maps to [`PricingError::LockBusy`]; a missing
    /// or out-of-window program maps to [`PricingError::ProgramInvalid`].
    async fn program_conversion(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: &RedemptionRequest,
    ) -> Result<Decimal, PricingError> {
        self.programs
            .find_active_conversion(tx, req.company_id, req.loyalty_program_id, req.at)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::LoyaltyProgram))?
            .ok_or(PricingError::ProgramInvalid)
    }
}
