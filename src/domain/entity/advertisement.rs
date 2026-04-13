use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AdPlacement;
use super::AdStatus;
use super::AdBillingModel;
use super::AuditMetadata;

/// Strongly-typed ID for Advertisement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdvertisementId(pub Uuid);

impl AdvertisementId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AdvertisementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AdvertisementId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AdvertisementId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AdvertisementId> for Uuid {
    fn from(id: AdvertisementId) -> Self { id.0 }
}

impl AsRef<Uuid> for AdvertisementId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AdvertisementId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Advertisement {
    pub id: Uuid,
    pub name: String,
    pub ad_number: String,
    pub provider_id: Uuid,
    pub placement: AdPlacement,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub image_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url_mobile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_reference: Option<String>,
    pub target_zones: serde_json::Value,
    pub target_services: serde_json::Value,
    pub target_audience: String,
    pub status: AdStatus,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub currency: String,
    pub billing_model: AdBillingModel,
    pub bid_amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_budget: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_budget: Option<Decimal>,
    pub budget_spent: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_impressions: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_clicks: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_cap: Option<i32>,
    pub total_impressions: i32,
    pub total_clicks: i32,
    pub total_conversions: i32,
    pub conversion_value: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_through_rate: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion_rate: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Advertisement {
    /// Create a builder for Advertisement
    pub fn builder() -> AdvertisementBuilder {
        AdvertisementBuilder::default()
    }

    /// Create a new Advertisement with required fields
    pub fn new(name: String, ad_number: String, provider_id: Uuid, placement: AdPlacement, image_url: String, target_zones: serde_json::Value, target_services: serde_json::Value, target_audience: String, status: AdStatus, start_at: DateTime<Utc>, end_at: DateTime<Utc>, currency: String, billing_model: AdBillingModel, bid_amount: Decimal, budget_spent: Decimal, total_impressions: i32, total_clicks: i32, total_conversions: i32, conversion_value: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            ad_number,
            provider_id,
            placement,
            position: None,
            headline: None,
            description: None,
            image_url,
            image_url_mobile: None,
            click_url: None,
            action_type: None,
            action_reference: None,
            target_zones,
            target_services,
            target_audience,
            status,
            start_at,
            end_at,
            currency,
            billing_model,
            bid_amount,
            daily_budget: None,
            total_budget: None,
            budget_spent,
            max_impressions: None,
            max_clicks: None,
            frequency_cap: None,
            total_impressions,
            total_clicks,
            total_conversions,
            conversion_value,
            click_through_rate: None,
            conversion_rate: None,
            approved_at: None,
            approved_by: None,
            rejection_reason: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AdvertisementId {
        AdvertisementId(self.id)
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
    pub fn status(&self) -> &AdStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the position field (chainable)
    pub fn with_position(mut self, value: i32) -> Self {
        self.position = Some(value);
        self
    }

    /// Set the headline field (chainable)
    pub fn with_headline(mut self, value: String) -> Self {
        self.headline = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the image_url_mobile field (chainable)
    pub fn with_image_url_mobile(mut self, value: String) -> Self {
        self.image_url_mobile = Some(value);
        self
    }

    /// Set the click_url field (chainable)
    pub fn with_click_url(mut self, value: String) -> Self {
        self.click_url = Some(value);
        self
    }

    /// Set the action_type field (chainable)
    pub fn with_action_type(mut self, value: String) -> Self {
        self.action_type = Some(value);
        self
    }

    /// Set the action_reference field (chainable)
    pub fn with_action_reference(mut self, value: String) -> Self {
        self.action_reference = Some(value);
        self
    }

    /// Set the daily_budget field (chainable)
    pub fn with_daily_budget(mut self, value: Decimal) -> Self {
        self.daily_budget = Some(value);
        self
    }

    /// Set the total_budget field (chainable)
    pub fn with_total_budget(mut self, value: Decimal) -> Self {
        self.total_budget = Some(value);
        self
    }

    /// Set the max_impressions field (chainable)
    pub fn with_max_impressions(mut self, value: i32) -> Self {
        self.max_impressions = Some(value);
        self
    }

    /// Set the max_clicks field (chainable)
    pub fn with_max_clicks(mut self, value: i32) -> Self {
        self.max_clicks = Some(value);
        self
    }

    /// Set the frequency_cap field (chainable)
    pub fn with_frequency_cap(mut self, value: i32) -> Self {
        self.frequency_cap = Some(value);
        self
    }

    /// Set the click_through_rate field (chainable)
    pub fn with_click_through_rate(mut self, value: Decimal) -> Self {
        self.click_through_rate = Some(value);
        self
    }

    /// Set the conversion_rate field (chainable)
    pub fn with_conversion_rate(mut self, value: Decimal) -> Self {
        self.conversion_rate = Some(value);
        self
    }

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
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
                "ad_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ad_number = v; }
                }
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "placement" => {
                    if let Ok(v) = serde_json::from_value(value) { self.placement = v; }
                }
                "position" => {
                    if let Ok(v) = serde_json::from_value(value) { self.position = v; }
                }
                "headline" => {
                    if let Ok(v) = serde_json::from_value(value) { self.headline = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "image_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.image_url = v; }
                }
                "image_url_mobile" => {
                    if let Ok(v) = serde_json::from_value(value) { self.image_url_mobile = v; }
                }
                "click_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.click_url = v; }
                }
                "action_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.action_type = v; }
                }
                "action_reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.action_reference = v; }
                }
                "target_zones" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_zones = v; }
                }
                "target_services" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_services = v; }
                }
                "target_audience" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_audience = v; }
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
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "billing_model" => {
                    if let Ok(v) = serde_json::from_value(value) { self.billing_model = v; }
                }
                "bid_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bid_amount = v; }
                }
                "daily_budget" => {
                    if let Ok(v) = serde_json::from_value(value) { self.daily_budget = v; }
                }
                "total_budget" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_budget = v; }
                }
                "budget_spent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.budget_spent = v; }
                }
                "max_impressions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_impressions = v; }
                }
                "max_clicks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_clicks = v; }
                }
                "frequency_cap" => {
                    if let Ok(v) = serde_json::from_value(value) { self.frequency_cap = v; }
                }
                "total_impressions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_impressions = v; }
                }
                "total_clicks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_clicks = v; }
                }
                "total_conversions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_conversions = v; }
                }
                "conversion_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_value = v; }
                }
                "click_through_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.click_through_rate = v; }
                }
                "conversion_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_rate = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Advertisement {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Advertisement"
    }
}

