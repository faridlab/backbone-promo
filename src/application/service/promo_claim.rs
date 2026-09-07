//! Cart-stage code claims (hand-authored, user-owned) — the claim-serialization surface.
//!
//! An `impl PromoWriteService` chunk over the vocabulary in [`super::promo_write_service`]:
//! adjudicate a shopper-presented promo code onto a cart, SERVER-SIDE and SERIALIZED.
//!
//! **Adjudicated server-side:** the request carries the typed string and the claiming
//! document's ref — nothing else. Which coupon and rule the string resolves to, whether the
//! code is active and in-window, and how much usage headroom remains are all derived inside
//! the verb; no discount, rule id, or count is ever client-supplied.
//!
//! **Serialized on the substrate that already exists:** the claim transaction takes the coupon
//! row `FOR UPDATE NOWAIT` — the SAME first lock `commit_coupon_redemption` takes — then reads
//! `used_count` and the active-claim count under it. Headroom is `used_count + active claims`
//! against `max_use`, so two carts racing the last use of a capped code serialize: the loser
//! maps to [`PricingError::LockBusy`] and, after the standing retry-once-with-jitter host
//! contract, reads the winner's reservation and is refused. A lost lock is transient
//! contention, never a deadlock: the claim transaction takes exactly {coupon}, like the burn.
//! The cart's existing claim is decided BEFORE the headroom count — a replay consumes no
//! unit, so the cart holding the last unit of a capped code re-claims its own code as an
//! idempotent replay (`already: true`) rather than being refused by its own reservation.
//!
//! **One uniform refusal:** every adjudication failure (unknown code, inactive, out of window,
//! exhausted, headroom consumed, cart slot occupied) is [`PricingError::ClaimRefused`] — one
//! variant, no per-state strings, so a public claim route cannot double as a code-enumeration
//! oracle. The refusal carries no detail about WHY.
//!
//! **Durable + inspectable:** the claim is a row in `promo.coupon_claims` (one ACTIVE claim
//! per cart, a partial unique index — the database is the bound, not a check-then-act read);
//! [`Self::claims_for_cart`] reads the history back. The claim never burns — `used_count`
//! advances only at commit, and the burn settles the claim (same document ref) inside its own
//! transaction, so the flip is atomic with the burn.
//!
//! **No second loyalty claim path:** a loyalty reward claim is the EXISTING serialized burn
//! (`redeem` / the per-order spend leg) — program→member-anchor lock order, balance-bounded,
//! idempotent per source, durable in the ledger. This chunk adds no loyalty write.
//!
//! Per the module's 4-layer rule this file holds no SQL — the locked adjudication read, the
//! claim insert, the settle, the release, and the history read live on
//! [`crate::infrastructure::persistence::CouponClaimRepository`].

use backbone_orm::company_scope;
use uuid::Uuid;

use super::promo_events::{PromoCodeClaimed, PromoCodeClaimReleased, PromoEvent, PromoEventSink};
use super::promo_ports::{
    CodeClaimView, CouponClaimStatus, LockResource, PricingError, PromoCodeClaimOutcome,
    PromoCodeClaimRequest,
};
use super::promo_write_service::{lock_busy_or_db, PromoWriteService};

impl PromoWriteService {
    // ---- 6. cart-stage code claims -------------------------------------------------------------

