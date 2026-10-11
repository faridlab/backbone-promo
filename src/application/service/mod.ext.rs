// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod promo_events;
pub mod promo_ports;
pub mod promo_write_service;
// The write surface, chunked: each is an `impl PromoWriteService` block over the vocabulary that
// stays in `promo_write_service` (so the `promo_write_service::{PromoWriteService, ...}` import
// paths are unchanged).
pub mod promo_resolve;
pub mod promo_cart;
pub mod promo_coupon;
pub mod promo_loyalty;
pub mod promo_loyalty_order;
// The cart-stage code claims (claim / release / inspect) — same impl-chunk pattern.
pub mod promo_claim;
pub use promo_events::{
    CouponRedeemed, LoggingSink, LoyaltyOrderPointsGranted, LoyaltyOrderPointsReversed,
    LoyaltyOrderPointsSpent, LoyaltyPointsEarned, LoyaltyPointsRedeemed, PromoCodeClaimed,
    PromoCodeClaimReleased, PromoEvent, PromoEventSink,
};
pub use promo_ports::{
    AccrualRequest, AllocationShare, CartLine, CartQuery, CodeClaimView, CouponClaimStatus,
    LockResource, OrderAdjustment, OrderPointsGrantOutcome, OrderPointsGrantRequest,
    OrderPointsReversalOutcome, OrderPointsReversalRequest, OrderPointsSpendOutcome,
    OrderPointsSpendRequest, PriceQuery, PriceResolverPort, PricingError, PromoCodeClaimOutcome,
    PromoCodeClaimRequest, RedemptionRequest, ResolvedCart, ResolvedLine, ResolvedPrice,
    TaxGroupTotal,
};
pub use promo_write_service::{
    AccrualOutcome, PromoPriceResolver, PromoWriteService, RedemptionOutcome,
};
