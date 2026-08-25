use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for LoyaltyOrderPoints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoyaltyOrderPointsId(pub Uuid);

impl LoyaltyOrderPointsId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LoyaltyOrderPointsId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LoyaltyOrderPointsId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LoyaltyOrderPointsId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LoyaltyOrderPointsId> for Uuid {
    fn from(id: LoyaltyOrderPointsId) -> Self { id.0 }
}

impl AsRef<Uuid> for LoyaltyOrderPointsId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LoyaltyOrderPointsId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoyaltyOrderPoints {
    pub id: Uuid,
    pub company_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub customer_id: Uuid,
    pub order_ref_type: String,
    pub order_ref_id: Uuid,
    pub coupon_code_id: Option<Uuid>,
    pub grant_base_amount: Decimal,
    pub granted_points: Decimal,
    pub spent_points: Decimal,
    pub granted_reversed_points: Decimal,
    pub spent_reversed_points: Decimal,
    pub granted_at: Option<DateTime<Utc>>,
    pub spent_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LoyaltyOrderPoints {
    /// Create a builder for LoyaltyOrderPoints
    pub fn builder() -> LoyaltyOrderPointsBuilder {
        <LoyaltyOrderPointsBuilder as Default>::default()
    }

    /// Create a new LoyaltyOrderPoints with required fields
    pub fn new(company_id: Uuid, loyalty_program_id: Uuid, customer_id: Uuid, order_ref_type: String, order_ref_id: Uuid, grant_base_amount: Decimal, granted_points: Decimal, spent_points: Decimal, granted_reversed_points: Decimal, spent_reversed_points: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            loyalty_program_id,
            customer_id,
            order_ref_type,
            order_ref_id,
            coupon_code_id: None,
            grant_base_amount,
            granted_points,
            spent_points,
            granted_reversed_points,
            spent_reversed_points,
            granted_at: None,
            spent_at: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LoyaltyOrderPointsId {
        LoyaltyOrderPointsId(self.id)
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

    /// Set the coupon_code_id field (chainable)
    pub fn with_coupon_code_id(mut self, value: Uuid) -> Self {
        self.coupon_code_id = Some(value);
        self
    }

    /// Set the granted_at field (chainable)
    pub fn with_granted_at(mut self, value: DateTime<Utc>) -> Self {
        self.granted_at = Some(value);
        self
    }

    /// Set the spent_at field (chainable)
    pub fn with_spent_at(mut self, value: DateTime<Utc>) -> Self {
        self.spent_at = Some(value);
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
                "loyalty_program_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.loyalty_program_id = v; }
                }
                "customer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.customer_id = v; }
                }
                "order_ref_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.order_ref_type = v; }
                }
                "order_ref_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.order_ref_id = v; }
                }
                "coupon_code_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.coupon_code_id = v; }
                }
                "grant_base_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.grant_base_amount = v; }
                }
                "granted_points" => {
                    if let Ok(v) = serde_json::from_value(value) { self.granted_points = v; }
                }
                "spent_points" => {
                    if let Ok(v) = serde_json::from_value(value) { self.spent_points = v; }
                }
                "granted_reversed_points" => {
                    if let Ok(v) = serde_json::from_value(value) { self.granted_reversed_points = v; }
                }
                "spent_reversed_points" => {
                    if let Ok(v) = serde_json::from_value(value) { self.spent_reversed_points = v; }
                }
                "granted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.granted_at = v; }
                }
                "spent_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.spent_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LoyaltyOrderPoints {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LoyaltyOrderPoints"
    }
}

