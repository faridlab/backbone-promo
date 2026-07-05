use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ApplyOn;
use super::AuditMetadata;

/// Strongly-typed ID for PromoBundleComponent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromoBundleComponentId(pub Uuid);

impl PromoBundleComponentId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PromoBundleComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PromoBundleComponentId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PromoBundleComponentId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PromoBundleComponentId> for Uuid {
    fn from(id: PromoBundleComponentId) -> Self { id.0 }
}

impl AsRef<Uuid> for PromoBundleComponentId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PromoBundleComponentId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PromoBundleComponent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub bundle_id: Uuid,
    pub apply_on: ApplyOn,
    pub item_id: Option<Uuid>,
    pub item_group_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub min_qty: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PromoBundleComponent {
    /// Create a builder for PromoBundleComponent
    pub fn builder() -> PromoBundleComponentBuilder {
        PromoBundleComponentBuilder::default()
    }

    /// Create a new PromoBundleComponent with required fields
    pub fn new(company_id: Uuid, bundle_id: Uuid, apply_on: ApplyOn, min_qty: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            bundle_id,
            apply_on,
            item_id: None,
            item_group_id: None,
            brand_id: None,
            min_qty,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PromoBundleComponentId {
        PromoBundleComponentId(self.id)
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
                "bundle_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bundle_id = v; }
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
                "min_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_qty = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for PromoBundleComponent {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PromoBundleComponent"
    }
}

impl backbone_core::PersistentEntity for PromoBundleComponent {
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

impl backbone_orm::EntityRepoMeta for PromoBundleComponent {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("bundle_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("item_group_id".to_string(), "uuid".to_string());
        m.insert("brand_id".to_string(), "uuid".to_string());
        m.insert("apply_on".to_string(), "apply_on".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for PromoBundleComponent entity
///
/// Provides a fluent API for constructing PromoBundleComponent instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PromoBundleComponentBuilder {
    company_id: Option<Uuid>,
    bundle_id: Option<Uuid>,
    apply_on: Option<ApplyOn>,
    item_id: Option<Uuid>,
    item_group_id: Option<Uuid>,
    brand_id: Option<Uuid>,
    min_qty: Option<Decimal>,
}

impl PromoBundleComponentBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the bundle_id field (required)
    pub fn bundle_id(mut self, value: Uuid) -> Self {
        self.bundle_id = Some(value);
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

    /// Set the min_qty field (default: `Decimal::from(1)`)
    pub fn min_qty(mut self, value: Decimal) -> Self {
        self.min_qty = Some(value);
        self
    }

    /// Build the PromoBundleComponent entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PromoBundleComponent, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let bundle_id = self.bundle_id.ok_or_else(|| "bundle_id is required".to_string())?;

        Ok(PromoBundleComponent {
            id: Uuid::new_v4(),
            company_id,
            bundle_id,
            apply_on: self.apply_on.unwrap_or(ApplyOn::default()),
            item_id: self.item_id,
            item_group_id: self.item_group_id,
            brand_id: self.brand_id,
            min_qty: self.min_qty.unwrap_or(Decimal::from(1)),
            metadata: AuditMetadata::default(),
        })
    }
}
