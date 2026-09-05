//! Coupon redemption — the bounded WRITE (hand-authored, user-owned).
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]: consume
//! one use of a coupon when a sale commits. Atomic, bounded, AND idempotent per source:
//!
//!   * **bounded** — the guarded increment makes `used_count` impossible to advance past `max_use`,
//!     even under concurrent redemptions (→ `CouponExhausted`).
//!   * **idempotent** — a `coupon_redemptions` ledger row keyed by `(company, coupon, source)`
//!     records WHICH document consumed the use. A retry of the same sale (a dropped ack, an
//!     at-least-once event) finds the existing row and returns the same result WITHOUT a second
//!     burn — the same partial-unique pattern the loyalty accrual leg uses.
//!
//! Per the module's 4-layer rule this file holds no SQL — the ledger claim and the guarded counter
//! bump live on `CouponRedemptionRepository` / `CouponCodeRepository`, and both repo methods take
//! THIS service's transaction so the claim and the burn commit together.

use backbone_orm::company_scope;
use uuid::Uuid;

use super::promo_events::{CouponRedeemed, PromoEvent, PromoEventSink};
use super::promo_ports::{LockResource, PricingError};
use super::promo_write_service::PromoWriteService;

impl PromoWriteService {
    // ---- 2. coupon redemption (bounded write) -------------------------------------------------

    /// Consume one use of a coupon when a sale commits. Atomic, bounded, AND idempotent per source:
    ///   * **bounded** — the guarded increment makes `used_count` impossible to advance past
    ///     `max_use`, even under concurrent redemptions (→ `CouponExhausted`).
    ///   * **idempotent** — a `coupon_redemptions` ledger row keyed by `(company, coupon, source)`
    ///     records WHICH document consumed the use. A retry of the same sale (a dropped ack, an
    ///     at-least-once event) finds the existing row and returns the same result WITHOUT a second
    ///     burn — the same partial-unique pattern the loyalty accrual leg uses.
    ///
    /// The coupon row is locked `FOR UPDATE NOWAIT` FIRST — before any write — so two sales burning
    /// the same coupon serialize on the row instead of racing the guarded bump (a lost lock maps to
    /// [`PricingError::LockBusy`]; host contract: 409, `Retry-After: 1`, retry once). This is the
    /// ONLY lock the coupon verb takes, so it never cycles against the loyalty verbs'
    /// program-row → anchor order.
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
        // row lock, the ledger claim, and the guarded counter bump all pass the `app.company_id` fence.
        company_scope::bind_company_on(&mut tx, company_id).await?;

        // Lock the coupon row first (NOWAIT). Ok(false) = no active row → fall through to the claim,
        // which refuses unknown coupons and lets the bump refuse exhausted ones — the typed
        // CouponInvalid / CouponExhausted semantics are unchanged.
        self.coupons
            .lock_for_burn(&mut tx, coupon_id, company_id)
            .await
            .map_err(|e| super::promo_write_service::lock_busy_or_db(e, LockResource::CouponCode))?;

        // Idempotency gate: claim this (coupon, source) exactly once. ON CONFLICT → already redeemed.
        let claimed = self
            .redemptions
            .claim(&mut tx, company_id, coupon_id, source_type, source_id)
            .await?;

        // Settle the cart-stage claim minted under THIS document ref (the claim-and-burn-under-
        // the-same-ref contract): claimed → redeemed, inside this transaction, so the flip is
        // atomic with the burn. Idempotent by its `status = 'claimed'` guard — a replayed burn
        // finds nothing to settle, and a burn with no prior claim (the plain selling path)
        // touches zero rows. On the exhausted refusal below, the rolled-back transaction
        // discards this settle with everything else. `settled_at` is the database clock: the
        // burn verb takes no caller instant, and the commit instant IS the settle instant.
        self.claims
            .settle_redeemed(&mut tx, company_id, coupon_id, source_type, source_id)
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
}