impl backbone_core::PersistentEntity for LoyaltyOrderPoints {
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

impl backbone_orm::EntityRepoMeta for LoyaltyOrderPoints {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("loyalty_program_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("order_ref_id".to_string(), "uuid".to_string());
        m.insert("coupon_code_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["order_ref_type"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for LoyaltyOrderPoints entity
///
/// Provides a fluent API for constructing LoyaltyOrderPoints instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LoyaltyOrderPointsBuilder {
    company_id: Option<Uuid>,
    loyalty_program_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    order_ref_type: Option<String>,
    order_ref_id: Option<Uuid>,
    coupon_code_id: Option<Uuid>,
    grant_base_amount: Option<Decimal>,
    granted_points: Option<Decimal>,
    spent_points: Option<Decimal>,
    granted_reversed_points: Option<Decimal>,
    spent_reversed_points: Option<Decimal>,
    granted_at: Option<DateTime<Utc>>,
    spent_at: Option<DateTime<Utc>>,
}

impl LoyaltyOrderPointsBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the loyalty_program_id field (required)
    pub fn loyalty_program_id(mut self, value: Uuid) -> Self {
        self.loyalty_program_id = Some(value);
        self
    }

    /// Set the customer_id field (required)
    pub fn customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the order_ref_type field (required)
    pub fn order_ref_type(mut self, value: String) -> Self {
        self.order_ref_type = Some(value);
        self
    }

    /// Set the order_ref_id field (required)
    pub fn order_ref_id(mut self, value: Uuid) -> Self {
        self.order_ref_id = Some(value);
        self
    }

    /// Set the coupon_code_id field (optional)
    pub fn coupon_code_id(mut self, value: Uuid) -> Self {
        self.coupon_code_id = Some(value);
        self
    }

    /// Set the grant_base_amount field (default: `Decimal::from(0)`)
    pub fn grant_base_amount(mut self, value: Decimal) -> Self {
        self.grant_base_amount = Some(value);
        self
    }

    /// Set the granted_points field (default: `Decimal::from(0)`)
    pub fn granted_points(mut self, value: Decimal) -> Self {
        self.granted_points = Some(value);
        self
    }

    /// Set the spent_points field (default: `Decimal::from(0)`)
    pub fn spent_points(mut self, value: Decimal) -> Self {
        self.spent_points = Some(value);
        self
    }

    /// Set the granted_reversed_points field (default: `Decimal::from(0)`)
    pub fn granted_reversed_points(mut self, value: Decimal) -> Self {
        self.granted_reversed_points = Some(value);
        self
    }

    /// Set the spent_reversed_points field (default: `Decimal::from(0)`)
    pub fn spent_reversed_points(mut self, value: Decimal) -> Self {
        self.spent_reversed_points = Some(value);
        self
    }

    /// Set the granted_at field (optional)
    pub fn granted_at(mut self, value: DateTime<Utc>) -> Self {
        self.granted_at = Some(value);
        self
    }

    /// Set the spent_at field (optional)
    pub fn spent_at(mut self, value: DateTime<Utc>) -> Self {
        self.spent_at = Some(value);
        self
    }

    /// Build the LoyaltyOrderPoints entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LoyaltyOrderPoints, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let loyalty_program_id = self.loyalty_program_id.ok_or_else(|| "loyalty_program_id is required".to_string())?;
        let customer_id = self.customer_id.ok_or_else(|| "customer_id is required".to_string())?;
        let order_ref_type = self.order_ref_type.ok_or_else(|| "order_ref_type is required".to_string())?;
        let order_ref_id = self.order_ref_id.ok_or_else(|| "order_ref_id is required".to_string())?;

        Ok(LoyaltyOrderPoints {
            id: Uuid::new_v4(),
            company_id,
            loyalty_program_id,
            customer_id,
            order_ref_type,
            order_ref_id,
            coupon_code_id: self.coupon_code_id,
            grant_base_amount: self.grant_base_amount.unwrap_or(Decimal::from(0)),
            granted_points: self.granted_points.unwrap_or(Decimal::from(0)),
            spent_points: self.spent_points.unwrap_or(Decimal::from(0)),
            granted_reversed_points: self.granted_reversed_points.unwrap_or(Decimal::from(0)),
            spent_reversed_points: self.spent_reversed_points.unwrap_or(Decimal::from(0)),
            granted_at: self.granted_at,
            spent_at: self.spent_at,
            metadata: AuditMetadata::default(),
        })
    }
}
