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

// ---- Cart-scoped resolution (ADR-002) --------------------------------------------------------
//
// `resolve` prices ONE line in isolation, so it can express neither a promotion whose condition
// spans distinct lines (bundling: "buy A + B") nor one gated on the whole basket (cart-total
// minimum: "spend 500k → 10% off"). `resolve_cart` takes the whole basket and returns, per line,
// the same per-line result `resolve` gives PLUS any order-level discounts (order-scoped rules and
// bundles) allocated back across the contributing lines. It stays a pure read — coupons are still
// burned only by `commit_coupon_redemption`.

/// One line the caller wants priced as part of a basket. `query.coupon_code` / `query.customer_*`
/// are read from the cart, not the line, so per-line coupon/customer here are ignored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CartLine {
    /// The caller's stable identifier for this line (echoed back in `ResolvedLine` and allocations).
    pub line_id: Uuid,
    /// The line's pricing dimensions — same shape a single-line `resolve` takes.
    pub query: PriceQuery,
}

/// The whole basket to price in one call. Customer, coupon and instant are cart-wide.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CartQuery {
    pub company_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub customer_group_id: Option<Uuid>,
    /// A coupon code the customer presented (unlocks `coupon_required` line rules). Case-insensitive.
    pub coupon_code: Option<String>,
    pub lines: Vec<CartLine>,
    /// The instant to evaluate every validity window against (the sale's posting time).
    pub at: chrono::DateTime<chrono::Utc>,
}

/// One line's resolved result inside a cart: its per-line price (as `resolve` would give) plus the
/// share of every order-level adjustment allocated to it. `net_line_total` already reflects both.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedLine {
    pub line_id: Uuid,
    pub item_id: Uuid,
    pub quantity: Decimal,
    /// Effective unit price after per-line rules (before order-level allocation).
    pub unit_price: Decimal,
    /// Per-unit discount from the winning per-line rule (0 if none).
    pub line_discount_amount: Decimal,
    /// The per-line rule that won, if any.
    pub applied_rule_id: Option<Uuid>,
    /// This line's share of all order-level adjustments (order rules + bundles).
    pub order_discount_share: Decimal,
    /// `unit_price·quantity − order_discount_share` (never negative).
    pub net_line_total: Decimal,
}

/// What produced an order-level discount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdjustmentSource {
    /// A `scope=order` PricingRule fired on the cart subtotal.
    OrderRule(Uuid),
    /// A PromoBundle was satisfied by the basket.
    Bundle(Uuid),
}

/// One order-level discount and how it was spread across lines (`allocated` sums to `discount_amount`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderAdjustment {
    pub source: AdjustmentSource,
    pub discount_amount: Decimal,
    /// `(line_id, share)` pairs; Σ share == `discount_amount` exactly (penny-reconciled).
    pub allocated: Vec<(Uuid, Decimal)>,
}

/// A free item a satisfied buy-X-get-Y bundle grants — a zero-priced line the consumer adds to the
/// basket. It is NOT an order discount (it doesn't reduce `order_discount_total`); it's extra goods.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardLine {
    pub bundle_id: Uuid,
    pub item_id: Uuid,
    pub quantity: Decimal,
}

/// The resolved basket: per-line results, the order-level adjustments that fired, and the totals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedCart {
    pub lines: Vec<ResolvedLine>,
    pub order_adjustments: Vec<OrderAdjustment>,
    /// Free items granted by satisfied buy-X-get-Y bundles (zero-priced; the consumer adds them).
    pub reward_lines: Vec<RewardLine>,
    /// Σ line unit_price·qty (after per-line rules, before order-level discounts).
    pub subtotal: Decimal,
    /// Σ order_adjustments.discount_amount (never exceeds `subtotal`).
    pub order_discount_total: Decimal,
    /// `subtotal − order_discount_total`.
    pub total: Decimal,
}

/// The port a selling/POS caller depends on. A composing service implements it over `PricingService`.
#[async_trait::async_trait]
pub trait PriceResolverPort: Send + Sync {
    /// Price one line in isolation (unchanged; existing callers keep using this).
    async fn resolve(&self, query: &PriceQuery) -> Result<ResolvedPrice, PricingError>;

    /// Price a whole basket: per-line rules, then order-scoped rules and bundles, with every
    /// order-level discount allocated back across the lines.
    async fn resolve_cart(&self, query: &CartQuery) -> Result<ResolvedCart, PricingError>;
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
