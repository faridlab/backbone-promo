use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PromoType;
use super::PromoEligibility;
use super::AuditMetadata;

/// Strongly-typed ID for ProviderPromo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderPromoId(pub Uuid);

impl ProviderPromoId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ProviderPromoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ProviderPromoId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ProviderPromoId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ProviderPromoId> for Uuid {
    fn from(id: ProviderPromoId) -> Self { id.0 }
}

impl AsRef<Uuid> for ProviderPromoId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ProviderPromoId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProviderPromo {
    pub id: Uuid,
    pub provider_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlet_id: Option<Uuid>,
    pub promo_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promo_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub promo_type: PromoType,
    pub discount_value: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_discount: Option<Decimal>,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_order_amount: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_weight_kg: Option<Decimal>,
    pub applicable_services: serde_json::Value,
    pub applicable_categories: serde_json::Value,
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_limit_per_customer: Option<i32>,
    pub usage_count: i32,
    pub eligible_customers: PromoEligibility,
    pub eligible_tiers: serde_json::Value,
    pub is_active: bool,
    pub is_public: bool,
    pub is_stackable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_conditions: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ProviderPromo {
    /// Create a builder for ProviderPromo
    pub fn builder() -> ProviderPromoBuilder {
        ProviderPromoBuilder::default()
    }

    /// Create a new ProviderPromo with required fields
    pub fn new(provider_id: Uuid, promo_name: String, promo_type: PromoType, discount_value: Decimal, currency: String, applicable_services: serde_json::Value, applicable_categories: serde_json::Value, valid_from: DateTime<Utc>, valid_until: DateTime<Utc>, usage_count: i32, eligible_customers: PromoEligibility, eligible_tiers: serde_json::Value, is_active: bool, is_public: bool, is_stackable: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            outlet_id: None,
            promo_name,
            promo_code: None,
            description: None,
            promo_type,
            discount_value,
            max_discount: None,
            currency,
            min_order_amount: None,
            min_weight_kg: None,
            applicable_services,
            applicable_categories,
            valid_from,
            valid_until,
            usage_limit: None,
            usage_limit_per_customer: None,
            usage_count,
            eligible_customers,
            eligible_tiers,
            is_active,
            is_public,
            is_stackable,
            banner_url: None,
            terms_conditions: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ProviderPromoId {
        ProviderPromoId(self.id)
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

    /// Set the outlet_id field (chainable)
    pub fn with_outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
        self
    }

    /// Set the promo_code field (chainable)
    pub fn with_promo_code(mut self, value: String) -> Self {
        self.promo_code = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the max_discount field (chainable)
    pub fn with_max_discount(mut self, value: Decimal) -> Self {
        self.max_discount = Some(value);
        self
    }

    /// Set the min_order_amount field (chainable)
    pub fn with_min_order_amount(mut self, value: Decimal) -> Self {
        self.min_order_amount = Some(value);
        self
    }

    /// Set the min_weight_kg field (chainable)
    pub fn with_min_weight_kg(mut self, value: Decimal) -> Self {
        self.min_weight_kg = Some(value);
        self
    }

    /// Set the usage_limit field (chainable)
    pub fn with_usage_limit(mut self, value: i32) -> Self {
        self.usage_limit = Some(value);
        self
    }

    /// Set the usage_limit_per_customer field (chainable)
    pub fn with_usage_limit_per_customer(mut self, value: i32) -> Self {
        self.usage_limit_per_customer = Some(value);
        self
    }

    /// Set the banner_url field (chainable)
    pub fn with_banner_url(mut self, value: String) -> Self {
        self.banner_url = Some(value);
        self
    }

    /// Set the terms_conditions field (chainable)
    pub fn with_terms_conditions(mut self, value: String) -> Self {
        self.terms_conditions = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "outlet_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outlet_id = v; }
                }
                "promo_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.promo_name = v; }
                }
                "promo_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.promo_code = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "promo_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.promo_type = v; }
                }
                "discount_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_value = v; }
                }
                "max_discount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_discount = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "min_order_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_order_amount = v; }
                }
                "min_weight_kg" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_weight_kg = v; }
                }
                "applicable_services" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_services = v; }
                }
                "applicable_categories" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_categories = v; }
                }
                "valid_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_from = v; }
                }
                "valid_until" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_until = v; }
                }
                "usage_limit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage_limit = v; }
                }
                "usage_limit_per_customer" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage_limit_per_customer = v; }
                }
                "usage_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage_count = v; }
                }
                "eligible_customers" => {
                    if let Ok(v) = serde_json::from_value(value) { self.eligible_customers = v; }
                }
                "eligible_tiers" => {
                    if let Ok(v) = serde_json::from_value(value) { self.eligible_tiers = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                "is_public" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_public = v; }
                }
                "is_stackable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_stackable = v; }
                }
                "banner_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.banner_url = v; }
                }
                "terms_conditions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.terms_conditions = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ProviderPromo {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ProviderPromo"
    }
}

