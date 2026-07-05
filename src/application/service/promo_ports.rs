//! Promo's outward contract — the DTOs the selling/POS write paths pass in and receive back.
//!
//! Selling and POS currently take `unit_price`/`discount` as GIVEN inputs. This is the seam that
//! lets them ask promo to RESOLVE those instead: a caller builds a `PriceQuery` per line and gets a
//! `ResolvedPrice` (effective unit price + per-unit discount + which rule/coupon applied). The caller
//! holds a `PriceResolverPort` trait object — **zero normal Cargo edge**; a composing service wires
//! promo's `PricingService` behind it, exactly as POS wires billing/payment behind its ports.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One line the caller wants priced, plus the dimensions the resolver matches rules against.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceQuery {
    pub company_id: Uuid,
    /// The list/base unit price before any promo (what selling/POS would otherwise charge).
    pub list_price: Decimal,
    pub quantity: Decimal,
    pub item_id: Uuid,
    pub item_group_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub customer_group_id: Option<Uuid>,
    /// A coupon code the customer presented (unlocks `coupon_required` rules). Case-insensitive.
    pub coupon_code: Option<String>,
    /// The instant to evaluate validity windows against (the sale's posting time).
    pub at: chrono::DateTime<chrono::Utc>,
}

/// The resolved price for a line: what to actually charge per unit and the discount that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedPrice {
    /// Effective unit price (never negative). Equals `list_price` when no rule applied.
    pub unit_price: Decimal,
    /// Per-unit discount off list (`list_price - unit_price`), 0 when no rule applied.
    pub discount_amount: Decimal,
    /// The rule that won, if any.
    pub applied_rule_id: Option<Uuid>,
    /// The coupon that unlocked the winning rule, if the win depended on one.
    pub applied_coupon_id: Option<Uuid>,
}

impl ResolvedPrice {
    /// The pass-through result: no rule applied, charge the list price.
    pub fn passthrough(list_price: Decimal) -> Self {
        Self {
            unit_price: list_price,
            discount_amount: Decimal::ZERO,
            applied_rule_id: None,
            applied_coupon_id: None,
        }
    }
}

/// The port a selling/POS caller depends on. A composing service implements it over `PricingService`.
#[async_trait::async_trait]
pub trait PriceResolverPort: Send + Sync {
    async fn resolve(&self, query: &PriceQuery) -> Result<ResolvedPrice, PricingError>;
}

/// A request to accrue loyalty points for a settled purchase (the earn leg).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccrualRequest {
    pub company_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub customer_id: Uuid,
    /// The spend that earns points (net of tax; caller's choice of base).
    pub purchase_amount: Decimal,
    /// The document that generated the spend (e.g. `pos_invoice` / `sales_invoice`) — the idempotency key.
    pub source_type: String,
    pub source_id: Uuid,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// A request to redeem points against a purchase (the burn leg).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedemptionRequest {
    pub company_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub customer_id: Uuid,
    pub points: Decimal,
    pub source_type: String,
    pub source_id: Uuid,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Errors surfaced to a caller.
#[derive(Debug, thiserror::Error)]
pub enum PricingError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("coupon not found or not active: {0}")]
    CouponInvalid(String),
    #[error("coupon exhausted (max_use reached)")]
    CouponExhausted,
    #[error("loyalty program not found or inactive")]
    ProgramInvalid,
    #[error("insufficient points: have {available}, asked {requested}")]
    InsufficientPoints { available: Decimal, requested: Decimal },
    #[error("invalid input: {0}")]
    Invalid(String),
}
