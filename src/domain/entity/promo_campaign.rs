use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::CampaignPromoType;
use super::CampaignTarget;
use super::CampaignStatus;
use super::CampaignFunder;
use super::AuditMetadata;

/// Strongly-typed ID for PromoCampaign
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromoCampaignId(pub Uuid);

impl PromoCampaignId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PromoCampaignId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PromoCampaignId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PromoCampaignId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PromoCampaignId> for Uuid {
    fn from(id: PromoCampaignId) -> Self { id.0 }
}

impl AsRef<Uuid> for PromoCampaignId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PromoCampaignId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PromoCampaign {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub promo_type: CampaignPromoType,
    pub discount_value: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_discount: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_order_value: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<i32>,
    pub applicable_services: serde_json::Value,
    pub applicable_providers: serde_json::Value,
    pub applicable_zones: serde_json::Value,
    pub target_audience: CampaignTarget,
    pub target_user_ids: serde_json::Value,
    pub first_order_only: bool,
    pub status: CampaignStatus,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_budget: Option<Decimal>,
    pub budget_used: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_redemptions: Option<i32>,
    pub max_per_user: i32,
    pub total_redemptions: i32,
    pub unique_users: i32,
    pub total_discount_given: Decimal,
    pub total_order_value: Decimal,
    pub funded_by: CampaignFunder,
    pub platform_share: Decimal,
    pub display_on_app: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_and_conditions: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PromoCampaign {
    /// Create a builder for PromoCampaign
    pub fn builder() -> PromoCampaignBuilder {
        PromoCampaignBuilder::default()
    }

    /// Create a new PromoCampaign with required fields
    pub fn new(name: String, code: String, promo_type: CampaignPromoType, discount_value: Decimal, applicable_services: serde_json::Value, applicable_providers: serde_json::Value, applicable_zones: serde_json::Value, target_audience: CampaignTarget, target_user_ids: serde_json::Value, first_order_only: bool, status: CampaignStatus, start_at: DateTime<Utc>, end_at: DateTime<Utc>, budget_used: Decimal, max_per_user: i32, total_redemptions: i32, unique_users: i32, total_discount_given: Decimal, total_order_value: Decimal, funded_by: CampaignFunder, platform_share: Decimal, display_on_app: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            code,
            description: None,
            promo_type,
            discount_value,
            max_discount: None,
            min_order_value: None,
            min_items: None,
            applicable_services,
            applicable_providers,
            applicable_zones,
            target_audience,
            target_user_ids,
            first_order_only,
            status,
            start_at,
            end_at,
            total_budget: None,
            budget_used,
            max_redemptions: None,
            max_per_user,
            total_redemptions,
            unique_users,
            total_discount_given,
            total_order_value,
            funded_by,
            platform_share,
            display_on_app,
            banner_image_url: None,
            terms_and_conditions: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PromoCampaignId {
        PromoCampaignId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &CampaignStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

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

    /// Set the min_order_value field (chainable)
    pub fn with_min_order_value(mut self, value: Decimal) -> Self {
        self.min_order_value = Some(value);
        self
    }

    /// Set the min_items field (chainable)
    pub fn with_min_items(mut self, value: i32) -> Self {
        self.min_items = Some(value);
        self
    }

    /// Set the total_budget field (chainable)
    pub fn with_total_budget(mut self, value: Decimal) -> Self {
        self.total_budget = Some(value);
        self
    }

    /// Set the max_redemptions field (chainable)
    pub fn with_max_redemptions(mut self, value: i32) -> Self {
        self.max_redemptions = Some(value);
        self
    }

    /// Set the banner_image_url field (chainable)
    pub fn with_banner_image_url(mut self, value: String) -> Self {
        self.banner_image_url = Some(value);
        self
    }

    /// Set the terms_and_conditions field (chainable)
    pub fn with_terms_and_conditions(mut self, value: String) -> Self {
        self.terms_and_conditions = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
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
                "min_order_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_order_value = v; }
                }
                "min_items" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_items = v; }
                }
                "applicable_services" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_services = v; }
                }
                "applicable_providers" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_providers = v; }
                }
                "applicable_zones" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_zones = v; }
                }
                "target_audience" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_audience = v; }
                }
                "target_user_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_user_ids = v; }
                }
                "first_order_only" => {
                    if let Ok(v) = serde_json::from_value(value) { self.first_order_only = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "start_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.start_at = v; }
                }
                "end_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.end_at = v; }
                }
                "total_budget" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_budget = v; }
                }
                "budget_used" => {
                    if let Ok(v) = serde_json::from_value(value) { self.budget_used = v; }
                }
                "max_redemptions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_redemptions = v; }
                }
                "max_per_user" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_per_user = v; }
                }
                "total_redemptions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_redemptions = v; }
                }
                "unique_users" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unique_users = v; }
                }
                "total_discount_given" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_discount_given = v; }
                }
                "total_order_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_order_value = v; }
                }
                "funded_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.funded_by = v; }
                }
                "platform_share" => {
                    if let Ok(v) = serde_json::from_value(value) { self.platform_share = v; }
                }
                "display_on_app" => {
                    if let Ok(v) = serde_json::from_value(value) { self.display_on_app = v; }
                }
                "banner_image_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.banner_image_url = v; }
                }
                "terms_and_conditions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.terms_and_conditions = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for PromoCampaign {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PromoCampaign"
    }
}

