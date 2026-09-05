//! The cart-stage coupon claim state (hand-authored, user-owned).
//!
//! SQL for `promo.coupon_claims` — the RESERVATION a shopper's typed code mintes
//! on a cart, adjudicated server-side under the coupon row lock. The claim does
//! NOT burn: `used_count` advances only at commit (`commit_coupon_redemption`);
//! until then the claim just consumes usage headroom (counted as
//! `used_count + active claims`). One ACTIVE claim per cart is a partial unique
//! index, not a service-side check — the database is the bound.
//!
//! Unit-shaped on purpose: the mutating statements run on the CALLER'S
//! transaction (the claim serializes on the coupon row the caller locked first;
//! the settle runs inside the burn's transaction), and the inspecting read takes
//! the pool as a parameter — the `find_usable` precedent. The table is
//! RLS-fenced (ADR-0014 strict), so every tx method rides the caller's
//! already-bound connection and the read is company-scoped by the caller.
//!
//! Per the module's 4-layer rule the SQL lives here; the claim verbs in
//! `application/service/promo_claim.rs` orchestrate.

use sqlx::Row;
use uuid::Uuid;

/// The adjudication facts pinned under the coupon row lock: what the claim is
/// entitled to, and the counters headroom is computed from.
#[derive(Debug, Clone)]
pub struct ClaimableCouponRow {
    pub coupon_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub code: String,
    pub used_count: i32,
    pub max_use: Option<i32>,
}

/// The cart's currently-active claim, when one exists.
#[derive(Debug, Clone)]
pub struct ActiveClaimRow {
    pub claim_id: Uuid,
    pub coupon_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub code: String,
}

