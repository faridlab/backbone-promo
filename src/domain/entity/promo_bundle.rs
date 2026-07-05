use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::BundleMatch;
use super::RateOrDiscount;
use super::AuditMetadata;

/// Strongly-typed ID for PromoBundle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromoBundleId(pub Uuid);

impl PromoBundleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PromoBundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PromoBundleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PromoBundleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PromoBundleId> for Uuid {
    fn from(id: PromoBundleId) -> Self { id.0 }
}

impl AsRef<Uuid> for PromoBundleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PromoBundleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PromoBundle {
    pub id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub priority: i32,
    pub match_type: BundleMatch,
    pub required_distinct: Option<i32>,
    pub reward: RateOrDiscount,
    pub discount_percentage: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub currency: String,
    pub min_order_amount: Decimal,
    pub stackable: bool,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PromoBundle {
    /// Create a builder for PromoBundle
    pub fn builder() -> PromoBundleBuilder {
        PromoBundleBuilder::default()
    }

    /// Create a new PromoBundle with required fields
    pub fn new(company_id: Uuid, title: String, priority: i32, match_type: BundleMatch, reward: RateOrDiscount, currency: String, min_order_amount: Decimal, stackable: bool, valid_from: DateTime<Utc>, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            title,
            priority,
            match_type,
            required_distinct: None,
            reward,
            discount_percentage: None,
            discount_amount: None,
            currency,
            min_order_amount,
            stackable,
            valid_from,
            valid_to: None,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PromoBundleId {
        PromoBundleId(self.id)
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

    /// Set the required_distinct field (chainable)
    pub fn with_required_distinct(mut self, value: i32) -> Self {
        self.required_distinct = Some(value);
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
                "match_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.match_type = v; }
                }
                "required_distinct" => {
                    if let Ok(v) = serde_json::from_value(value) { self.required_distinct = v; }
                }
                "reward" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reward = v; }
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
                "min_order_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_order_amount = v; }
                }
                "stackable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stackable = v; }
                }
                "valid_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_from = v; }
                }
                "valid_to" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valid_to = v; }
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

impl super::Entity for PromoBundle {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PromoBundle"
    }
}

impl backbone_core::PersistentEntity for PromoBundle {
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

impl backbone_orm::EntityRepoMeta for PromoBundle {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("match_type".to_string(), "bundle_match".to_string());
        m.insert("reward".to_string(), "rate_or_discount".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["title", "currency"]
    }
}

/// Builder for PromoBundle entity
///
/// Provides a fluent API for constructing PromoBundle instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PromoBundleBuilder {
    company_id: Option<Uuid>,
    title: Option<String>,
    priority: Option<i32>,
    match_type: Option<BundleMatch>,
    required_distinct: Option<i32>,
    reward: Option<RateOrDiscount>,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    currency: Option<String>,
    min_order_amount: Option<Decimal>,
    stackable: Option<bool>,
    valid_from: Option<DateTime<Utc>>,
    valid_to: Option<DateTime<Utc>>,
    is_active: Option<bool>,
}

impl PromoBundleBuilder {
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

    /// Set the match_type field (default: `BundleMatch::default()`)
    pub fn match_type(mut self, value: BundleMatch) -> Self {
        self.match_type = Some(value);
        self
    }

    /// Set the required_distinct field (optional)
    pub fn required_distinct(mut self, value: i32) -> Self {
        self.required_distinct = Some(value);
        self
    }

    /// Set the reward field (default: `RateOrDiscount::default()`)
    pub fn reward(mut self, value: RateOrDiscount) -> Self {
        self.reward = Some(value);
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

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the PromoBundle entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PromoBundle, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let title = self.title.ok_or_else(|| "title is required".to_string())?;
        let valid_from = self.valid_from.ok_or_else(|| "valid_from is required".to_string())?;

        Ok(PromoBundle {
            id: Uuid::new_v4(),
            company_id,
            title,
            priority: self.priority.unwrap_or(0),
            match_type: self.match_type.unwrap_or(BundleMatch::default()),
            required_distinct: self.required_distinct,
            reward: self.reward.unwrap_or(RateOrDiscount::default()),
            discount_percentage: self.discount_percentage,
            discount_amount: self.discount_amount,
            currency: self.currency.unwrap_or("IDR".to_string()),
            min_order_amount: self.min_order_amount.unwrap_or(Decimal::from(0)),
            stackable: self.stackable.unwrap_or(false),
            valid_from,
            valid_to: self.valid_to,
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
