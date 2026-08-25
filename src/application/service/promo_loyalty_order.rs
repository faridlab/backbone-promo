//! Per-order loyalty accounting (hand-authored, user-owned) — the order-flow verbs.
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]: the
//! compose surface a host (POS / selling) drives at order confirm (`grant_order_points`), payment
//! (`spend_order_points`) and return (`reverse_order_points`). Server-authoritative throughout
//! (PSX-5): requests carry order FACTS — amounts, refs, customer intent — and the server derives
//! every point delta (`floor(base · collection_factor)` / reversal proportions) and every money
//! value (`points · conversion_factor`). A request never carries a point delta to write.
//!
//! Every verb is idempotent (per order row / per entry partial-unique / per return document — a
//! re-driven confirm, payment or return is a no-op returning the same outcome) and serialized per
//! member through the anchor lock (see [`super::promo_loyalty_order`] lock order: program row
//! first, then member anchor; the coupon verb locks only the coupon row — no cycle).
//!
//! Reversals are COMPENSATING ledger entries, never a status flip: the entry ledger is an
//! append-only signed SUM, each compensating row carries its own source document (the return), and
//! partial returns are amounts — expressible only as proportional legs, not flips.
//!
//! Per the module's 4-layer rule this file holds no SQL — the order-row upserts and counter
//! updates live on `LoyaltyOrderPointsRepository`, the ledger legs on `LoyaltyPointEntryRepository`,
//! the member lock on `LoyaltyMemberAnchorRepository`, and the program reads on
//! `LoyaltyProgramRepository`.

use backbone_orm::company_scope;
use rust_decimal::Decimal;

use crate::infrastructure::persistence::{NewAccrualRow, NewRedemptionRow, NewReversalRow};

use super::promo_events::{
    LoyaltyOrderPointsGranted, LoyaltyOrderPointsReversed, LoyaltyOrderPointsSpent, PromoEvent,
    PromoEventSink,
};
use super::promo_ports::{
    LockResource, OrderPointsGrantOutcome, OrderPointsGrantRequest, OrderPointsReversalOutcome,
    OrderPointsReversalRequest, OrderPointsSpendOutcome, OrderPointsSpendRequest, PricingError,
};
use super::promo_write_service::{lock_busy_or_db, money, PromoWriteService};

impl PromoWriteService {
    // ---- 5. per-order loyalty accounting -------------------------------------------------------