impl backbone_core::PersistentEntity for PromoCampaign {
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

impl backbone_orm::EntityRepoMeta for PromoCampaign {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("promo_type".to_string(), "campaign_promo_type".to_string());
        m.insert("target_audience".to_string(), "campaign_target".to_string());
        m.insert("status".to_string(), "campaign_status".to_string());
        m.insert("funded_by".to_string(), "campaign_funder".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "code"]
    }
}

/// Builder for PromoCampaign entity
///
/// Provides a fluent API for constructing PromoCampaign instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PromoCampaignBuilder {
    name: Option<String>,
    code: Option<String>,
    description: Option<String>,
    promo_type: Option<CampaignPromoType>,
    discount_value: Option<Decimal>,
    max_discount: Option<Decimal>,
    min_order_value: Option<Decimal>,
    min_items: Option<i32>,
    applicable_services: Option<serde_json::Value>,
    applicable_providers: Option<serde_json::Value>,
    applicable_zones: Option<serde_json::Value>,
    target_audience: Option<CampaignTarget>,
    target_user_ids: Option<serde_json::Value>,
    first_order_only: Option<bool>,
    status: Option<CampaignStatus>,
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
    total_budget: Option<Decimal>,
    budget_used: Option<Decimal>,
    max_redemptions: Option<i32>,
    max_per_user: Option<i32>,
    total_redemptions: Option<i32>,
    unique_users: Option<i32>,
    total_discount_given: Option<Decimal>,
    total_order_value: Option<Decimal>,
    funded_by: Option<CampaignFunder>,
    platform_share: Option<Decimal>,
    display_on_app: Option<bool>,
    banner_image_url: Option<String>,
    terms_and_conditions: Option<String>,
}

