//! The hand-authored promo write path (user-owned; survives regen) — the HUB.
//!
//! Promo's write service has five responsibilities, none of which post GL:
//!   1. `resolve` — the marquee READ. Single-line effective price via a deterministic winner.
//!   2. `resolve_cart` — the cart-scoped READ. The same line pass + a bundle pass + an order pass,
//!      reconciled so `Σ net_line_total == total` exactly, with per-tax-group allocation.
//!   3. `commit_coupon_redemption` — the bounded WRITE. Atomic, idempotent-per-source coupon burn.
//!   4. loyalty ledger — `accrue` (idempotent per source) + `redeem` (serialized, balance-bounded,
//!      expiry-aware).
//!   5. per-order loyalty accounting — `grant_order_points` / `spend_order_points` /
//!      `reverse_order_points`: the order-flow verbs a host (POS / selling) composes at confirm,
//!      payment and return.
//!
//! **This file is the hub:** it holds the service's vocabulary — the struct, its constructor, the
//! `money()` helper, the NOWAIT lock-error mapping, and the legacy loyalty outcome types — plus the
//! `PromoPriceResolver` port adapter. The responsibilities are chunked into focused siblings, each
//! an `impl PromoWriteService` block over these same types:
//!
//! - [`super::promo_resolve`] — single-line price resolution (`resolve`).
//! - [`super::promo_cart`] — cart-scoped resolution with bundles + order rules (`resolve_cart`).
//! - [`super::promo_coupon`] — the bounded coupon burn (`commit_coupon_redemption`).
//! - [`super::promo_loyalty`] — the loyalty ledger (`accrue`, `redeem`).
//! - [`super::promo_loyalty_order`] — the per-order loyalty verbs (grant / spend / reverse).
//!
//! Money is IDR, 2dp, half-away-from-zero. Points are whole (floored on accrual) and are NOT money.
//!
//! **Lock order (fixed):** a loyalty transaction takes the program row FIRST, then the member
//! anchor; the coupon verb locks only the coupon row. No transaction takes {program, anchor} and
//! {coupon} in opposite orders, so a `LockBusy` is always transient contention, never a deadlock.

use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    CouponCodeRepository, CouponRedemptionRepository, LoyaltyMemberAnchorRepository,
    LoyaltyOrderPointsRepository, LoyaltyPointEntryRepository, LoyaltyProgramRepository,
    PricingRuleRepository, PromoBundleComponentRepository, PromoBundleGiftRepository,
    PromoBundleRepository,
};

use super::promo_ports::{
    CartQuery, LockResource, PriceQuery, PriceResolverPort, PricingError, ResolvedCart,
    ResolvedPrice,
};

/// Round to 2dp, half away from zero (IDR money).
pub(super) fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Map a sqlx error off a `FOR UPDATE NOWAIT` statement: ONLY the lock-not-available SQLSTATE
/// (55P03) becomes the typed [`PricingError::LockBusy`]; every other database error flows through
/// as [`PricingError::Db`] untouched — nothing is swallowed, and no other code is remapped.
pub(super) fn lock_busy_or_db(err: sqlx::Error, resource: LockResource) -> PricingError {
    if let sqlx::Error::Database(ref db) = err {
        if db.code().as_deref() == Some("55P03") {
            return PricingError::LockBusy { resource };
        }
    }
    PricingError::Db(err)
}

/// The promo write service: orchestrates the repositories that hold the SQL, and owns the units of
/// work (the coupon burn and every loyalty verb each run in a transaction it opens).
pub struct PromoWriteService {
    pub(super) pool: PgPool,
    pub(super) rules: PricingRuleRepository,
    pub(super) coupons: CouponCodeRepository,
    pub(super) redemptions: CouponRedemptionRepository,
    pub(super) bundles: PromoBundleRepository,
    pub(super) bundle_components: PromoBundleComponentRepository,
    pub(super) bundle_gifts: PromoBundleGiftRepository,
    pub(super) programs: LoyaltyProgramRepository,
    pub(super) entries: LoyaltyPointEntryRepository,
    pub(super) order_points: LoyaltyOrderPointsRepository,
    pub(super) anchors: LoyaltyMemberAnchorRepository,
}

impl PromoWriteService {
    pub fn new(pool: PgPool) -> Self {
        let rules = PricingRuleRepository::new(pool.clone());
        let coupons = CouponCodeRepository::new(pool.clone());
        let redemptions = CouponRedemptionRepository::new(pool.clone());
        let bundles = PromoBundleRepository::new(pool.clone());
        let bundle_components = PromoBundleComponentRepository::new(pool.clone());
        let bundle_gifts = PromoBundleGiftRepository::new(pool.clone());
        let programs = LoyaltyProgramRepository::new(pool.clone());
        let entries = LoyaltyPointEntryRepository::new(pool.clone());
        let order_points = LoyaltyOrderPointsRepository::new(pool.clone());
        let anchors = LoyaltyMemberAnchorRepository::new();
        Self {
            pool,
            rules,
            coupons,
            redemptions,
            bundles,
            bundle_components,
            bundle_gifts,
            programs,
            entries,
            order_points,
            anchors,
        }
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