    /// Grant (earn) a member's points for one logical order, at order confirm. Re-drive safe: the
    /// order row's partial-unique key and the ledger entry's `(company, source, entry_type)` key
    /// together make a replayed confirm a no-op returning the same outcome. Points are DERIVED:
    /// `floor(grant_base_amount · collection_factor)` — a zero result writes nothing (matching
    /// `accrue`'s zero behavior). Cross-legacy defense: a host that also drove the bare `accrue`
    /// with the same source key dedupes on the entry partial-unique.
    pub async fn grant_order_points(
        &self,
        req: &OrderPointsGrantRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<OrderPointsGrantOutcome, PricingError> {
        if req.grant_base_amount < Decimal::ZERO {
            return Err(PricingError::Invalid("grant_base_amount must be non-negative".into()));
        }
        if req.order_ref_type.is_empty() || req.order_ref_type.len() > 40 {
            return Err(PricingError::Invalid("order_ref_type must be 1..=40 chars".into()));
        }
        let (collection_factor, expiry_days) =
            self.load_active_program(req.company_id, req.loyalty_program_id, req.at).await?;

        let points = (req.grant_base_amount * collection_factor).floor();
        if points <= Decimal::ZERO {
            return Ok(OrderPointsGrantOutcome {
                order_points_id: None,
                entry_id: None,
                points: Decimal::ZERO,
                already: false,
            });
        }
        let expiry = expiry_days.map(|d| req.at + chrono::Duration::days(d as i64));

        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company on the request — bind it so the anchor mint, the ledger
        // claim, and the order-row upsert all run inside this tenant's fence.
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Member serialization: the anchor exists/was locked before any balance-affecting write.
        self.anchors
            .ensure_and_lock(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::MemberBalance))?;

        // Order row: born with its grant leg; a conflict means this order already accounted for.
        let inserted = self
            .order_points
            .insert_grant(&mut tx, &crate::infrastructure::persistence::NewOrderGrantRow {
                company_id: req.company_id,
                loyalty_program_id: req.loyalty_program_id,
                customer_id: req.customer_id,
                order_ref_type: &req.order_ref_type,
                order_ref_id: req.order_ref_id,
                coupon_code_id: req.coupon_code_id,
                grant_base_amount: money(req.grant_base_amount),
                granted_points: points,
                granted_at: req.at,
            })
            .await?;

        // Ledger leg: the same partial-unique idempotency `accrue` rides.
        let entry_id = self
            .entries
            .claim_accrual_on(&mut tx, &NewAccrualRow {
                company_id: req.company_id,
                loyalty_program_id: req.loyalty_program_id,
                customer_id: req.customer_id,
                points,
                purchase_amount: money(req.grant_base_amount),
                source_type: &req.order_ref_type,
                source_id: req.order_ref_id,
                at: req.at,
                expiry,
            })
            .await?;

        let (order_points_id, already, stored_points) = match (inserted, entry_id) {
            // Fresh row + fresh entry: the plain grant.
            (Some(id), Some(_)) => (id, false, points),
            // Fresh row, pre-existing entry: a legacy bare accrue already earned for this source —
            // the row records the grant, but nothing new was written to the ledger.
            (Some(id), None) => (id, true, points),
            // Pre-existing row, fresh entry: a spend-first order earning late — set the grant leg.
            (None, Some(_)) => {
                let row = self
                    .order_points
                    .find_by_order_ref(
                        &mut tx,
                        req.company_id,
                        req.loyalty_program_id,
                        &req.order_ref_type,
                        req.order_ref_id,
                    )
                    .await?
                    .ok_or(PricingError::Invalid("order points row vanished mid-grant".into()))?;
                self.order_points
                    .set_granted(
                        &mut tx,
                        req.company_id,
                        req.loyalty_program_id,
                        &req.order_ref_type,
                        req.order_ref_id,
                        money(req.grant_base_amount),
                        points,
                        req.at,
                    )
                    .await?;
                (row.id, false, points)
            }
            // Pre-existing row + pre-existing entry: the full replay.
            (None, None) => {
                let row = self
                    .order_points
                    .find_by_order_ref(
                        &mut tx,
                        req.company_id,
                        req.loyalty_program_id,
                        &req.order_ref_type,
                        req.order_ref_id,
                    )
                    .await?
                    .ok_or(PricingError::Invalid("order points row vanished mid-grant".into()))?;
                (row.id, true, row.granted_points)
            }
        };

        tx.commit().await?;
        if entry_id.is_some() {
            sink.publish(&PromoEvent::LoyaltyOrderPointsGranted(LoyaltyOrderPointsGranted {
                order_points_id,
                loyalty_program_id: req.loyalty_program_id,
                company_id: req.company_id,
                customer_id: req.customer_id,
                order_ref_type: req.order_ref_type.clone(),
                order_ref_id: req.order_ref_id,
                points: stored_points,
                grant_base_amount: money(req.grant_base_amount),
            }));
        }
        Ok(OrderPointsGrantOutcome {
            order_points_id: Some(order_points_id),
            entry_id,
            points: stored_points,
            already,
        })
    }

    /// Spend (redeem) a member's points against one logical order, at payment/commit. `points` is
    /// the customer's INTENT: the server bounds it by the expiry-aware available balance and derives
    /// the money value. Lock order: program row (NOWAIT) → member anchor (NOWAIT) — same order as
    /// `redeem`, so every balance writer serializes identically. A lost lock surfaces as
    /// [`PricingError::LockBusy`] (host contract: 409, retry once after jitter); a balance refusal
    /// is typed — [`PricingError::PointsExpired`] when lapsed points would have covered the ask,
    /// [`PricingError::InsufficientPoints`] when they would not.
    pub async fn spend_order_points(
        &self,
        req: &OrderPointsSpendRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<OrderPointsSpendOutcome, PricingError> {
        if req.points <= Decimal::ZERO {
            return Err(PricingError::Invalid("points to spend must be positive".into()));
        }
        if req.order_ref_type.is_empty() || req.order_ref_type.len() > 40 {
            return Err(PricingError::Invalid("order_ref_type must be 1..=40 chars".into()));
        }
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Lock order: the program row pins conversion_factor against admin edits (fail-fast over
        // blocking); the anchor serializes every balance writer for this member.
        let conversion_factor = self
            .programs
            .find_active_conversion(&mut tx, req.company_id, req.loyalty_program_id, req.at)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::LoyaltyProgram))?
            .ok_or(PricingError::ProgramInvalid)?;

        self.anchors
            .ensure_and_lock(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::MemberBalance))?;

        // Idempotent replay: this order already spent — return the stored spend.
        if let Some(prior) = self
            .entries
            .find_redemption_by_source(&mut tx, req.company_id, &req.order_ref_type, req.order_ref_id)
            .await?
        {
            let points = -prior.points;
            let available_after = self
                .entries
                .balances_at(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id, req.at)
                .await?
                .0;
            tx.commit().await?;
            return Ok(OrderPointsSpendOutcome {
                entry_id: prior.id,
                points,
                discount_value: money(points * conversion_factor),
                available_after,
                already: true,
            });
        }

