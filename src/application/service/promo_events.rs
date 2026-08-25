//! Promo domain events (hand-authored, user-owned) — the public extension surface.
//!
//! Promo posts NO GL and owns no money of record. It resolves prices (a pure read the selling/POS
//! write paths consume) and runs the loyalty points ledger. These events are the hooks a downstream
//! consumer (analytics, notifications, a claw-back on returns) subscribes to. Points are NOT money.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A coupon was consumed against a source document (its `used_count` advanced, bounded by `max_use`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CouponRedeemed {
    pub coupon_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub company_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
}

/// Points accrued from a purchase (the earn leg). Idempotent per source document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyPointsEarned {
    pub entry_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Uuid,
    pub points: Decimal,
    pub purchase_amount: Decimal,
    pub source_type: String,
    pub source_id: Uuid,
}

/// Points spent against a purchase (the burn leg), bounded by the member's available balance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyPointsRedeemed {
    pub entry_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Uuid,
    pub points: Decimal,
    pub discount_value: Decimal,
    pub source_type: String,
    pub source_id: Uuid,
}

/// Points granted for one logical order (the order-flow earn leg). Idempotent per order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyOrderPointsGranted {
    pub order_points_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Uuid,
    pub order_ref_type: String,
    pub order_ref_id: Uuid,
    pub points: Decimal,
    pub grant_base_amount: Decimal,
}

/// Points spent against one logical order (the order-flow burn leg). Idempotent per order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyOrderPointsSpent {
    pub order_points_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Uuid,
    pub order_ref_type: String,
    pub order_ref_id: Uuid,
    pub points: Decimal,
    pub discount_value: Decimal,
}

/// (Part of) one order's points reversed against a return document. Idempotent per return.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyOrderPointsReversed {
    pub order_points_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Uuid,
    pub order_ref_type: String,
    pub order_ref_id: Uuid,
    pub grant_reversed: Decimal,
    pub spend_restored: Decimal,
    pub reversal_ref_type: String,
    pub reversal_ref_id: Uuid,
}

/// The promo domain-event union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PromoEvent {
    CouponRedeemed(CouponRedeemed),
    LoyaltyPointsEarned(LoyaltyPointsEarned),
    LoyaltyPointsRedeemed(LoyaltyPointsRedeemed),
    LoyaltyOrderPointsGranted(LoyaltyOrderPointsGranted),
    LoyaltyOrderPointsSpent(LoyaltyOrderPointsSpent),
    LoyaltyOrderPointsReversed(LoyaltyOrderPointsReversed),
}

/// Sink the write path publishes to. A consuming service supplies its own (bus, outbox, …).
pub trait PromoEventSink {
    fn publish(&self, event: &PromoEvent);
}

/// A no-op/logging sink for tests and single-process composition.
#[derive(Debug, Default, Clone)]
pub struct LoggingSink;

impl PromoEventSink for LoggingSink {
    fn publish(&self, event: &PromoEvent) {
        tracing::info!(?event, "promo event");
    }
}
