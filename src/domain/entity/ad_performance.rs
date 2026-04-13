use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AdType;
use super::AdPlacement;
use super::AdStatus;
use super::AuditMetadata;

/// Strongly-typed ID for AdPerformance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdPerformanceId(pub Uuid);

impl AdPerformanceId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AdPerformanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AdPerformanceId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AdPerformanceId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AdPerformanceId> for Uuid {
    fn from(id: AdPerformanceId) -> Self { id.0 }
}

impl AsRef<Uuid> for AdPerformanceId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AdPerformanceId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdPerformance {
    pub id: Uuid,
    pub advertisement_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    pub date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hour: Option<i32>,
    pub ad_type: AdType,
    pub placement: AdPlacement,
    pub status: AdStatus,
    pub impressions: i32,
    pub unique_impressions: i32,
    pub clicks: i32,
    pub unique_clicks: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctr: Option<Decimal>,
    pub conversions: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion_rate: Option<Decimal>,
    pub conversion_value: Decimal,
    pub spend: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_remaining: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpc: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpm: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpa: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roi: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roas: Option<Decimal>,
    pub shares: i32,
    pub saves: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engagement_rate: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_view_duration_seconds: Option<i32>,
    pub reach: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<Decimal>,
    pub audience_demographics: serde_json::Value,
    pub by_device: serde_json::Value,
    pub by_platform: serde_json::Value,
    pub by_region: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<Decimal>,
    pub currency: String,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl AdPerformance {
    /// Create a builder for AdPerformance
    pub fn builder() -> AdPerformanceBuilder {
        AdPerformanceBuilder::default()
    }

    /// Create a new AdPerformance with required fields
    pub fn new(advertisement_id: Uuid, date: NaiveDate, ad_type: AdType, placement: AdPlacement, status: AdStatus, impressions: i32, unique_impressions: i32, clicks: i32, unique_clicks: i32, conversions: i32, conversion_value: Decimal, spend: Decimal, shares: i32, saves: i32, reach: i32, audience_demographics: serde_json::Value, by_device: serde_json::Value, by_platform: serde_json::Value, by_region: serde_json::Value, currency: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            advertisement_id,
            provider_id: None,
            date,
            hour: None,
            ad_type,
            placement,
            status,
            impressions,
            unique_impressions,
            clicks,
            unique_clicks,
            ctr: None,
            conversions,
            conversion_rate: None,
            conversion_value,
            spend,
            budget_remaining: None,
            cpc: None,
            cpm: None,
            cpa: None,
            roi: None,
            roas: None,
            shares,
            saves,
            engagement_rate: None,
            avg_view_duration_seconds: None,
            reach,
            frequency: None,
            audience_demographics,
            by_device,
            by_platform,
            by_region,
            quality_score: None,
            relevance_score: None,
            currency,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AdPerformanceId {
        AdPerformanceId(self.id)
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

    /// Set the provider_id field (chainable)
    pub fn with_provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the hour field (chainable)
    pub fn with_hour(mut self, value: i32) -> Self {
        self.hour = Some(value);
        self
    }

    /// Set the ctr field (chainable)
    pub fn with_ctr(mut self, value: Decimal) -> Self {
        self.ctr = Some(value);
        self
    }

    /// Set the conversion_rate field (chainable)
    pub fn with_conversion_rate(mut self, value: Decimal) -> Self {
        self.conversion_rate = Some(value);
        self
    }

    /// Set the budget_remaining field (chainable)
    pub fn with_budget_remaining(mut self, value: Decimal) -> Self {
        self.budget_remaining = Some(value);
        self
    }

    /// Set the cpc field (chainable)
    pub fn with_cpc(mut self, value: Decimal) -> Self {
        self.cpc = Some(value);
        self
    }

    /// Set the cpm field (chainable)
    pub fn with_cpm(mut self, value: Decimal) -> Self {
        self.cpm = Some(value);
        self
    }

    /// Set the cpa field (chainable)
    pub fn with_cpa(mut self, value: Decimal) -> Self {
        self.cpa = Some(value);
        self
    }

    /// Set the roi field (chainable)
    pub fn with_roi(mut self, value: Decimal) -> Self {
        self.roi = Some(value);
        self
    }

    /// Set the roas field (chainable)
    pub fn with_roas(mut self, value: Decimal) -> Self {
        self.roas = Some(value);
        self
    }

    /// Set the engagement_rate field (chainable)
    pub fn with_engagement_rate(mut self, value: Decimal) -> Self {
        self.engagement_rate = Some(value);
        self
    }

    /// Set the avg_view_duration_seconds field (chainable)
    pub fn with_avg_view_duration_seconds(mut self, value: i32) -> Self {
        self.avg_view_duration_seconds = Some(value);
        self
    }

    /// Set the frequency field (chainable)
    pub fn with_frequency(mut self, value: Decimal) -> Self {
        self.frequency = Some(value);
        self
    }

    /// Set the quality_score field (chainable)
    pub fn with_quality_score(mut self, value: Decimal) -> Self {
        self.quality_score = Some(value);
        self
    }

    /// Set the relevance_score field (chainable)
    pub fn with_relevance_score(mut self, value: Decimal) -> Self {
        self.relevance_score = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "advertisement_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.advertisement_id = v; }
                }
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date = v; }
                }
                "hour" => {
                    if let Ok(v) = serde_json::from_value(value) { self.hour = v; }
                }
                "ad_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ad_type = v; }
                }
                "placement" => {
                    if let Ok(v) = serde_json::from_value(value) { self.placement = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "impressions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.impressions = v; }
                }
                "unique_impressions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unique_impressions = v; }
                }
                "clicks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.clicks = v; }
                }
                "unique_clicks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unique_clicks = v; }
                }
                "ctr" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ctr = v; }
                }
                "conversions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversions = v; }
                }
                "conversion_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_rate = v; }
                }
                "conversion_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_value = v; }
                }
                "spend" => {
                    if let Ok(v) = serde_json::from_value(value) { self.spend = v; }
                }
                "budget_remaining" => {
                    if let Ok(v) = serde_json::from_value(value) { self.budget_remaining = v; }
                }
                "cpc" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cpc = v; }
                }
                "cpm" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cpm = v; }
                }
                "cpa" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cpa = v; }
                }
                "roi" => {
                    if let Ok(v) = serde_json::from_value(value) { self.roi = v; }
                }
                "roas" => {
                    if let Ok(v) = serde_json::from_value(value) { self.roas = v; }
                }
                "shares" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shares = v; }
                }
                "saves" => {
                    if let Ok(v) = serde_json::from_value(value) { self.saves = v; }
                }
                "engagement_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.engagement_rate = v; }
                }
                "avg_view_duration_seconds" => {
                    if let Ok(v) = serde_json::from_value(value) { self.avg_view_duration_seconds = v; }
                }
                "reach" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reach = v; }
                }
                "frequency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.frequency = v; }
                }
                "audience_demographics" => {
                    if let Ok(v) = serde_json::from_value(value) { self.audience_demographics = v; }
                }
                "by_device" => {
                    if let Ok(v) = serde_json::from_value(value) { self.by_device = v; }
                }
                "by_platform" => {
                    if let Ok(v) = serde_json::from_value(value) { self.by_platform = v; }
                }
                "by_region" => {
                    if let Ok(v) = serde_json::from_value(value) { self.by_region = v; }
                }
                "quality_score" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quality_score = v; }
                }
                "relevance_score" => {
                    if let Ok(v) = serde_json::from_value(value) { self.relevance_score = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for AdPerformance {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "AdPerformance"
    }
}

impl backbone_core::PersistentEntity for AdPerformance {
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

impl backbone_orm::EntityRepoMeta for AdPerformance {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("advertisement_id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("ad_type".to_string(), "ad_type".to_string());
        m.insert("placement".to_string(), "ad_placement".to_string());
        m.insert("status".to_string(), "ad_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
}

/// Builder for AdPerformance entity
///
/// Provides a fluent API for constructing AdPerformance instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AdPerformanceBuilder {
    advertisement_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    date: Option<NaiveDate>,
    hour: Option<i32>,
    ad_type: Option<AdType>,
    placement: Option<AdPlacement>,
    status: Option<AdStatus>,
    impressions: Option<i32>,
    unique_impressions: Option<i32>,
    clicks: Option<i32>,
    unique_clicks: Option<i32>,
    ctr: Option<Decimal>,
    conversions: Option<i32>,
    conversion_rate: Option<Decimal>,
    conversion_value: Option<Decimal>,
    spend: Option<Decimal>,
    budget_remaining: Option<Decimal>,
    cpc: Option<Decimal>,
    cpm: Option<Decimal>,
    cpa: Option<Decimal>,
    roi: Option<Decimal>,
    roas: Option<Decimal>,
    shares: Option<i32>,
    saves: Option<i32>,
    engagement_rate: Option<Decimal>,
    avg_view_duration_seconds: Option<i32>,
    reach: Option<i32>,
    frequency: Option<Decimal>,
    audience_demographics: Option<serde_json::Value>,
    by_device: Option<serde_json::Value>,
    by_platform: Option<serde_json::Value>,
    by_region: Option<serde_json::Value>,
    quality_score: Option<Decimal>,
    relevance_score: Option<Decimal>,
    currency: Option<String>,
}

impl AdPerformanceBuilder {
    /// Set the advertisement_id field (required)
    pub fn advertisement_id(mut self, value: Uuid) -> Self {
        self.advertisement_id = Some(value);
        self
    }

    /// Set the provider_id field (optional)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the date field (required)
    pub fn date(mut self, value: NaiveDate) -> Self {
        self.date = Some(value);
        self
    }

    /// Set the hour field (optional)
    pub fn hour(mut self, value: i32) -> Self {
        self.hour = Some(value);
        self
    }

    /// Set the ad_type field (required)
    pub fn ad_type(mut self, value: AdType) -> Self {
        self.ad_type = Some(value);
        self
    }

    /// Set the placement field (required)
    pub fn placement(mut self, value: AdPlacement) -> Self {
        self.placement = Some(value);
        self
    }

    /// Set the status field (required)
    pub fn status(mut self, value: AdStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the impressions field (default: `0`)
    pub fn impressions(mut self, value: i32) -> Self {
        self.impressions = Some(value);
        self
    }

    /// Set the unique_impressions field (default: `0`)
    pub fn unique_impressions(mut self, value: i32) -> Self {
        self.unique_impressions = Some(value);
        self
    }

    /// Set the clicks field (default: `0`)
    pub fn clicks(mut self, value: i32) -> Self {
        self.clicks = Some(value);
        self
    }

    /// Set the unique_clicks field (default: `0`)
    pub fn unique_clicks(mut self, value: i32) -> Self {
        self.unique_clicks = Some(value);
        self
    }

    /// Set the ctr field (optional)
    pub fn ctr(mut self, value: Decimal) -> Self {
        self.ctr = Some(value);
        self
    }

    /// Set the conversions field (default: `0`)
    pub fn conversions(mut self, value: i32) -> Self {
        self.conversions = Some(value);
        self
    }

    /// Set the conversion_rate field (optional)
    pub fn conversion_rate(mut self, value: Decimal) -> Self {
        self.conversion_rate = Some(value);
        self
    }

    /// Set the conversion_value field (default: `Decimal::from(0)`)
    pub fn conversion_value(mut self, value: Decimal) -> Self {
        self.conversion_value = Some(value);
        self
    }

    /// Set the spend field (default: `Decimal::from(0)`)
    pub fn spend(mut self, value: Decimal) -> Self {
        self.spend = Some(value);
        self
    }

    /// Set the budget_remaining field (optional)
    pub fn budget_remaining(mut self, value: Decimal) -> Self {
        self.budget_remaining = Some(value);
        self
    }

    /// Set the cpc field (optional)
    pub fn cpc(mut self, value: Decimal) -> Self {
        self.cpc = Some(value);
        self
    }

    /// Set the cpm field (optional)
    pub fn cpm(mut self, value: Decimal) -> Self {
        self.cpm = Some(value);
        self
    }

    /// Set the cpa field (optional)
    pub fn cpa(mut self, value: Decimal) -> Self {
        self.cpa = Some(value);
        self
    }

    /// Set the roi field (optional)
    pub fn roi(mut self, value: Decimal) -> Self {
        self.roi = Some(value);
        self
    }

    /// Set the roas field (optional)
    pub fn roas(mut self, value: Decimal) -> Self {
        self.roas = Some(value);
        self
    }

    /// Set the shares field (default: `0`)
    pub fn shares(mut self, value: i32) -> Self {
        self.shares = Some(value);
        self
    }

    /// Set the saves field (default: `0`)
    pub fn saves(mut self, value: i32) -> Self {
        self.saves = Some(value);
        self
    }

    /// Set the engagement_rate field (optional)
    pub fn engagement_rate(mut self, value: Decimal) -> Self {
        self.engagement_rate = Some(value);
        self
    }

    /// Set the avg_view_duration_seconds field (optional)
    pub fn avg_view_duration_seconds(mut self, value: i32) -> Self {
        self.avg_view_duration_seconds = Some(value);
        self
    }

    /// Set the reach field (default: `0`)
    pub fn reach(mut self, value: i32) -> Self {
        self.reach = Some(value);
        self
    }

    /// Set the frequency field (optional)
    pub fn frequency(mut self, value: Decimal) -> Self {
        self.frequency = Some(value);
        self
    }

    /// Set the audience_demographics field (default: `serde_json::json!({})`)
    pub fn audience_demographics(mut self, value: serde_json::Value) -> Self {
        self.audience_demographics = Some(value);
        self
    }

    /// Set the by_device field (default: `serde_json::json!({})`)
    pub fn by_device(mut self, value: serde_json::Value) -> Self {
        self.by_device = Some(value);
        self
    }

    /// Set the by_platform field (default: `serde_json::json!({})`)
    pub fn by_platform(mut self, value: serde_json::Value) -> Self {
        self.by_platform = Some(value);
        self
    }

    /// Set the by_region field (default: `serde_json::json!({})`)
    pub fn by_region(mut self, value: serde_json::Value) -> Self {
        self.by_region = Some(value);
        self
    }

    /// Set the quality_score field (optional)
    pub fn quality_score(mut self, value: Decimal) -> Self {
        self.quality_score = Some(value);
        self
    }

    /// Set the relevance_score field (optional)
    pub fn relevance_score(mut self, value: Decimal) -> Self {
        self.relevance_score = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Build the AdPerformance entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<AdPerformance, String> {
        let advertisement_id = self.advertisement_id.ok_or_else(|| "advertisement_id is required".to_string())?;
        let date = self.date.ok_or_else(|| "date is required".to_string())?;
        let ad_type = self.ad_type.ok_or_else(|| "ad_type is required".to_string())?;
        let placement = self.placement.ok_or_else(|| "placement is required".to_string())?;
        let status = self.status.ok_or_else(|| "status is required".to_string())?;

        Ok(AdPerformance {
            id: Uuid::new_v4(),
            advertisement_id,
            provider_id: self.provider_id,
            date,
            hour: self.hour,
            ad_type,
            placement,
            status,
            impressions: self.impressions.unwrap_or(0),
            unique_impressions: self.unique_impressions.unwrap_or(0),
            clicks: self.clicks.unwrap_or(0),
            unique_clicks: self.unique_clicks.unwrap_or(0),
            ctr: self.ctr,
            conversions: self.conversions.unwrap_or(0),
            conversion_rate: self.conversion_rate,
            conversion_value: self.conversion_value.unwrap_or(Decimal::from(0)),
            spend: self.spend.unwrap_or(Decimal::from(0)),
            budget_remaining: self.budget_remaining,
            cpc: self.cpc,
            cpm: self.cpm,
            cpa: self.cpa,
            roi: self.roi,
            roas: self.roas,
            shares: self.shares.unwrap_or(0),
            saves: self.saves.unwrap_or(0),
            engagement_rate: self.engagement_rate,
            avg_view_duration_seconds: self.avg_view_duration_seconds,
            reach: self.reach.unwrap_or(0),
            frequency: self.frequency,
            audience_demographics: self.audience_demographics.unwrap_or(serde_json::json!({})),
            by_device: self.by_device.unwrap_or(serde_json::json!({})),
            by_platform: self.by_platform.unwrap_or(serde_json::json!({})),
            by_region: self.by_region.unwrap_or(serde_json::json!({})),
            quality_score: self.quality_score,
            relevance_score: self.relevance_score,
            currency: self.currency.unwrap_or("IDR".to_string()),
            metadata: AuditMetadata::default(),
        })
    }
}
