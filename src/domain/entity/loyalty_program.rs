use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::LoyaltyProgramType;
use super::AuditMetadata;

/// Strongly-typed ID for LoyaltyProgram
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoyaltyProgramId(pub Uuid);

impl LoyaltyProgramId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LoyaltyProgramId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LoyaltyProgramId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LoyaltyProgramId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LoyaltyProgramId> for Uuid {
    fn from(id: LoyaltyProgramId) -> Self { id.0 }
}

impl AsRef<Uuid> for LoyaltyProgramId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LoyaltyProgramId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoyaltyProgram {
    pub id: Uuid,
    pub company_id: Uuid,
    pub program_name: String,
    pub program_type: LoyaltyProgramType,
    pub collection_factor: Decimal,
    pub conversion_factor: Decimal,
    pub expiry_duration_days: Option<i32>,
    pub from_date: DateTime<Utc>,
    pub to_date: Option<DateTime<Utc>>,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LoyaltyProgram {
    /// Create a builder for LoyaltyProgram
    pub fn builder() -> LoyaltyProgramBuilder {
        LoyaltyProgramBuilder::default()
    }

    /// Create a new LoyaltyProgram with required fields
    pub fn new(company_id: Uuid, program_name: String, program_type: LoyaltyProgramType, collection_factor: Decimal, conversion_factor: Decimal, from_date: DateTime<Utc>, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            program_name,
            program_type,
            collection_factor,
            conversion_factor,
            expiry_duration_days: None,
            from_date,
            to_date: None,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LoyaltyProgramId {
        LoyaltyProgramId(self.id)
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

    /// Set the expiry_duration_days field (chainable)
    pub fn with_expiry_duration_days(mut self, value: i32) -> Self {
        self.expiry_duration_days = Some(value);
        self
    }

    /// Set the to_date field (chainable)
    pub fn with_to_date(mut self, value: DateTime<Utc>) -> Self {
        self.to_date = Some(value);
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
                "program_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.program_name = v; }
                }
                "program_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.program_type = v; }
                }
                "collection_factor" => {
                    if let Ok(v) = serde_json::from_value(value) { self.collection_factor = v; }
                }
                "conversion_factor" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_factor = v; }
                }
                "expiry_duration_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_duration_days = v; }
                }
                "from_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_date = v; }
                }
                "to_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_date = v; }
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

impl super::Entity for LoyaltyProgram {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LoyaltyProgram"
    }
}

impl backbone_core::PersistentEntity for LoyaltyProgram {
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

impl backbone_orm::EntityRepoMeta for LoyaltyProgram {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("program_type".to_string(), "loyalty_program_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["program_name"]
    }
}

/// Builder for LoyaltyProgram entity
///
/// Provides a fluent API for constructing LoyaltyProgram instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LoyaltyProgramBuilder {
    company_id: Option<Uuid>,
    program_name: Option<String>,
    program_type: Option<LoyaltyProgramType>,
    collection_factor: Option<Decimal>,
    conversion_factor: Option<Decimal>,
    expiry_duration_days: Option<i32>,
    from_date: Option<DateTime<Utc>>,
    to_date: Option<DateTime<Utc>>,
    is_active: Option<bool>,
}

impl LoyaltyProgramBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the program_name field (required)
    pub fn program_name(mut self, value: String) -> Self {
        self.program_name = Some(value);
        self
    }

    /// Set the program_type field (default: `LoyaltyProgramType::default()`)
    pub fn program_type(mut self, value: LoyaltyProgramType) -> Self {
        self.program_type = Some(value);
        self
    }

    /// Set the collection_factor field (required)
    pub fn collection_factor(mut self, value: Decimal) -> Self {
        self.collection_factor = Some(value);
        self
    }

    /// Set the conversion_factor field (required)
    pub fn conversion_factor(mut self, value: Decimal) -> Self {
        self.conversion_factor = Some(value);
        self
    }

    /// Set the expiry_duration_days field (optional)
    pub fn expiry_duration_days(mut self, value: i32) -> Self {
        self.expiry_duration_days = Some(value);
        self
    }

    /// Set the from_date field (required)
    pub fn from_date(mut self, value: DateTime<Utc>) -> Self {
        self.from_date = Some(value);
        self
    }

    /// Set the to_date field (optional)
    pub fn to_date(mut self, value: DateTime<Utc>) -> Self {
        self.to_date = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the LoyaltyProgram entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LoyaltyProgram, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let program_name = self.program_name.ok_or_else(|| "program_name is required".to_string())?;
        let collection_factor = self.collection_factor.ok_or_else(|| "collection_factor is required".to_string())?;
        let conversion_factor = self.conversion_factor.ok_or_else(|| "conversion_factor is required".to_string())?;
        let from_date = self.from_date.ok_or_else(|| "from_date is required".to_string())?;

        Ok(LoyaltyProgram {
            id: Uuid::new_v4(),
            company_id,
            program_name,
            program_type: self.program_type.unwrap_or(LoyaltyProgramType::default()),
            collection_factor,
            conversion_factor,
            expiry_duration_days: self.expiry_duration_days,
            from_date,
            to_date: self.to_date,
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