impl backbone_core::PersistentEntity for Advertisement {
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

impl backbone_orm::EntityRepoMeta for Advertisement {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("placement".to_string(), "ad_placement".to_string());
        m.insert("status".to_string(), "ad_status".to_string());
        m.insert("billing_model".to_string(), "ad_billing_model".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "ad_number", "image_url", "target_audience", "currency"]
    }
}

/// Builder for Advertisement entity
///
/// Provides a fluent API for constructing Advertisement instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AdvertisementBuilder {
    name: Option<String>,
    ad_number: Option<String>,
    provider_id: Option<Uuid>,
    placement: Option<AdPlacement>,
    position: Option<i32>,
    headline: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
    image_url_mobile: Option<String>,
    click_url: Option<String>,
    action_type: Option<String>,
    action_reference: Option<String>,
    target_zones: Option<serde_json::Value>,
    target_services: Option<serde_json::Value>,
    target_audience: Option<String>,
    status: Option<AdStatus>,
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
    currency: Option<String>,
    billing_model: Option<AdBillingModel>,
    bid_amount: Option<Decimal>,
    daily_budget: Option<Decimal>,
    total_budget: Option<Decimal>,
    budget_spent: Option<Decimal>,
    max_impressions: Option<i32>,
    max_clicks: Option<i32>,
    frequency_cap: Option<i32>,
    total_impressions: Option<i32>,
    total_clicks: Option<i32>,
    total_conversions: Option<i32>,
    conversion_value: Option<Decimal>,
    click_through_rate: Option<Decimal>,
    conversion_rate: Option<Decimal>,
    approved_at: Option<DateTime<Utc>>,
    approved_by: Option<Uuid>,
    rejection_reason: Option<String>,
}