        // Expiry-aware balance, under the member lock.
        let (available, lapsed) = self
            .entries
            .balances_at(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id, req.at)
            .await?;
        if req.points > available {
            if available + lapsed >= req.points {
                return Err(PricingError::PointsExpired { lapsed, available });
            }
            return Err(PricingError::InsufficientPoints { available, requested: req.points });
        }

        let discount_value = money(req.points * conversion_factor);
        let entry_id = self
            .entries
            .insert_redemption(&mut tx, &NewRedemptionRow {
                company_id: req.company_id,
                loyalty_program_id: req.loyalty_program_id,
                customer_id: req.customer_id,
                points: -req.points,
                source_type: &req.order_ref_type,
                source_id: req.order_ref_id,
                at: req.at,
            })
            .await?;

        // Order row spent leg: born with it when the order only spent (points earned elsewhere),
        // set on it when the order already carried a grant.
        let order_points_id = match self
            .order_points
            .insert_spend(&mut tx, &crate::infrastructure::persistence::NewOrderSpendRow {
                company_id: req.company_id,
                loyalty_program_id: req.loyalty_program_id,
                customer_id: req.customer_id,
                order_ref_type: &req.order_ref_type,
                order_ref_id: req.order_ref_id,
                spent_points: req.points,
                spent_at: req.at,
            })
            .await?
        {
            Some(id) => id,
            None => {
                let row = self
                    .order_points
                    .find_by_order_ref(
                        &mut tx,
                        req.company_id,
                        req.loyalty_program_id,
                        &req.order_ref_type,
                        req.order_ref_id,
                    )
                    .await?
                    .ok_or(PricingError::Invalid("order points row vanished mid-spend".into()))?;
                self.order_points
                    .set_spent(
                        &mut tx,
                        req.company_id,
                        req.loyalty_program_id,
                        &req.order_ref_type,
                        req.order_ref_id,
                        req.points,
                        req.at,
                    )
                    .await?;
                row.id
            }
        };

        let available_after = self
            .entries
            .balances_at(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id, req.at)
            .await?
            .0;