    /// Claim a promo code onto a cart: resolve the typed string server-side, take the coupon
    /// row lock, decide the cart's existing claim FIRST, and only for a cart holding nothing
    /// count headroom (`used_count + active claims` vs `max_use`) and mint the claim — exactly
    /// one active claim per cart, idempotent when this cart re-claims the code it already
    /// holds. The ordering is the contract: a replay consumes no further headroom, so the
    /// same-cart branch is decided BEFORE the headroom count — otherwise a cart holding the
    /// LAST unit of a capped code would be refused by its own reservation when it re-typed
    /// its own code.
    ///
    /// One return path per branch; no fall-through anywhere (a branch that has decided always
    /// returns). Every adjudication refusal is the uniform [`PricingError::ClaimRefused`];
    /// lock contention is [`PricingError::LockBusy`] (retry once after jitter, per the
    /// standing host contract).
    ///
    /// Claim and burn under the SAME document ref: `commit_coupon_redemption` settles the
    /// claim when it commits.
    pub async fn claim_promo_code(
        &self,
        req: &PromoCodeClaimRequest,
        sink: &dyn PromoEventSink,
    ) -> Result<PromoCodeClaimOutcome, PricingError> {
        if req.cart_ref_type.is_empty() || req.cart_ref_type.len() > 40 {
            return Err(PricingError::Invalid("cart_ref_type must be 1..=40 chars".into()));
        }
        // The shopper's input is normalized HERE — never trusted to arrive canonical.
        let code = req.code.trim().to_uppercase();
        if code.is_empty() || code.len() > 40 {
            // Uniform: an empty or oversized string is a refused claim, indistinguishable from
            // an unknown code on the wire.
            return Err(PricingError::ClaimRefused);
        }

        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company on the request — bind it so the locked adjudication
        // read and the claim insert both run inside this tenant's fence.
        company_scope::bind_company_on(&mut tx, req.company_id).await?;

        // Resolve + lock in ONE statement: the coupon row an active, in-window,
        // not-exhausted-at-rest claim can rest on, with its counters read UNDER the lock.
        // Ok(None) = refused (unknown / inactive / out of window / exhausted at rest) — one
        // uniform variant, no distinguishing detail.
        let coupon = self
            .claims
            .lock_claimable(&mut tx, req.company_id, &code, req.at)
            .await
            .map_err(|e| lock_busy_or_db(e, LockResource::CouponCode))?
            .ok_or(PricingError::ClaimRefused)?;

        // The cart's existing active claim decides the branch FIRST — before the headroom
        // count. A replay mints nothing and consumes no unit, so it must not be measured
        // against the cap: a cart holding the LAST unit of a capped code re-claims its own
        // code as a replay, not as a headroom contender.
        //   same coupon  → idempotent replay (the SAME outcome, no second row);
        //   other coupon → refused (release first — one active claim per cart);
        //   none         → mint, gated by headroom, with the partial unique index as the
        //                  race backstop.
        match self
            .claims
            .find_active_for_cart(&mut tx, req.company_id, &req.cart_ref_type, req.cart_ref_id)
            .await?
        {
            Some(active) if active.coupon_id == coupon.coupon_id => {
                // Idempotent replay: commit the read-only transaction and hand back the claim
                // this cart already holds. `already` is the only difference from a fresh win.
                tx.commit().await?;
                Ok(PromoCodeClaimOutcome {
                    claim_id: active.claim_id,
                    coupon_id: active.coupon_id,
                    pricing_rule_id: active.pricing_rule_id,
                    code: active.code,
                    already: true,
                })
            }
            Some(_) => Err(PricingError::ClaimRefused),
            None => {
                // Headroom under the lock, for a FRESH unit only: committed uses + live
                // reservations. A capped coupon with no unit left for THIS claim is refused
                // — uniformly, like every other state. (The replay arm above never reaches
                // this count — its unit is already reserved by this very cart.)
                if let Some(max_use) = coupon.max_use {
                    let reserved = self
                        .claims
                        .count_active_for_coupon(&mut tx, req.company_id, coupon.coupon_id)
                        .await?;
                    if i64::from(coupon.used_count) + reserved + 1 > i64::from(max_use) {
                        return Err(PricingError::ClaimRefused);
                    }
                }
                let claim_id = self
                    .claims
                    .insert_claim(
                        &mut tx,
                        req.company_id,
                        &req.cart_ref_type,
                        req.cart_ref_id,
                        &coupon,
                        req.at,
                    )
                    .await;
                let claim_id = match claim_id {
                    Ok(id) => id,
                    // A same-cart racer won the active-claim slot (the partial unique index).
                    // Exactly this code is the uniform refusal; the transaction rolls back
                    // with nothing else written.
                    Err(e) => {
                        return Err(unique_violation_or(e, PricingError::ClaimRefused));
                    }
                };
                tx.commit().await?;
                sink.publish(&PromoEvent::PromoCodeClaimed(PromoCodeClaimed {
                    claim_id,
                    company_id: req.company_id,
                    coupon_id: coupon.coupon_id,
                    pricing_rule_id: coupon.pricing_rule_id,
                    cart_ref_type: req.cart_ref_type.clone(),
                    cart_ref_id: req.cart_ref_id,
                    code: coupon.code.clone(),
                }));
                Ok(PromoCodeClaimOutcome {
                    claim_id,
                    coupon_id: coupon.coupon_id,
                    pricing_rule_id: coupon.pricing_rule_id,
                    code: coupon.code,
                    already: false,
                })
            }
        }
    }

