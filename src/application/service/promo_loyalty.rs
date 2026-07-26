//! Loyalty ledger — accrue + redeem (hand-authored, user-owned).
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]. Points
//! accrue idempotently per source (one document earns at most once, however many times the paid
//! event is replayed); redemptions serialize per member and are bounded by the available balance.
//! `discount_value = points · conversion_factor`. Points are whole (floored on accrual) and are NOT
//! money.
//!
//! Per the module's 4-layer rule this file holds no SQL — the accrual claim, the advisory lock, the
//! balance read, and the redemption insert live on `LoyaltyPointEntryRepository`, and the
//! program/collection/conversion reads on `LoyaltyProgramRepository`. Every tx-taking repo method
//! rides the bind this service makes.

use backbone_orm::company_scope;
use rust_decimal::Decimal;

use crate::infrastructure::persistence::{NewAccrualRow, NewRedemptionRow};

use super::promo_events::{LoyaltyPointsEarned, LoyaltyPointsRedeemed, PromoEvent, PromoEventSink};
use super::promo_ports::{AccrualRequest, PricingError, RedemptionRequest};
use super::promo_write_service::{money, AccrualOutcome, PromoWriteService, RedemptionOutcome};

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