        tx.commit().await?;
        sink.publish(&PromoEvent::LoyaltyOrderPointsSpent(LoyaltyOrderPointsSpent {
            order_points_id,
            loyalty_program_id: req.loyalty_program_id,
            company_id: req.company_id,
            customer_id: req.customer_id,
            order_ref_type: req.order_ref_type.clone(),
            order_ref_id: req.order_ref_id,
            points: req.points,
            discount_value,
        }));
        Ok(OrderPointsSpendOutcome {
            entry_id,
            points: req.points,
            discount_value,
            available_after,
            already: false,
        })
    }

    /// Reverse (part of) one order's points against a RETURN document. `return_amount` `None` =
    /// full cancel; `Some(base)` = a partial return's earning base. Both legs are DERIVED and
    /// bounded server-side:
    ///
    /// * grant clawback `= min(remaining_granted, [floor(return_amount · collection_factor)],
    ///   available)` — the no-negative-balance policy: a clawback never drives a member negative,
    ///   so a full return after the member already SPENT the points claws back less than the grant
    ///   (an owner-visible business decision; the spend side still restores fully).
    /// * spend restoration `= min(remaining_spent, floor(remaining_spent · return_amount /
    ///   grant_base_amount))` — proportional to the returned share of the earning base (`0` when
    ///   the base is `0`).
    ///
    /// The DB bounds double as the hard stop: the order-row CHECKs refuse over-reversal, and the
    /// conservation trigger refuses any ledger write that would push the expiry-aware balance
    /// negative. Idempotent per RETURN document (each leg rides the entry partial-unique key).
    /// A spend restoration copies the order's original `earned` entry's `expiry_date` (NULL when
    /// the order never earned — the restored points simply never lapse).
    pub async fn reverse_order_points(
        &self,
        req: &OrderPointsReversalRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<OrderPointsReversalOutcome, PricingError> {
        if let Some(amount) = req.return_amount {
            if amount < Decimal::ZERO {
                return Err(PricingError::Invalid("return_amount must be non-negative".into()));
            }
        }
        if req.order_ref_type.is_empty() || req.order_ref_type.len() > 40 {
            return Err(PricingError::Invalid("order_ref_type must be 1..=40 chars".into()));
        }
        if req.reversal_ref_type.is_empty() || req.reversal_ref_type.len() > 40 {
            return Err(PricingError::Invalid("reversal_ref_type must be 1..=40 chars".into()));
        }
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Lock order: program row → member anchor (same as spend). Deliberately NOT
        // window-checked — a return months later must still reverse under a retired program.
        let (collection_factor, _) = self
            .programs
            .find_factors_locked(&mut tx, req.company_id, req.loyalty_program_id)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::LoyaltyProgram))?
            .ok_or(PricingError::ProgramInvalid)?;

        self.anchors
            .ensure_and_lock(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::MemberBalance))?;

        // Idempotent replay: this RETURN document already reversed — return its stored legs.
        if let Some(prior) = self
            .entries
            .find_reversal_by_source(&mut tx, req.company_id, &req.reversal_ref_type, req.reversal_ref_id)
            .await?
        {
            let row = self
                .order_points
                .find_by_order_ref(
                    &mut tx,
                    req.company_id,
                    req.loyalty_program_id,
                    &req.order_ref_type,
                    req.order_ref_id,
                )
                .await?
                .ok_or(PricingError::Invalid("no loyalty accounting for this order".into()))?;
            tx.commit().await?;
            return Ok(OrderPointsReversalOutcome {
                order_points_id: row.id,
                grant_reversed: prior.grant_reversed,
                spend_restored: prior.spend_reversed,
                already: true,
            });
        }

        let row = self
            .order_points
            .find_by_order_ref(
                &mut tx,
                req.company_id,
                req.loyalty_program_id,
                &req.order_ref_type,
                req.order_ref_id,
            )
            .await?
            .ok_or(PricingError::Invalid("no loyalty accounting for this order".into()))?;

        // Server-authoritative derivations.
        let remaining_granted = row.granted_points - row.granted_reversed_points;
        let remaining_spent = row.spent_points - row.spent_reversed_points;
        let (available, _) = self
            .entries
            .balances_at(&mut tx, req.company_id, req.customer_id, req.loyalty_program_id, req.at)
            .await?;
        let (grant_leg, spend_leg) = match req.return_amount {
            None => (remaining_granted.min(available), remaining_spent),
            Some(amount) => {
                let by_base = (amount * collection_factor).floor();
                let grant = remaining_granted.min(by_base).min(available);
                let spend = if row.grant_base_amount == Decimal::ZERO {
                    Decimal::ZERO
                } else {
                    (remaining_spent * amount / row.grant_base_amount).floor()
                };
                (grant, spend.min(remaining_spent))
            }
        };

        // Compensating ledger legs, each idempotent on its own entry-type key. Both legs copy
        // the original earned entry's expiry so a reversal lapses exactly when the entry it
        // reverses would — a NULL-expiry reversal would outlive the grant it negates and drag
        // the expiry-aware balance negative once the grant lapses. NULL only when this order
        // never earned (documented edge).
        let earned_expiry = self
            .entries
            .find_earned_expiry(&mut tx, req.company_id, &req.order_ref_type, req.order_ref_id)
            .await?;
        let mut wrote_any = false;
        if grant_leg > Decimal::ZERO {
            self.entries
                .insert_reversal(&mut tx, &NewReversalRow {
                    company_id: req.company_id,
                    loyalty_program_id: req.loyalty_program_id,
                    customer_id: req.customer_id,
                    entry_type: "grant_reversed",
                    points: -grant_leg,
                    source_type: &req.reversal_ref_type,
                    source_id: req.reversal_ref_id,
                    at: req.at,
                    expiry: earned_expiry,
                })
                .await?;
            wrote_any = true;
        }
        if spend_leg > Decimal::ZERO {
            self.entries
                .insert_reversal(&mut tx, &NewReversalRow {
                    company_id: req.company_id,
                    loyalty_program_id: req.loyalty_program_id,
                    customer_id: req.customer_id,
                    entry_type: "spend_reversed",
                    points: spend_leg,
                    source_type: &req.reversal_ref_type,
                    source_id: req.reversal_ref_id,
                    at: req.at,
                    expiry: earned_expiry,
                })
                .await?;
            wrote_any = true;
        }

        // Counter advance — the order-row CHECKs refuse over-reversal at DB level.
        if wrote_any {
            self.order_points
                .add_reversal_counters(
                    &mut tx,
                    req.company_id,
                    req.loyalty_program_id,
                    &req.order_ref_type,
                    req.order_ref_id,
                    grant_leg,
                    spend_leg,
                )
                .await?;
        }

        tx.commit().await?;
        if wrote_any {
            sink.publish(&PromoEvent::LoyaltyOrderPointsReversed(LoyaltyOrderPointsReversed {
                order_points_id: row.id,
                loyalty_program_id: req.loyalty_program_id,
                company_id: req.company_id,
                customer_id: req.customer_id,
                order_ref_type: req.order_ref_type.clone(),
                order_ref_id: req.order_ref_id,
                grant_reversed: grant_leg,
                spend_restored: spend_leg,
                reversal_ref_type: req.reversal_ref_type.clone(),
                reversal_ref_id: req.reversal_ref_id,
            }));
        }
        Ok(OrderPointsReversalOutcome {
            order_points_id: row.id,
            grant_reversed: grant_leg,
            spend_restored: spend_leg,
            already: false,
        })
    }
}
