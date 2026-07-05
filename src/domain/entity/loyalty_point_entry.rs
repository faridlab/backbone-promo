use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::LoyaltyEntryType;
use super::AuditMetadata;

/// Strongly-typed ID for LoyaltyPointEntry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoyaltyPointEntryId(pub Uuid);

impl LoyaltyPointEntryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LoyaltyPointEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LoyaltyPointEntryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LoyaltyPointEntryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LoyaltyPointEntryId> for Uuid {
    fn from(id: LoyaltyPointEntryId) -> Self { id.0 }
}

impl AsRef<Uuid> for LoyaltyPointEntryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LoyaltyPointEntryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoyaltyPointEntry {
    pub id: Uuid,
    pub company_id: Uuid,
    pub loyalty_program_id: Uuid,
    pub customer_id: Uuid,
    pub entry_type: LoyaltyEntryType,
    pub points: Decimal,
    pub purchase_amount: Decimal,
    pub source_type: String,
    pub source_id: Uuid,
    pub posting_date: DateTime<Utc>,
    pub expiry_date: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LoyaltyPointEntry {
    /// Create a builder for LoyaltyPointEntry
    pub fn builder() -> LoyaltyPointEntryBuilder {
        LoyaltyPointEntryBuilder::default()
    }

    /// Create a new LoyaltyPointEntry with required fields
    pub fn new(company_id: Uuid, loyalty_program_id: Uuid, customer_id: Uuid, entry_type: LoyaltyEntryType, points: Decimal, purchase_amount: Decimal, source_type: String, source_id: Uuid, posting_date: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            loyalty_program_id,
            customer_id,
            entry_type,
            points,
            purchase_amount,
            source_type,
            source_id,
            posting_date,
            expiry_date: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LoyaltyPointEntryId {
        LoyaltyPointEntryId(self.id)
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

    /// Set the expiry_date field (chainable)
    pub fn with_expiry_date(mut self, value: DateTime<Utc>) -> Self {
        self.expiry_date = Some(value);
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
                "entry_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.entry_type = v; }
                }
                "points" => {
                    if let Ok(v) = serde_json::from_value(value) { self.points = v; }
                }
                "purchase_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_amount = v; }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_type = v; }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "expiry_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_date = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LoyaltyPointEntry {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LoyaltyPointEntry"
    }
}

impl backbone_core::PersistentEntity for LoyaltyPointEntry {
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

impl backbone_orm::EntityRepoMeta for LoyaltyPointEntry {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("loyalty_program_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("entry_type".to_string(), "loyalty_entry_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["source_type"]
    }
}

/// Builder for LoyaltyPointEntry entity
///
/// Provides a fluent API for constructing LoyaltyPointEntry instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LoyaltyPointEntryBuilder {
    company_id: Option<Uuid>,
    loyalty_program_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    entry_type: Option<LoyaltyEntryType>,
    points: Option<Decimal>,
    purchase_amount: Option<Decimal>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    posting_date: Option<DateTime<Utc>>,
    expiry_date: Option<DateTime<Utc>>,
}

impl LoyaltyPointEntryBuilder {
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

    /// Set the entry_type field (required)
    pub fn entry_type(mut self, value: LoyaltyEntryType) -> Self {
        self.entry_type = Some(value);
        self
    }

    /// Set the points field (required)
    pub fn points(mut self, value: Decimal) -> Self {
        self.points = Some(value);
        self
    }

    /// Set the purchase_amount field (default: `Decimal::from(0)`)
    pub fn purchase_amount(mut self, value: Decimal) -> Self {
        self.purchase_amount = Some(value);
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

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: DateTime<Utc>) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the expiry_date field (optional)
    pub fn expiry_date(mut self, value: DateTime<Utc>) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Build the LoyaltyPointEntry entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LoyaltyPointEntry, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let loyalty_program_id = self.loyalty_program_id.ok_or_else(|| "loyalty_program_id is required".to_string())?;
        let customer_id = self.customer_id.ok_or_else(|| "customer_id is required".to_string())?;
        let entry_type = self.entry_type.ok_or_else(|| "entry_type is required".to_string())?;
        let points = self.points.ok_or_else(|| "points is required".to_string())?;
        let source_type = self.source_type.ok_or_else(|| "source_type is required".to_string())?;
        let source_id = self.source_id.ok_or_else(|| "source_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;

        Ok(LoyaltyPointEntry {
            id: Uuid::new_v4(),
            company_id,
            loyalty_program_id,
            customer_id,
            entry_type,
            points,
            purchase_amount: self.purchase_amount.unwrap_or(Decimal::from(0)),
            source_type,
            source_id,
            posting_date,
            expiry_date: self.expiry_date,
            metadata: AuditMetadata::default(),
        })
    }
}