impl PromoCampaignBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the code field (required)
    pub fn code(mut self, value: String) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the promo_type field (required)
    pub fn promo_type(mut self, value: CampaignPromoType) -> Self {
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

    /// Set the min_order_value field (optional)
    pub fn min_order_value(mut self, value: Decimal) -> Self {
        self.min_order_value = Some(value);
        self
    }

    /// Set the min_items field (optional)
    pub fn min_items(mut self, value: i32) -> Self {
        self.min_items = Some(value);
        self
    }

    /// Set the applicable_services field (default: `serde_json::json!([])`)
    pub fn applicable_services(mut self, value: serde_json::Value) -> Self {
        self.applicable_services = Some(value);
        self
    }

    /// Set the applicable_providers field (default: `serde_json::json!([])`)
    pub fn applicable_providers(mut self, value: serde_json::Value) -> Self {
        self.applicable_providers = Some(value);
        self
    }

    /// Set the applicable_zones field (default: `serde_json::json!([])`)
    pub fn applicable_zones(mut self, value: serde_json::Value) -> Self {
        self.applicable_zones = Some(value);
        self
    }

    /// Set the target_audience field (default: `CampaignTarget::default()`)
    pub fn target_audience(mut self, value: CampaignTarget) -> Self {
        self.target_audience = Some(value);
        self
    }

    /// Set the target_user_ids field (default: `serde_json::json!([])`)
    pub fn target_user_ids(mut self, value: serde_json::Value) -> Self {
        self.target_user_ids = Some(value);
        self
    }

    /// Set the first_order_only field (default: `false`)
    pub fn first_order_only(mut self, value: bool) -> Self {
        self.first_order_only = Some(value);
        self
    }

    /// Set the status field (default: `CampaignStatus::default()`)
    pub fn status(mut self, value: CampaignStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the start_at field (required)
    pub fn start_at(mut self, value: DateTime<Utc>) -> Self {
        self.start_at = Some(value);
        self
    }

    /// Set the end_at field (required)
    pub fn end_at(mut self, value: DateTime<Utc>) -> Self {
        self.end_at = Some(value);
        self
    }

    /// Set the total_budget field (optional)
    pub fn total_budget(mut self, value: Decimal) -> Self {
        self.total_budget = Some(value);
        self
    }

    /// Set the budget_used field (default: `Decimal::from(0)`)
    pub fn budget_used(mut self, value: Decimal) -> Self {
        self.budget_used = Some(value);
        self
    }

    /// Set the max_redemptions field (optional)
    pub fn max_redemptions(mut self, value: i32) -> Self {
        self.max_redemptions = Some(value);
        self
    }

    /// Set the max_per_user field (default: `1`)
    pub fn max_per_user(mut self, value: i32) -> Self {
        self.max_per_user = Some(value);
        self
    }

    /// Set the total_redemptions field (default: `0`)
    pub fn total_redemptions(mut self, value: i32) -> Self {
        self.total_redemptions = Some(value);
        self
    }

    /// Set the unique_users field (default: `0`)
    pub fn unique_users(mut self, value: i32) -> Self {
        self.unique_users = Some(value);
        self
    }

    /// Set the total_discount_given field (default: `Decimal::from(0)`)
    pub fn total_discount_given(mut self, value: Decimal) -> Self {
        self.total_discount_given = Some(value);
        self
    }

    /// Set the total_order_value field (default: `Decimal::from(0)`)
    pub fn total_order_value(mut self, value: Decimal) -> Self {
        self.total_order_value = Some(value);
        self
    }

    /// Set the funded_by field (default: `CampaignFunder::default()`)
    pub fn funded_by(mut self, value: CampaignFunder) -> Self {
        self.funded_by = Some(value);
        self
    }

    /// Set the platform_share field (default: `Decimal::from(100)`)
    pub fn platform_share(mut self, value: Decimal) -> Self {
        self.platform_share = Some(value);
        self
    }

    /// Set the display_on_app field (default: `true`)
    pub fn display_on_app(mut self, value: bool) -> Self {
        self.display_on_app = Some(value);
        self
    }

    /// Set the banner_image_url field (optional)
    pub fn banner_image_url(mut self, value: String) -> Self {
        self.banner_image_url = Some(value);
        self
    }

    /// Set the terms_and_conditions field (optional)
    pub fn terms_and_conditions(mut self, value: String) -> Self {
        self.terms_and_conditions = Some(value);
        self
    }

    /// Build the PromoCampaign entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PromoCampaign, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let promo_type = self.promo_type.ok_or_else(|| "promo_type is required".to_string())?;
        let discount_value = self.discount_value.ok_or_else(|| "discount_value is required".to_string())?;
        let start_at = self.start_at.ok_or_else(|| "start_at is required".to_string())?;
        let end_at = self.end_at.ok_or_else(|| "end_at is required".to_string())?;

        Ok(PromoCampaign {
            id: Uuid::new_v4(),
            name,
            code,
            description: self.description,
            promo_type,
            discount_value,
            max_discount: self.max_discount,
            min_order_value: self.min_order_value,
            min_items: self.min_items,
            applicable_services: self.applicable_services.unwrap_or(serde_json::json!([])),
            applicable_providers: self.applicable_providers.unwrap_or(serde_json::json!([])),
            applicable_zones: self.applicable_zones.unwrap_or(serde_json::json!([])),
            target_audience: self.target_audience.unwrap_or(CampaignTarget::default()),
            target_user_ids: self.target_user_ids.unwrap_or(serde_json::json!([])),
            first_order_only: self.first_order_only.unwrap_or(false),
            status: self.status.unwrap_or(CampaignStatus::default()),
            start_at,
            end_at,
            total_budget: self.total_budget,
            budget_used: self.budget_used.unwrap_or(Decimal::from(0)),
            max_redemptions: self.max_redemptions,
            max_per_user: self.max_per_user.unwrap_or(1),
            total_redemptions: self.total_redemptions.unwrap_or(0),
            unique_users: self.unique_users.unwrap_or(0),
            total_discount_given: self.total_discount_given.unwrap_or(Decimal::from(0)),
            total_order_value: self.total_order_value.unwrap_or(Decimal::from(0)),
            funded_by: self.funded_by.unwrap_or(CampaignFunder::default()),
            platform_share: self.platform_share.unwrap_or(Decimal::from(100)),
            display_on_app: self.display_on_app.unwrap_or(true),
            banner_image_url: self.banner_image_url,
            terms_and_conditions: self.terms_and_conditions,
            metadata: AuditMetadata::default(),
        })
    }
}
