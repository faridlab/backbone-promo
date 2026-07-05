use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::RuleScope;
use super::ApplyOn;
use super::RateOrDiscount;
use super::AuditMetadata;

/// Strongly-typed ID for PricingRule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PricingRuleId(pub Uuid);

impl PricingRuleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PricingRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PricingRuleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PricingRuleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PricingRuleId> for Uuid {
    fn from(id: PricingRuleId) -> Self { id.0 }
}

impl AsRef<Uuid> for PricingRuleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PricingRuleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PricingRule {
    pub id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub priority: i32,
    pub scope: RuleScope,
    pub min_order_amount: Decimal,
    pub stackable: bool,
    pub apply_on: ApplyOn,
    pub item_id: Option<Uuid>,
    pub item_group_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub customer_group_id: Option<Uuid>,
    pub min_qty: Decimal,
    pub max_qty: Option<Decimal>,
    pub min_amount: Decimal,
    pub rate_or_discount: RateOrDiscount,
    pub rate: Option<Decimal>,
    pub discount_percentage: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub currency: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub coupon_required: bool,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PricingRule {
    /// Create a builder for PricingRule
    pub fn builder() -> PricingRuleBuilder {
        PricingRuleBuilder::default()
    }

    /// Create a new PricingRule with required fields
    pub fn new(company_id: Uuid, title: String, priority: i32, scope: RuleScope, min_order_amount: Decimal, stackable: bool, apply_on: ApplyOn, min_qty: Decimal, min_amount: Decimal, rate_or_discount: RateOrDiscount, currency: String, valid_from: DateTime<Utc>, coupon_required: bool, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            title,
            priority,
            scope,
            min_order_amount,
            stackable,
            apply_on,
            item_id: None,
            item_group_id: None,
            brand_id: None,
            customer_id: None,
            customer_group_id: None,
            min_qty,
            max_qty: None,
            min_amount,
            rate_or_discount,
            rate: None,
            discount_percentage: None,
            discount_amount: None,
            currency,
            valid_from,
            valid_to: None,
            coupon_required,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PricingRuleId {
        PricingRuleId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the item_id field (chainable)
    pub fn with_item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the item_group_id field (chainable)
    pub fn with_item_group_id(mut self, value: Uuid) -> Self {
        self.item_group_id = Some(value);
        self
    }

    /// Set the brand_id field (chainable)
    pub fn with_brand_id(mut self, value: Uuid) -> Self {
        self.brand_id = Some(value);
        self
    }

    /// Set the customer_id field (chainable)
    pub fn with_customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the customer_group_id field (chainable)
    pub fn with_customer_group_id(mut self, value: Uuid) -> Self {
        self.customer_group_id = Some(value);
        self
    }

    /// Set the max_qty field (chainable)
    pub fn with_max_qty(mut self, value: Decimal) -> Self {
        self.max_qty = Some(value);
        self
    }

    /// Set the rate field (chainable)
    pub fn with_rate(mut self, value: Decimal) -> Self {
        self.rate = Some(value);
        self
    }

    /// Set the discount_percentage field (chainable)
    pub fn with_discount_percentage(mut self, value: Decimal) -> Self {
        self.discount_percentage = Some(value);
        self
    }

    /// Set the discount_amount field (chainable)
    pub fn with_discount_amount(mut self, value: Decimal) -> Self {
        self.discount_amount = Some(value);
        self
    }

    /// Set the valid_to field (chainable)
    pub fn with_valid_to(mut self, value: DateTime<Utc>) -> Self {
        self.valid_to = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "title" => {
                    if let Ok(v) = serde_json::from_value(value) { self.title = v; }
                }
                "priority" => {
                    if let Ok(v) = serde_json::from_value(value) { self.priority = v; }
                }
                "scope" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scope = v; }
                }
                "min_order_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_order_amount = v; }
                }
                "stackable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stackable = v; }
                }
                "apply_on" => {
                    if let Ok(v) = serde_json::from_value(value) { self.apply_on = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "item_group_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_group_id = v; }
                }
                "brand_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.brand_id = v; }
                }
                "customer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.customer_id = v; }
                }
                "customer_group_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.customer_group_id = v; }
                }
                "min_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_qty = v; }
                }
                "max_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_qty = v; }
                }
                "min_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_amount = v; }
                }
                "rate_or_discount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rate_or_discount = v; }
                }
                "rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rate = v; }
                }
                "discount_percentage" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_percentage = v; }
                }
                "discount_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_amount = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "valid_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_from = v; }
                }
                "valid_to" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_to = v; }
                }
                "coupon_required" => {
                    if let Ok(v) = serde_json::from_value(value) { self.coupon_required = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for PricingRule {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PricingRule"
    }
}

impl backbone_core::PersistentEntity for PricingRule {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for PricingRule {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("item_group_id".to_string(), "uuid".to_string());
        m.insert("brand_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("customer_group_id".to_string(), "uuid".to_string());
        m.insert("scope".to_string(), "rule_scope".to_string());
        m.insert("apply_on".to_string(), "apply_on".to_string());
        m.insert("rate_or_discount".to_string(), "rate_or_discount".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["title", "currency"]
    }
}

/// Builder for PricingRule entity
///
/// Provides a fluent API for constructing PricingRule instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PricingRuleBuilder {
    company_id: Option<Uuid>,
    title: Option<String>,
    priority: Option<i32>,
    scope: Option<RuleScope>,
    min_order_amount: Option<Decimal>,
    stackable: Option<bool>,
    apply_on: Option<ApplyOn>,
    item_id: Option<Uuid>,
    item_group_id: Option<Uuid>,
    brand_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    customer_group_id: Option<Uuid>,
    min_qty: Option<Decimal>,
    max_qty: Option<Decimal>,
    min_amount: Option<Decimal>,
    rate_or_discount: Option<RateOrDiscount>,
    rate: Option<Decimal>,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    currency: Option<String>,
    valid_from: Option<DateTime<Utc>>,
    valid_to: Option<DateTime<Utc>>,
    coupon_required: Option<bool>,
    is_active: Option<bool>,
}

impl PricingRuleBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the title field (required)
    pub fn title(mut self, value: String) -> Self {
        self.title = Some(value);
        self
    }

    /// Set the priority field (default: `0`)
    pub fn priority(mut self, value: i32) -> Self {
        self.priority = Some(value);
        self
    }

    /// Set the scope field (default: `RuleScope::default()`)
    pub fn scope(mut self, value: RuleScope) -> Self {
        self.scope = Some(value);
        self
    }

    /// Set the min_order_amount field (default: `Decimal::from(0)`)
    pub fn min_order_amount(mut self, value: Decimal) -> Self {
        self.min_order_amount = Some(value);
        self
    }

    /// Set the stackable field (default: `false`)
    pub fn stackable(mut self, value: bool) -> Self {
        self.stackable = Some(value);
        self
    }

    /// Set the apply_on field (default: `ApplyOn::default()`)
    pub fn apply_on(mut self, value: ApplyOn) -> Self {
        self.apply_on = Some(value);
        self
    }

    /// Set the item_id field (optional)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the item_group_id field (optional)
    pub fn item_group_id(mut self, value: Uuid) -> Self {
        self.item_group_id = Some(value);
        self
    }

    /// Set the brand_id field (optional)
    pub fn brand_id(mut self, value: Uuid) -> Self {
        self.brand_id = Some(value);
        self
    }

    /// Set the customer_id field (optional)
    pub fn customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the customer_group_id field (optional)
    pub fn customer_group_id(mut self, value: Uuid) -> Self {
        self.customer_group_id = Some(value);
        self
    }

    /// Set the min_qty field (default: `Decimal::from(0)`)
    pub fn min_qty(mut self, value: Decimal) -> Self {
        self.min_qty = Some(value);
        self
    }

    /// Set the max_qty field (optional)
    pub fn max_qty(mut self, value: Decimal) -> Self {
        self.max_qty = Some(value);
        self
    }

    /// Set the min_amount field (default: `Decimal::from(0)`)
    pub fn min_amount(mut self, value: Decimal) -> Self {
        self.min_amount = Some(value);
        self
    }

    /// Set the rate_or_discount field (default: `RateOrDiscount::default()`)
    pub fn rate_or_discount(mut self, value: RateOrDiscount) -> Self {
        self.rate_or_discount = Some(value);
        self
    }

    /// Set the rate field (optional)
    pub fn rate(mut self, value: Decimal) -> Self {
        self.rate = Some(value);
        self
    }

    /// Set the discount_percentage field (optional)
    pub fn discount_percentage(mut self, value: Decimal) -> Self {
        self.discount_percentage = Some(value);
        self
    }

    /// Set the discount_amount field (optional)
    pub fn discount_amount(mut self, value: Decimal) -> Self {
        self.discount_amount = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the valid_from field (required)
    pub fn valid_from(mut self, value: DateTime<Utc>) -> Self {
        self.valid_from = Some(value);
        self
    }

    /// Set the valid_to field (optional)
    pub fn valid_to(mut self, value: DateTime<Utc>) -> Self {
        self.valid_to = Some(value);
        self
    }

    /// Set the coupon_required field (default: `false`)
    pub fn coupon_required(mut self, value: bool) -> Self {
        self.coupon_required = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the PricingRule entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PricingRule, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let title = self.title.ok_or_else(|| "title is required".to_string())?;
        let valid_from = self.valid_from.ok_or_else(|| "valid_from is required".to_string())?;

        Ok(PricingRule {
            id: Uuid::new_v4(),
            company_id,
            title,
            priority: self.priority.unwrap_or(0),
            scope: self.scope.unwrap_or(RuleScope::default()),
            min_order_amount: self.min_order_amount.unwrap_or(Decimal::from(0)),
            stackable: self.stackable.unwrap_or(false),
            apply_on: self.apply_on.unwrap_or(ApplyOn::default()),
            item_id: self.item_id,
            item_group_id: self.item_group_id,
            brand_id: self.brand_id,
            customer_id: self.customer_id,
            customer_group_id: self.customer_group_id,
            min_qty: self.min_qty.unwrap_or(Decimal::from(0)),
            max_qty: self.max_qty,
            min_amount: self.min_amount.unwrap_or(Decimal::from(0)),
            rate_or_discount: self.rate_or_discount.unwrap_or(RateOrDiscount::default()),
            rate: self.rate,
            discount_percentage: self.discount_percentage,
            discount_amount: self.discount_amount,
            currency: self.currency.unwrap_or("IDR".to_string()),
            valid_from,
            valid_to: self.valid_to,
            coupon_required: self.coupon_required.unwrap_or(false),
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
