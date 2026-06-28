//! A reusable **document template** owned by an LLC: a lease/contract body, a
//! tenant letter, a welcome email, a notice, etc. The `body` is a Handlebars
//! template whose placeholders (`{{tenant_name}}`, `{{property_address}}`,
//! `{{rent}}`, `{{llc_name}}`, `{{signature_block}}`, …) are merged at render
//! time with the LLC's branding and the target lease/recipient.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llc_template")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub llc_id: Uuid,
    /// `lease` | `tenant_letter` | `welcome_email` | `notice` | `other`.
    pub kind: String,
    pub name: String,
    /// Subject line — used when the template is sent as an email.
    pub subject: Option<String>,
    /// Handlebars template source.
    pub body: String,
    /// Whether this is the default template for its `kind` under this LLC.
    pub is_default: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