    /// Release the cart's active claim (the shopper removed the code, or the cart is being
    /// abandoned). Frees the cart's code slot and the claim's unit of usage headroom
    /// immediately — the explicit, now-shaped answer to the abandoned-code lockout, not a
    /// delayed sweep. Idempotent: `Ok(None)` when the cart holds nothing active.
    pub async fn release_promo_claim(
        &self,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
        at: chrono::DateTime<chrono::Utc>,
        sink: &dyn PromoEventSink,
    ) -> Result<Option<Uuid>, PricingError> {
        let mut tx = self.pool.begin().await?;
        // RLS scope (ADR-0008): company on the parameter — bind it so the transition runs
        // inside this tenant's fence.
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let released = self
            .claims
            .release_for_cart(&mut tx, company_id, cart_ref_type, cart_ref_id, at)
            .await?;
        tx.commit().await?;
        if let Some(claim_id) = released {
            // The coupon id rides the event so consumers can recompute headroom without a join.
            // The read runs inside a company scope: the claims table is RLS-fenced, the pool
            // carries no company binding on the public cart path, and the scoped-execute
            // helper only fences when a scope is present — without this wrap the lookup runs
            // raw, sees zero rows, and the release surfaces as a refusal.
            let coupon_id = company_scope::with_company_scope(
                Some(company_id),
                self.claims
                    .coupon_of_claim(&self.pool, company_id, claim_id),
            )
            .await?;
            sink.publish(&PromoEvent::PromoCodeClaimReleased(PromoCodeClaimReleased {
                claim_id,
                company_id,
                coupon_id,
                cart_ref_type: cart_ref_type.to_string(),
                cart_ref_id,
            }));
        }
        Ok(released)
    }

    /// The cart's claim history, every status, oldest first — the inspectable claim state.
    /// A pure read: `with_company_scope` fences it, the verbs stay the only writers.
    pub async fn claims_for_cart(
        &self,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
    ) -> Result<Vec<CodeClaimView>, PricingError> {
        let rows = company_scope::with_company_scope(
            Some(company_id),
            self.claims.list_for_cart(&self.pool, company_id, cart_ref_type, cart_ref_id),
        )
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| CodeClaimView {
                claim_id: r.claim_id,
                coupon_id: r.coupon_id,
                pricing_rule_id: r.pricing_rule_id,
                code: r.code,
                status: match r.status {
                    "released" => CouponClaimStatus::Released,
                    "redeemed" => CouponClaimStatus::Redeemed,
                    _ => CouponClaimStatus::Claimed,
                },
                claimed_at: r.claimed_at,
                settled_at: r.settled_at,
            })
            .collect())
    }
}

/// Map exactly the unique-violation SQLSTATE (23505) to `fallback`; every other database error
/// flows through as [`PricingError::Db`] untouched — nothing is swallowed, and no other code is
/// remapped (the same one-code discipline as `lock_busy_or_db`).
fn unique_violation_or(err: sqlx::Error, fallback: PricingError) -> PricingError {
    if let sqlx::Error::Database(ref db) = err {
        if db.code().as_deref() == Some("23505") {
            return fallback;
        }
    }
    PricingError::Db(err)
}