impl backbone_core::PersistentEntity for ProviderPromo {
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

impl backbone_orm::EntityRepoMeta for ProviderPromo {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("outlet_id".to_string(), "uuid".to_string());
        m.insert("promo_type".to_string(), "promo_type".to_string());
        m.insert("eligible_customers".to_string(), "promo_eligibility".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["promo_name", "currency"]
    }
}

/// Builder for ProviderPromo entity
///
/// Provides a fluent API for constructing ProviderPromo instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ProviderPromoBuilder {
    provider_id: Option<Uuid>,
    outlet_id: Option<Uuid>,
    promo_name: Option<String>,
    promo_code: Option<String>,
    description: Option<String>,
    promo_type: Option<PromoType>,
    discount_value: Option<Decimal>,
    max_discount: Option<Decimal>,
    currency: Option<String>,
    min_order_amount: Option<Decimal>,
    min_weight_kg: Option<Decimal>,
    applicable_services: Option<serde_json::Value>,
    applicable_categories: Option<serde_json::Value>,
    valid_from: Option<DateTime<Utc>>,
    valid_until: Option<DateTime<Utc>>,
    usage_limit: Option<i32>,
    usage_limit_per_customer: Option<i32>,
    usage_count: Option<i32>,
    eligible_customers: Option<PromoEligibility>,
    eligible_tiers: Option<serde_json::Value>,
    is_active: Option<bool>,
    is_public: Option<bool>,
    is_stackable: Option<bool>,
    banner_url: Option<String>,
    terms_conditions: Option<String>,
}