/// A claim row as stored (the inspectable history a host reads back).
#[derive(Debug, Clone)]
pub struct CouponClaimRow {
    pub claim_id: Uuid,
    pub coupon_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub code: String,
    pub status: &'static str,
    pub claimed_at: chrono::DateTime<chrono::Utc>,
    pub settled_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Repository for the promo.coupon_claims claim-state table. Claim-lifecycle
/// machinery, not a domain entity with a CRUD surface — the claim/release verbs
/// and the burn's same-ref settle are the only writers.
#[derive(Debug, Clone, Copy, Default)]
pub struct CouponClaimRepository;

impl CouponClaimRepository {
    pub fn new() -> Self {
        Self
    }
}

impl CouponClaimRepository {
    /// Resolve + lock the coupon in ONE statement: `FOR UPDATE NOWAIT` on the
    /// row an active, in-window, not-yet-exhausted-at-rest claim can rest on.
    /// The row's `used_count` / `max_use` are read UNDER the lock, so the
    /// headroom decision below is raced by nobody.
    ///
    /// `Ok(None)` = no row matched (unknown code, wrong company, inactive,
    /// soft-deleted, out of window, or already exhausted at rest) — the caller
    /// refuses uniformly without distinguishing the cause. A lock loss surfaces
    /// as SQLSTATE 55P03; the service maps that one code to
    /// `PricingError::LockBusy` (the same mapping the burn's lock takes).
    pub async fn lock_claimable(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        code: &str,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ClaimableCouponRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, pricing_rule_id, code, used_count, max_use
            FROM promo.coupon_codes
            WHERE company_id = $1
              AND code = $2
              AND status = 'active'
              AND (metadata->>'deleted_at') IS NULL
              AND valid_from <= $3
              AND (valid_upto IS NULL OR valid_upto >= $3)
              AND (max_use IS NULL OR used_count < max_use)
            FOR UPDATE NOWAIT
            "#,
        )
        .bind(company_id)
        .bind(code)
        .bind(at)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| ClaimableCouponRow {
            coupon_id: r.get("id"),
            pricing_rule_id: r.get("pricing_rule_id"),
            code: r.get("code"),
            used_count: r.get("used_count"),
            max_use: r.get("max_use"),
        }))
    }

    /// Count the coupon's un-settled claims — the reservation share of headroom.
    /// Runs on the caller's transaction, which holds the coupon row lock, so the
    /// count cannot move between read and claim-insert.
    pub async fn count_active_for_coupon(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        coupon_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM promo.coupon_claims
               WHERE company_id = $1 AND coupon_id = $2 AND status = 'claimed'"#,
        )
        .bind(company_id)
        .bind(coupon_id)
        .fetch_one(&mut *conn)
        .await
    }

    /// The cart's currently-active claim, if any. Runs on the caller's
    /// transaction; combined with the insert's partial-unique backstop this is
    /// advisory (the index is the bound), but it lets the service answer an
    /// idempotent replay and an occupied cart without speculating.
    pub async fn find_active_for_cart(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
    ) -> Result<Option<ActiveClaimRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, coupon_id, pricing_rule_id, code
               FROM promo.coupon_claims
               WHERE company_id = $1 AND cart_ref_type = $2 AND cart_ref_id = $3
                 AND status = 'claimed'"#,
        )
        .bind(company_id)
        .bind(cart_ref_type)
        .bind(cart_ref_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| ActiveClaimRow {
            claim_id: r.get("id"),
            coupon_id: r.get("coupon_id"),
            pricing_rule_id: r.get("pricing_rule_id"),
            code: r.get("code"),
        }))
    }

    /// Mint the cart's claim row. A concurrent same-cart claimant that wins the
    /// partial unique index turns this into a 23505 — the service maps that one
    /// code to the uniform refusal; nothing was written ahead of it.
    pub async fn insert_claim(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
        coupon: &ClaimableCouponRow,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Uuid, sqlx::Error> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO promo.coupon_claims
                   (company_id, cart_ref_type, cart_ref_id, coupon_id, code,
                    pricing_rule_id, status, claimed_at)
               VALUES ($1, $2, $3, $4, $5, $6, 'claimed', $7)
               RETURNING id"#,
        )
        .bind(company_id)
        .bind(cart_ref_type)
        .bind(cart_ref_id)
        .bind(coupon.coupon_id)
        .bind(&coupon.code)
        .bind(coupon.pricing_rule_id)
        .bind(at)
        .fetch_one(&mut *conn)
        .await?;
        Ok(id)
    }

    /// Transition the cart's active claim to `released` (freed headroom + freed
    /// cart slot). Idempotent: zero rows touched when nothing is active.
    /// Returns the released claim's id when a row flipped.
    pub async fn release_for_cart(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar(
            r#"UPDATE promo.coupon_claims
               SET status = 'released', settled_at = $4
               WHERE company_id = $1 AND cart_ref_type = $2 AND cart_ref_id = $3
                 AND status = 'claimed'
               RETURNING id"#,
        )
        .bind(company_id)
        .bind(cart_ref_type)
        .bind(cart_ref_id)
        .bind(at)
        .fetch_optional(&mut *conn)
        .await
    }

    /// Transition the claims this burn settles to `redeemed`. Runs INSIDE the
    /// burn's transaction, keyed by the SAME document ref the claim was minted
    /// under, so the claim flips exactly when the burn commits — never before,
    /// never without it. Idempotent by the `status = 'claimed'` guard;
    /// `settled_at` is the database clock (the burn verb takes no caller
    /// instant — the commit instant IS the settle instant).
    pub async fn settle_redeemed(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        coupon_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let res = sqlx::query(
            r#"UPDATE promo.coupon_claims
               SET status = 'redeemed', settled_at = NOW()
               WHERE company_id = $1 AND coupon_id = $2
                 AND cart_ref_type = $3 AND cart_ref_id = $4
                 AND status = 'claimed'"#,
        )
        .bind(company_id)
        .bind(coupon_id)
        .bind(cart_ref_type)
        .bind(cart_ref_id)
        .execute(&mut *conn)
        .await?;
        Ok(res.rows_affected())
    }

    /// The coupon a claim row holds — the released event's join-free coupon reference.
    /// Company scoping is the caller's (`with_company_scope`).
    pub async fn coupon_of_claim(
        &self,
        pool: &sqlx::PgPool,
        company_id: Uuid,
        claim_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar(
            r#"SELECT coupon_id FROM promo.coupon_claims
               WHERE company_id = $1 AND id = $2"#,
        )
        .bind(company_id)
        .bind(claim_id)
        .fetch_one(pool)
        .await
    }

    /// The cart's claim history (every status) — the inspectable state. Company
    /// scoping is the caller's (`with_company_scope`), mirroring `find_usable`.
    pub async fn list_for_cart(
        &self,
        pool: &sqlx::PgPool,
        company_id: Uuid,
        cart_ref_type: &str,
        cart_ref_id: Uuid,
    ) -> Result<Vec<CouponClaimRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, coupon_id, pricing_rule_id, code, status::text AS status,
                      claimed_at, settled_at
               FROM promo.coupon_claims
               WHERE company_id = $1 AND cart_ref_type = $2 AND cart_ref_id = $3
               ORDER BY claimed_at"#,
        )
        .bind(company_id)
        .bind(cart_ref_type)
        .bind(cart_ref_id)
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| CouponClaimRow {
                claim_id: r.get("id"),
                coupon_id: r.get("coupon_id"),
                pricing_rule_id: r.get("pricing_rule_id"),
                code: r.get("code"),
                status: match r.get::<&str, _>("status") {
                    "released" => "released",
                    "redeemed" => "redeemed",
                    _ => "claimed",
                },
                claimed_at: r.get("claimed_at"),
                settled_at: r.get("settled_at"),
            })
            .collect())
    }
}
