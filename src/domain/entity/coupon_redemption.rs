use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for CouponRedemption
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CouponRedemptionId(pub Uuid);

impl CouponRedemptionId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for CouponRedemptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for CouponRedemptionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for CouponRedemptionId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<CouponRedemptionId> for Uuid {
    fn from(id: CouponRedemptionId) -> Self { id.0 }
}

impl AsRef<Uuid> for CouponRedemptionId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for CouponRedemptionId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CouponRedemption {
    pub id: Uuid,
    pub company_id: Uuid,
    pub coupon_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub redeemed_at: DateTime<Utc>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl CouponRedemption {
    /// Create a builder for CouponRedemption
    pub fn builder() -> CouponRedemptionBuilder {
        CouponRedemptionBuilder::default()
    }

    /// Create a new CouponRedemption with required fields
    pub fn new(company_id: Uuid, coupon_id: Uuid, pricing_rule_id: Uuid, source_type: String, source_id: Uuid, redeemed_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            coupon_id,
            pricing_rule_id,
            source_type,
            source_id,
            redeemed_at,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> CouponRedemptionId {
        CouponRedemptionId(self.id)
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
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "coupon_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.coupon_id = v; }
                }
                "pricing_rule_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.pricing_rule_id = v; }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_type = v; }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_id = v; }
                }
                "redeemed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.redeemed_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for CouponRedemption {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "CouponRedemption"
    }
}

impl backbone_core::PersistentEntity for CouponRedemption {
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

impl backbone_orm::EntityRepoMeta for CouponRedemption {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("coupon_id".to_string(), "uuid".to_string());
        m.insert("pricing_rule_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["source_type"]
    }
}

/// Builder for CouponRedemption entity
///
/// Provides a fluent API for constructing CouponRedemption instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct CouponRedemptionBuilder {
    company_id: Option<Uuid>,
    coupon_id: Option<Uuid>,
    pricing_rule_id: Option<Uuid>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    redeemed_at: Option<DateTime<Utc>>,
}

impl CouponRedemptionBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the coupon_id field (required)
    pub fn coupon_id(mut self, value: Uuid) -> Self {
        self.coupon_id = Some(value);
        self
    }

    /// Set the pricing_rule_id field (required)
    pub fn pricing_rule_id(mut self, value: Uuid) -> Self {
        self.pricing_rule_id = Some(value);
        self
    }

    /// Set the source_type field (required)
    pub fn source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_id field (required)
    pub fn source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the redeemed_at field (default: `Utc::now()`)
    pub fn redeemed_at(mut self, value: DateTime<Utc>) -> Self {
        self.redeemed_at = Some(value);
        self
    }

    /// Build the CouponRedemption entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<CouponRedemption, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let coupon_id = self.coupon_id.ok_or_else(|| "coupon_id is required".to_string())?;
        let pricing_rule_id = self.pricing_rule_id.ok_or_else(|| "pricing_rule_id is required".to_string())?;
        let source_type = self.source_type.ok_or_else(|| "source_type is required".to_string())?;
        let source_id = self.source_id.ok_or_else(|| "source_id is required".to_string())?;

        Ok(CouponRedemption {
            id: Uuid::new_v4(),
            company_id,
            coupon_id,
            pricing_rule_id,
            source_type,
            source_id,
            redeemed_at: self.redeemed_at.unwrap_or(Utc::now()),
            metadata: AuditMetadata::default(),
        })
    }
}