impl ProviderPromoBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the outlet_id field (optional)
    pub fn outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
        self
    }

    /// Set the promo_name field (required)
    pub fn promo_name(mut self, value: String) -> Self {
        self.promo_name = Some(value);
        self
    }

    /// Set the promo_code field (optional)
    pub fn promo_code(mut self, value: String) -> Self {
        self.promo_code = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the promo_type field (default: `PromoType::default()`)
    pub fn promo_type(mut self, value: PromoType) -> Self {
        self.promo_type = Some(value);
        self
    }

    /// Set the discount_value field (required)
    pub fn discount_value(mut self, value: Decimal) -> Self {
        self.discount_value = Some(value);
        self
    }

    /// Set the max_discount field (optional)
    pub fn max_discount(mut self, value: Decimal) -> Self {
        self.max_discount = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the min_order_amount field (optional)
    pub fn min_order_amount(mut self, value: Decimal) -> Self {
        self.min_order_amount = Some(value);
        self
    }

    /// Set the min_weight_kg field (optional)
    pub fn min_weight_kg(mut self, value: Decimal) -> Self {
        self.min_weight_kg = Some(value);
        self
    }

    /// Set the applicable_services field (default: `serde_json::json!([])`)
    pub fn applicable_services(mut self, value: serde_json::Value) -> Self {
        self.applicable_services = Some(value);
        self
    }

    /// Set the applicable_categories field (default: `serde_json::json!([])`)
    pub fn applicable_categories(mut self, value: serde_json::Value) -> Self {
        self.applicable_categories = Some(value);
        self
    }

    /// Set the valid_from field (required)
    pub fn valid_from(mut self, value: DateTime<Utc>) -> Self {
        self.valid_from = Some(value);
        self
    }

    /// Set the valid_until field (required)
    pub fn valid_until(mut self, value: DateTime<Utc>) -> Self {
        self.valid_until = Some(value);
        self
    }

    /// Set the usage_limit field (optional)
    pub fn usage_limit(mut self, value: i32) -> Self {
        self.usage_limit = Some(value);
        self
    }

    /// Set the usage_limit_per_customer field (optional)
    pub fn usage_limit_per_customer(mut self, value: i32) -> Self {
        self.usage_limit_per_customer = Some(value);
        self
    }

    /// Set the usage_count field (default: `0`)
    pub fn usage_count(mut self, value: i32) -> Self {
        self.usage_count = Some(value);
        self
    }

    /// Set the eligible_customers field (default: `PromoEligibility::default()`)
    pub fn eligible_customers(mut self, value: PromoEligibility) -> Self {
        self.eligible_customers = Some(value);
        self
    }

    /// Set the eligible_tiers field (default: `serde_json::json!([])`)
    pub fn eligible_tiers(mut self, value: serde_json::Value) -> Self {
        self.eligible_tiers = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Set the is_public field (default: `true`)
    pub fn is_public(mut self, value: bool) -> Self {
        self.is_public = Some(value);
        self
    }

    /// Set the is_stackable field (default: `false`)
    pub fn is_stackable(mut self, value: bool) -> Self {
        self.is_stackable = Some(value);
        self
    }

    /// Set the banner_url field (optional)
    pub fn banner_url(mut self, value: String) -> Self {
        self.banner_url = Some(value);
        self
    }

    /// Set the terms_conditions field (optional)
    pub fn terms_conditions(mut self, value: String) -> Self {
        self.terms_conditions = Some(value);
        self
    }

    /// Build the ProviderPromo entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ProviderPromo, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let promo_name = self.promo_name.ok_or_else(|| "promo_name is required".to_string())?;
        let discount_value = self.discount_value.ok_or_else(|| "discount_value is required".to_string())?;
        let valid_from = self.valid_from.ok_or_else(|| "valid_from is required".to_string())?;
        let valid_until = self.valid_until.ok_or_else(|| "valid_until is required".to_string())?;

        Ok(ProviderPromo {
            id: Uuid::new_v4(),
            provider_id,
            outlet_id: self.outlet_id,
            promo_name,
            promo_code: self.promo_code,
            description: self.description,
            promo_type: self.promo_type.unwrap_or(PromoType::default()),
            discount_value,
            max_discount: self.max_discount,
            currency: self.currency.unwrap_or("IDR".to_string()),
            min_order_amount: self.min_order_amount,
            min_weight_kg: self.min_weight_kg,
            applicable_services: self.applicable_services.unwrap_or(serde_json::json!([])),
            applicable_categories: self.applicable_categories.unwrap_or(serde_json::json!([])),
            valid_from,
            valid_until,
            usage_limit: self.usage_limit,
            usage_limit_per_customer: self.usage_limit_per_customer,
            usage_count: self.usage_count.unwrap_or(0),
            eligible_customers: self.eligible_customers.unwrap_or(PromoEligibility::default()),
            eligible_tiers: self.eligible_tiers.unwrap_or(serde_json::json!([])),
            is_active: self.is_active.unwrap_or(true),
            is_public: self.is_public.unwrap_or(true),
            is_stackable: self.is_stackable.unwrap_or(false),
            banner_url: self.banner_url,
            terms_conditions: self.terms_conditions,
            metadata: AuditMetadata::default(),
        })
    }
}
