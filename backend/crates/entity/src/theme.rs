//! Per-tenant **theme** / white-label configuration: branding colours, logo, and
//! legal boilerplate templates. Exactly one row per tenant. Powers the frontend
//! theming system so a client can rebrand the entire experience.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "theme")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub tenant_id: Uuid,
    pub company_name: String,
    pub logo_url: Option<String>,
    /// Primary brand colour as a hex string, e.g. `#F5451F`.
    pub primary_color: String,
    pub accent_color: String,
    /// Default UI mode: `light` | `dark` | `system`.
    pub default_mode: String,
    /// Legal jargon / boilerplate verbiage, keyed by template name (JSON object).
    pub legal_templates: Json,
    /// Tenant overrides for notification templates, keyed by template name.
    /// Values are either a body string or `{ "subject": …, "body": … }`;
    /// platform defaults live in `api::notify` and apply when a key is absent.
    pub notification_templates: Json,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