impl AdvertisementBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the ad_number field (required)
    pub fn ad_number(mut self, value: String) -> Self {
        self.ad_number = Some(value);
        self
    }

    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the placement field (required)
    pub fn placement(mut self, value: AdPlacement) -> Self {
        self.placement = Some(value);
        self
    }

    /// Set the position field (optional)
    pub fn position(mut self, value: i32) -> Self {
        self.position = Some(value);
        self
    }

    /// Set the headline field (optional)
    pub fn headline(mut self, value: String) -> Self {
        self.headline = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the image_url field (required)
    pub fn image_url(mut self, value: String) -> Self {
        self.image_url = Some(value);
        self
    }

    /// Set the image_url_mobile field (optional)
    pub fn image_url_mobile(mut self, value: String) -> Self {
        self.image_url_mobile = Some(value);
        self
    }

    /// Set the click_url field (optional)
    pub fn click_url(mut self, value: String) -> Self {
        self.click_url = Some(value);
        self
    }

    /// Set the action_type field (optional)
    pub fn action_type(mut self, value: String) -> Self {
        self.action_type = Some(value);
        self
    }

    /// Set the action_reference field (optional)
    pub fn action_reference(mut self, value: String) -> Self {
        self.action_reference = Some(value);
        self
    }

    /// Set the target_zones field (default: `serde_json::json!([])`)
    pub fn target_zones(mut self, value: serde_json::Value) -> Self {
        self.target_zones = Some(value);
        self
    }

    /// Set the target_services field (default: `serde_json::json!([])`)
    pub fn target_services(mut self, value: serde_json::Value) -> Self {
        self.target_services = Some(value);
        self
    }

    /// Set the target_audience field (default: `"all".to_string()`)
    pub fn target_audience(mut self, value: String) -> Self {
        self.target_audience = Some(value);
        self
    }

    /// Set the status field (default: `AdStatus::default()`)
    pub fn status(mut self, value: AdStatus) -> Self {
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

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the billing_model field (default: `AdBillingModel::default()`)
    pub fn billing_model(mut self, value: AdBillingModel) -> Self {
        self.billing_model = Some(value);
        self
    }

    /// Set the bid_amount field (required)
    pub fn bid_amount(mut self, value: Decimal) -> Self {
        self.bid_amount = Some(value);
        self
    }

    /// Set the daily_budget field (optional)
    pub fn daily_budget(mut self, value: Decimal) -> Self {
        self.daily_budget = Some(value);
        self
    }

    /// Set the total_budget field (optional)
    pub fn total_budget(mut self, value: Decimal) -> Self {
        self.total_budget = Some(value);
        self
    }

    /// Set the budget_spent field (default: `Decimal::from(0)`)
    pub fn budget_spent(mut self, value: Decimal) -> Self {
        self.budget_spent = Some(value);
        self
    }

    /// Set the max_impressions field (optional)
    pub fn max_impressions(mut self, value: i32) -> Self {
        self.max_impressions = Some(value);
        self
    }

    /// Set the max_clicks field (optional)
    pub fn max_clicks(mut self, value: i32) -> Self {
        self.max_clicks = Some(value);
        self
    }

    /// Set the frequency_cap field (optional)
    pub fn frequency_cap(mut self, value: i32) -> Self {
        self.frequency_cap = Some(value);
        self
    }

    /// Set the total_impressions field (default: `0`)
    pub fn total_impressions(mut self, value: i32) -> Self {
        self.total_impressions = Some(value);
        self
    }

    /// Set the total_clicks field (default: `0`)
    pub fn total_clicks(mut self, value: i32) -> Self {
        self.total_clicks = Some(value);
        self
    }

    /// Set the total_conversions field (default: `0`)
    pub fn total_conversions(mut self, value: i32) -> Self {
        self.total_conversions = Some(value);
        self
    }

    /// Set the conversion_value field (default: `Decimal::from(0)`)
    pub fn conversion_value(mut self, value: Decimal) -> Self {
        self.conversion_value = Some(value);
        self
    }

    /// Set the click_through_rate field (optional)
    pub fn click_through_rate(mut self, value: Decimal) -> Self {
        self.click_through_rate = Some(value);
        self
    }

    /// Set the conversion_rate field (optional)
    pub fn conversion_rate(mut self, value: Decimal) -> Self {
        self.conversion_rate = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Build the Advertisement entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Advertisement, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let ad_number = self.ad_number.ok_or_else(|| "ad_number is required".to_string())?;
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let placement = self.placement.ok_or_else(|| "placement is required".to_string())?;
        let image_url = self.image_url.ok_or_else(|| "image_url is required".to_string())?;
        let start_at = self.start_at.ok_or_else(|| "start_at is required".to_string())?;
        let end_at = self.end_at.ok_or_else(|| "end_at is required".to_string())?;
        let bid_amount = self.bid_amount.ok_or_else(|| "bid_amount is required".to_string())?;

        Ok(Advertisement {
            id: Uuid::new_v4(),
            name,
            ad_number,
            provider_id,
            placement,
            position: self.position,
            headline: self.headline,
            description: self.description,
            image_url,
            image_url_mobile: self.image_url_mobile,
            click_url: self.click_url,
            action_type: self.action_type,
            action_reference: self.action_reference,
            target_zones: self.target_zones.unwrap_or(serde_json::json!([])),
            target_services: self.target_services.unwrap_or(serde_json::json!([])),
            target_audience: self.target_audience.unwrap_or("all".to_string()),
            status: self.status.unwrap_or(AdStatus::default()),
            start_at,
            end_at,
            currency: self.currency.unwrap_or("IDR".to_string()),
            billing_model: self.billing_model.unwrap_or(AdBillingModel::default()),
            bid_amount,
            daily_budget: self.daily_budget,
            total_budget: self.total_budget,
            budget_spent: self.budget_spent.unwrap_or(Decimal::from(0)),
            max_impressions: self.max_impressions,
            max_clicks: self.max_clicks,
            frequency_cap: self.frequency_cap,
            total_impressions: self.total_impressions.unwrap_or(0),
            total_clicks: self.total_clicks.unwrap_or(0),
            total_conversions: self.total_conversions.unwrap_or(0),
            conversion_value: self.conversion_value.unwrap_or(Decimal::from(0)),
            click_through_rate: self.click_through_rate,
            conversion_rate: self.conversion_rate,
            approved_at: self.approved_at,
            approved_by: self.approved_by,
            rejection_reason: self.rejection_reason,
            metadata: AuditMetadata::default(),
        })
    }
}
